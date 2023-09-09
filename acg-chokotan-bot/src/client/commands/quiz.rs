use std::sync::Arc;

use serenity::async_trait;
use serenity::client::Context;
use serenity::framework::standard::{CommandResult, Args};
use serenity::framework::standard::macros::{command, group};
use serenity::http::Http;
use serenity::model::channel::Message;
use serenity::model::prelude::ChannelId;
use songbird::{EventContext, Call};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::Error;
use crate::client::messages::QuizSongMessage;
use crate::client::{SongArtistQuiz, CurrentTrack, PlayTrackCommand};
use crate::quiz::QuizSongData;

// TODO commands:
//  - skip songs that buffer
//  - set sensitive/threshold
//  - show settings
//  - button to skip song
//  - show sample that was used
//  - show anime/event it is from
//  - alternative names for artists
//  - default quiz type
//  - don't auto go to next song after guessing

#[group]
#[commands(start_quiz, stop_quiz, skip_song)]
struct Quiz;

#[command]
async fn start_quiz(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let handler = crate::voice::get_voice_call_handler(ctx, msg).await?;

    let channel_id = msg.channel_id;
    // Check if there is already a song playing
    {
        let mut data = ctx.data.write().await;
        let curr_track = data.get_mut::<CurrentTrack>().expect("song commands not initialised");
        if curr_track.is_some() {
            Err(Error::SongAlreadyPlaying)?;
        } else {
            *curr_track = Some(CurrentTrack {
                channel_id,
                command: PlayTrackCommand::StartQuiz,
                handle: None,
                cancel_token: None,
            });
        }
    }


    let quiz = {
        let data = ctx.data.read().await;
        data.get::<SongArtistQuiz>().expect("song artist not initialised").clone()
    };

    let quiz_type = args.quoted().single::<String>().ok();

    quiz.start(channel_id, quiz_type)?;

    let http = ctx.http.clone();
    let qc = quiz.clone();
    let quiz_task = tokio::spawn(run_quiz(
        qc,
        http,
        channel_id,
        handler,
    ));
    quiz.set_task(quiz_task);

    Ok(())
}

#[command]
async fn stop_quiz(ctx: &Context, msg: &Message) -> CommandResult {
    let mut data = ctx.data.write().await;
    let quiz = data.get::<SongArtistQuiz>().expect("song artist not initialised").clone();
    // TODO: only allow stop in same channel
    if let Some(task) = quiz.stop()? {
        task.await??;
    }
    let curr_track = data.get_mut::<CurrentTrack>().expect("song commands not initialised");
    
    // TODO - don't actually take the track, just change the command
    //  to something that will allow `stop` to stop it
    if let Some(_track) = curr_track.take() {
        // TODO:
        //  we don't actually set the track handle in here or anything
        //  but we would stop it and cancel the token here
        // TODO - current track should actually synced with the track 
        //  that is currently playing or that has the handle to play it
    }
    msg.reply(&ctx.http, "Quiz stopped.").await?;
    Ok(())
}

#[command]
#[aliases(skip)]
async fn skip_song(ctx: &Context, _msg: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let quiz = data.get::<SongArtistQuiz>().expect("song artist not initialised").clone();
    quiz.skip_song()?;
    Ok(())
}

struct TrackEndHandler(CancellationToken);

#[async_trait]
impl songbird::EventHandler for TrackEndHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<songbird::Event> {
        if let EventContext::Track(_) = ctx {
            log::debug!("Cancelling - end of track");
            self.0.cancel();
        }
        None
    }
}

async fn run_quiz(
    quiz: SongArtistQuiz,
    http: Arc<Http>,
    channel_id: ChannelId,
    handler: Arc<Mutex<Call>>,
) -> Result<(), Error> {
    let quiz_token = quiz.cancel_token();

    // struct passed into the loader 
    //  to get the song info and sample used
    let mut song_data = QuizSongData::default();
    let mut next_load = Box::pin(quiz.load_next_song(&mut song_data));
    let mut load_result = None;

    let mut song_num = 0;
    let mut quit = false;
    while !quit {
        // wait for the song if it didn't finish loading
        // TODO: add a way to cancel this loading
        // TODO: also get messages from this, maybe return a stream
        let (result, mut song_msg) = 
            if let Some(result) = load_result.take() { // TODO: save the time it was loaded
                // drop this completed future since its holding a reference to `song_data`
                drop(next_load);
                let msg = QuizSongMessage::new(
                    http.clone(),
                    channel_id,
                    song_num + 1,
                    true,
                    // could be extended to pass in results of loading
                ).await?;
                (result, msg)
            } else {
                // still fetching/buffering next song
                let msg = QuizSongMessage::new(
                    http.clone(),
                    channel_id,
                    song_num + 1,
                    false,
                ).await?;
                (next_load.await, msg)
            };

        // Extract the song info - we can only do this here 
        // because a reference `song_data` is not being held by the loader
        let song_info = song_data.song_info.take();

        // Check error/update embed status
        let source= match result { 
            Ok(v) => {
                song_msg.set_loaded().await?;
                v
            },
            Err(e) => {
                song_msg.set_error(e, song_info).await?;
                song_num += 1;
                next_load = Box::pin(quiz.load_next_song(&mut song_data));
                continue;
            },
        };

        // TODO: also, set the event to check against the total duration (in case of lag)
        //   will the track even resume playing in that case?
        let (track, handle) = songbird::create_player(source);
        let next_song_token = CancellationToken::new();
        handle.add_event(
            songbird::Event::Track(songbird::TrackEvent::End), 
            TrackEndHandler(next_song_token.clone())
        ).expect("failed to add track end event");
    
        // TODO: Put the handle into current track and cancel the last one

        // Go to next song and play it
        song_num += 1;
        quiz.set_song_number(song_num)?;
        let song_token = next_song_token;
        next_load = Box::pin(quiz.load_next_song(&mut song_data));

        let mut guard = handler.lock().await;
        guard.play_only(track);

        // TODO - add button on embed to skip
        quiz.set_song_token(song_token.clone())?;

        // Update embed status
        song_msg.set_playing().await?;

        // Wait for next song to be played or for quiz cancellation
        loop {
            tokio::select! {
                _ = song_token.cancelled() => break,
                _ = quiz_token.cancelled() => {
                    quit = true;
                    break
                }
                x = &mut next_load, if load_result.is_none() => load_result = Some(x),
            }
        }

        // Update embed to status of last song
        song_msg.set_finished(song_info).await?;
    }
    Ok(())
}