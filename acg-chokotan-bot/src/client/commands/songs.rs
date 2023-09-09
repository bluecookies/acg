use std::time::{Instant, Duration};
use serenity::async_trait;
use serenity::client::Context;
use serenity::framework::standard::{Args, CommandResult};
use serenity::framework::standard::macros::{command, group};
use serenity::model::channel::Message;
use songbird::{EventContext, Event};
use songbird::tracks::LoopState;
use tokio::sync::mpsc::Sender;

use crate::Error;
use crate::audio::Message as SongMessage;
use crate::client::{CurrentTrack, PlayTrackCommand};


// TODO: add debug command for song metadata
#[group]
#[commands(join, play, stop)]
struct Song;

// TODO: detect when permissions are not valid to join
#[command]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let user_id = msg.author.id;
    // could use #[only_in(guilds)]
    let guild_id = if let Some(id) = msg.guild_id { id } else {
        msg.reply(&ctx.http, "The `join` command can only be used in a server.").await?;
        return Ok(());
    };
    let map = if let Some(map) = ctx.cache.guild_field(guild_id, |g| g.voice_states.clone()) {
        map
    } else {
        return Err(format!("failed to get guild fields for guild id {}", guild_id))?;
    };
    if let Some(connect_to) = map.get(&user_id).and_then(|v| v.channel_id) {
        // join the voice channel
        let manager = songbird::get(ctx).await
            .ok_or(Error::SongbirdNotInitialised)?
            .clone();

        let (handler, result) = manager.join(guild_id, connect_to).await;
        result?;
        let mut lock_guard = handler.lock().await;
        lock_guard.deafen(true).await?;
    } else {
        msg.reply(&ctx.http, "You are not in a voice channel.").await?;
        return Ok(());
    }
    Ok(())
}

// TODO: allow stop the previous song if it is playing
#[command]
#[min_args(1)]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let handler = crate::voice::get_voice_call_handler(ctx, msg).await?;

    let url = args.quoted().single::<String>().expect("not enough args");
    // Check if there is already a song playing
    {
        let mut data = ctx.data.write().await;
        let curr_track = data.get_mut::<CurrentTrack>().expect("song commands not initialised");
        if curr_track.is_some() {
            Err(Error::SongAlreadyPlaying)?;
        } else {
            *curr_track = Some(CurrentTrack {
                channel_id: msg.channel_id,
                command: PlayTrackCommand::Play,
                handle: None,
                cancel_token: None,
            });
        }
    }

    // create an embed with the loading message
    let mut song_info = crate::client::messages::PlaySongMessage::new(
        ctx.http.clone(),
        msg,
        url.clone(),
    ).await?;

    let sample = stream_song::Sample::start();
    let (source, mut loader_rx, cancel_token) = crate::audio::create_input(&url, sample).await?;
    let (mut track, track_handle) = songbird::create_player(source);

    // Set the actual track handle
    {
        let mut data = ctx.data.write().await;
        let curr_track = data.get_mut::<CurrentTrack>().expect("song commands not initialised");
        if let Some(track) = curr_track {
            track.handle = Some(track_handle.clone());
            track.cancel_token = Some(cancel_token.clone());
        } else {
            // the track must have been stopped already
            song_info.set_finished().await?;
            return Ok(())
        }
    }

    let (tx, mut track_rx) = tokio::sync::mpsc::channel(128);
    // this will always succeed since we set is_seekable to true
    track.set_loops(LoopState::Infinite).expect("failed to set loops");
    track_handle.add_event(Event::Periodic(Duration::from_millis(1000), None), TrackTickHandler(tx))?;
    // receive messages from loader
    // update embed
    // start playing once buffered enough
    let mut track = Some(track);
    let mut last_update = (Instant::now(), None);
    const UPDATE_DURATION: Duration = Duration::from_millis(1000);
    let mut loading_done = false;
    loop {
        tokio::select! {
            Some(x) = track_rx.recv() => match x {
                TrackMessage::Position(t) => song_info.update_time(t).await?,
            },
            // the token must be cancelled (from track end or otherwise) for this loop to exit
            _ = cancel_token.cancelled() => { 
                log::debug!("Cancelling song: {}", &url);
                break;
            }
            x = loader_rx.recv(), if !loading_done => {
                if let Some(m) = x {
                    match m {
                        SongMessage::TotalDuration(t) => song_info.update_total_duration(t.seconds as f64 + t.frac).await?,
                        SongMessage::Update(t) => {
                            if last_update.0.elapsed() >= UPDATE_DURATION {
                                song_info.update_loaded(t.seconds as f64 + t.frac).await?;
                                last_update = (Instant::now(), None);
                            } else {
                                last_update.1 = Some(t);
                            }

                            // 15 seconds buffered for now - TODO
                            if t.seconds >= 15 {
                                if let Some(t) = track.take() {
                                    song_info.set_playing().await?;
                                    let mut guard = handler.lock().await;
                                    guard.play_only(t);
                                }
                            }
                        }
                        SongMessage::DecodeError(e) => {
                            log::debug!("Decode error in song {}: {}", &url, &e);
                            song_info.set_error(e).await?;
                            cancel_token.cancel();
                        }
                    }
                } else {
                    loading_done = true;
                    // song finished loading or errored
                    if let Some(t) = last_update.1 {
                        song_info.update_loaded(t.seconds as f64 + t.frac).await?;
                    }
                    // not enough was buffered or the messages were missed
                    // either way, play the track anyways
                    if let Some(t) = track.take() {
                        song_info.set_playing().await?;
                        let mut guard = handler.lock().await;
                        guard.play_only(t);
                    }
                }
            }
            else => break,
        }
    }
    song_info.set_finished().await?;
    // TODO: remove current track here

    Ok(())
}


// Panics if CurrentTrack data is not initialised
#[command]
async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
    // Check if there is a song playing
    // and it is started in the same channel
    let mut data = ctx.data.write().await;
    let current_track = data.get_mut::<CurrentTrack>().expect("song commands not initialised").take();
    if let Some(track) = current_track {
        // Check whether the current track was started from a play command
        match track.command {
            PlayTrackCommand::StartQuiz => {
                msg.reply(&ctx.http, "Quiz in progress. Stop the quiz first.").await?;
                return Ok(());
            }
            PlayTrackCommand::Play => (),
            #[allow(unreachable_patterns)]
            _ => {
                // IncorrectStopCommand(command)
                msg.reply(&ctx.http, "Track was not started with `play`").await?;
                return Ok(());
            }
        }

        if msg.channel_id != track.channel_id {
            msg.reply(&ctx.http, "Cannot stop track from this channel").await?; // TODO: put these into crate error
            return Ok(());
        }

        if let Some(ref token) = track.cancel_token {
            token.cancel();
        }

        if let Some(ref handle) = track.handle {
            if let Err(e) = handle.stop() {
                let content = format!("Error stopping track: {}", e);
                msg.reply(&ctx.http, content).await?;    
            }
        }
    } else {
        // there is no track playing right now
        msg.reply(&ctx.http, "No track currently playing").await?;
        return Ok(());
    }
    Ok(())
}



enum TrackMessage {
    // Track position in seconds
    Position(f64),
}

struct TrackTickHandler(Sender<TrackMessage>);

#[async_trait]
impl songbird::EventHandler for TrackTickHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            for (ts, _th) in track_list.iter() {
                let msg = TrackMessage::Position(ts.position.as_secs_f64());
                // let _ = self.0.send(msg).await;
                if let Err(e) = self.0.send(msg).await {
                    println!("failed to send track message: {}", e);
                }
            }
        }
        None
    }
}
