use std::time::{Duration, Instant};

use poise::serenity_prelude as serenity;
use serenity::async_trait;

use songbird::tracks::LoopState;
use songbird::{Event, EventContext};
use tokio::sync::mpsc::Sender;

use crate::{Data, Error};
type Context<'a> = poise::Context<'a, Data, Error>;
type Command = poise::Command<Data, Error>;

use crate::audio::Message as SongMessage;
use crate::bot::CurrentTrack;
use crate::voice;

pub(super) fn commands() -> impl IntoIterator<Item = Command> {
    [join(), play(), stop()].into_iter().map(|mut cmd| {
        cmd.category = Some("Songs");
        cmd
    })
}

// TODO: detect when permissions are not valid to join
/// Join the voice chat
#[poise::command(slash_command, prefix_command)]
async fn join(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id;
    // TODO: could use #[only_in(guilds)]
    let guild_id = if let Some(id) = ctx.guild_id() {
        id
    } else {
        ctx.reply("The `join` command can only be used in a server.")
            .await?;
        return Ok(());
    };
    let context = ctx.serenity_context();
    let connect_to = context
        .cache
        .guild_field(guild_id, |gid| voice::get_voice_channel_id(gid, user_id))
        .ok_or(Error::GuildFieldError(guild_id))??;

    // join the voice channel
    let manager = songbird::get(context)
        .await
        .ok_or(Error::SongbirdNotInitialised)?
        .clone();

    let (handler, result) = manager.join(guild_id, connect_to).await;
    result.map_err(Error::VoiceJoinError)?;
    {
        let mut guard = handler.lock().await;
        guard.deafen(true).await?;
    }

    Ok(())
}

// TODO: is the loaded/progress numbers incorrect now?
/// Play a song
#[poise::command(slash_command, prefix_command)]
async fn play(
    ctx: Context<'_>,
    #[description = "URL of the (mp3) song to play"] url: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(Error::NotInGuild)?;
    let context = ctx.serenity_context();
    let handler = voice::call_handler(context, guild_id).await?;
    voice::check_in_channel(&handler).await?;

    // Create an embed with the loading message
    let mut song_info = crate::client::messages::PlaySongMessage::new(ctx, url.clone()).await?;

    let sample = stream_song::Sample::start();
    let (source, mut loader_rx, cancel_token) = crate::audio::create_input(&url, sample).await?;
    let (mut track, track_handle) = songbird::create_player(source);

    // Set the actual track handle
    {
        let handle = track_handle.clone();
        let cancel_token = cancel_token.clone();
        let mut guard = ctx.data().track.write().expect("lock poisoned");
        let _ = guard.replace(CurrentTrack {
            handle,
            cancel_token,
        });
    }

    let (tx, mut track_rx) = tokio::sync::mpsc::channel(128);
    // this will always succeed since we set is_seekable to true
    track
        .set_loops(LoopState::Infinite)
        .expect("failed to set loops");
    track_handle.add_event(
        Event::Periodic(Duration::from_millis(1000), None),
        TrackTickHandler(tx),
    )?;
    // receive messages from loader
    // update embed
    // start playing once buffered enough
    let mut track = Some(track);
    let mut last_update = (Instant::now(), None);
    const UPDATE_DURATION: Duration = Duration::from_millis(1200);
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
                        SongMessage::StartSample(_) => (),
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
    // but this might depend on if we were cancelled

    Ok(())
}

// Panics if CurrentTrack data is not initialised
/// Stops the currently playing song
#[poise::command(slash_command, prefix_command)]
async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    // Check if there is a song playing
    let track = {
        let mut guard = ctx.data().track.write().expect("lock poisoned");
        guard.take()
    };

    // Drop the track to stop the song and cancel the token
    if let Some(t) = track {
        // TODO: this might not actually be an error
        if let Err(e) = t.handle.stop() {
            ctx.reply(format!("Error stopping track: {}", e)).await?;
        }
        drop(t);
    } else {
        ctx.reply("No track currently playing").await?;
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
                // is this a bug?
                if let Err(e) = self.0.send(msg).await {
                    println!("failed to send track message: {}", e);
                }
            }
        }
        None
    }
}
