use std::sync::Arc;
use std::sync::RwLock;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use poise::serenity_prelude as serenity;
use serenity::async_trait;
use serenity::{ChannelId, Http};
use songbird::{Call, EventContext};

mod config;
mod settings;

use crate::{Data, Error};
type Context<'a> = poise::Context<'a, Data, Error>;
type Command = poise::Command<Data, Error>;

use crate::bot::CurrentTrack;
use crate::client::messages::QuizSongMessage;
use crate::quiz::QuizSongData;
use crate::quiz::SongArtistQuiz;
use crate::voice;

pub(super) fn commands() -> impl IntoIterator<Item = Command> {
    [start_quiz(), stop_quiz(), skip_song()]
        .into_iter()
        .chain(settings::commands())
        .chain(config::commands())
        .map(|mut cmd| {
            cmd.category = Some("Quiz");
            cmd
        })
}

// TODO:
//  autocomplete on quiz name type
// TODO: only in guild
// TODO: add slash command back
//  need additional prompt for params
//  use a diff func call and get slash_action and parameters
/// Démarrer le blindtest
#[poise::command(prefix_command, discard_spare_arguments, aliases("start_quiz"))]
async fn start_quiz(
    ctx: Context<'_>,
    #[description = "Name of the quiz configuration"] name: String,
    params: poise::KeyValueArgs,
) -> Result<(), Error> {
    // Check that we are in a voice channel in the server this was called in
    let guild_id = ctx.guild_id().ok_or(Error::NotInGuild)?;
    let context = ctx.serenity_context();
    let handler = voice::call_handler(context, guild_id).await?;
    voice::check_in_channel(&handler).await?;

    let channel_id = ctx.channel_id();
    let data = ctx.data();

    let quiz = &data.quiz;
    quiz.start(channel_id, name, params.0)?;

    // Spawn the quiz logic loop
    let qc = quiz.clone();
    let curr_track = ctx.data().track.clone();
    let http = ctx.serenity_context().http.clone();
    let quiz_task = tokio::task::spawn(run_quiz(qc, handler, curr_track, channel_id, http));
    quiz.set_task(quiz_task);

    Ok(())
}

/// Stop the currently playing quiz
#[poise::command(slash_command, prefix_command)]
async fn stop_quiz(ctx: Context<'_>) -> Result<(), Error> {
    let quiz = &ctx.data().quiz;
    // TODO: only allow stop in same channel
    if let Some(task) = quiz.stop()? {
        task.await.map_err(Error::QuizTaskErr)??;
    }

    ctx.reply("Quiz stopped.").await?;
    Ok(())
}

// TODO: check in same channel
/// Skip the currently playing song in the quiz
#[poise::command(slash_command, prefix_command, aliases("skip"))]
async fn skip_song(ctx: Context<'_>) -> Result<(), Error> {
    ctx.data().quiz.skip_song()?;
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
    handler: Arc<Mutex<Call>>,
    curr_track: Arc<RwLock<Option<CurrentTrack>>>,
    channel_id: ChannelId,
    http: Arc<Http>,
) -> Result<(), Error> {
    let quiz_token = quiz.cancel_token();

    // struct passed into the loader
    //  to get the song info and sample used
    let mut song_data = QuizSongData::default();
    let mut next_load = Box::pin(quiz.load_next_song(&mut song_data));
    let mut load_result = None;

    let mut song_num = 0;
    let mut quit = false;
    let mut num_errors = 0;
    while !quit {
        if num_errors >= 3 {
            channel_id
                .say(&http, "Stopping quiz because too many errors.")
                .await?;
            // This returns the handle of the task we are in currently
            // If we awaited this, it would never exit
            let _ = quiz.stop()?;
            break;
        }
        // Wait for the song if it didn't finish loading
        // TODO: add a way to cancel this loading
        // TODO: also get messages from this, maybe return a stream
        let (result, mut song_msg) = if let Some(result) = load_result.take() {
            // TODO: save the time it was loaded
            // drop this completed future since its holding a reference to `song_data`
            drop(next_load);
            let msg = QuizSongMessage::new(
                http.clone(),
                channel_id,
                song_num + 1,
                true,
                // could be extended to pass in results of loading
            )
            .await?;
            (result, msg)
        } else {
            // We are still fetching/buffering next song
            let mut msg =
                QuizSongMessage::new(http.clone(), channel_id, song_num + 1, false).await?;

            // Set a cancel token here to allow skipping the song when it won't load
            let cancel_token = CancellationToken::new();
            quiz.set_song_token(cancel_token.clone())?;
            tokio::select! {
                () = cancel_token.cancelled() => {
                    song_num += 1;
                    quiz.set_song_number(song_num)?;
                    let song_info = std::mem::take(&mut song_data);
                    next_load = Box::pin(quiz.load_next_song(&mut song_data));
                    msg.set_cancelled(song_info).await?;
                    continue;
                }
                result = next_load => (result, msg),
            }
        };

        // Extract the song info - we can only do this here
        // because a reference `song_data` is not being held by the loader
        let song_info = std::mem::take(&mut song_data);

        // Check error/update embed status
        let source = match result {
            Ok(v) => {
                song_msg.set_loaded().await?;
                num_errors = 0;
                v
            }
            Err(e @ Error::NoQuizRunning) => {
                song_msg.set_error(e, song_info).await?;
                break;
            }
            Err(e) => {
                song_msg.set_error(e, song_info).await?;
                song_num += 1;
                quiz.set_song_number(song_num)?;
                next_load = Box::pin(quiz.load_next_song(&mut song_data));
                num_errors += 1;
                continue;
            }
        };

        // TODO: also, set the event to check against the total duration (in case of lag)
        //   will the track even resume playing in that case?
        let (track, handle) = songbird::create_player(source);
        let next_song_token = CancellationToken::new();
        handle
            .add_event(
                songbird::Event::Track(songbird::TrackEvent::End),
                TrackEndHandler(next_song_token.clone()),
            )
            .expect("failed to add track end event");

        // Go to next song and play it
        song_num += 1;
        log::trace!("Setting song number to {}", song_num);
        quiz.set_song_number(song_num)?;
        let song_token = next_song_token;
        next_load = Box::pin(quiz.load_next_song(&mut song_data));

        // Scope the mutex lock guard to avoid deadlocking
        // The audio driver will try to lock the call (to pause) when disconnected/moved
        {
            let mut guard = handler.lock().await;
            guard.play_only(track);
        }
        // Update the global current track and drop the old one
        {
            let cancel_token = song_token.clone();
            let mut guard = curr_track.write().expect("lock poisoned");
            let _ = guard.replace(CurrentTrack {
                handle,
                cancel_token,
            });
        }

        log::trace!("Playing track...");

        // TODO - add button on embed to skip
        quiz.set_song_token(song_token.clone())?;

        // Update embed status
        song_msg.set_playing().await?;

        // Wait for next song to be played or for quiz cancellation
        loop {
            log::trace!("Waiting for quiz song to end...");
            tokio::select! {
                _ = song_token.cancelled() => break,
                _ = quiz_token.cancelled() => {
                    quit = true;
                    break
                }
                x = &mut next_load, if load_result.is_none() => load_result = Some(x),
            }
        }
        log::trace!("Quiz song finished");

        // Update embed to status of last song
        song_msg.set_finished(song_info).await?;
    }
    Ok(())
}
