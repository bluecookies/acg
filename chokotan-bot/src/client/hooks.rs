use poise::{serenity_prelude as serenity, Event, FrameworkContext, FrameworkError};
use serenity::{Activity, Channel, Context as SerenityContext, Message, OnlineStatus, Ready};

use song_artist::Guess;

use crate::{Data, Error};
type Context<'a> = poise::Context<'a, Data, Error>;

// TODO: load this and the other stuff from a file
static TEXT_OK: &str = "<:shiHib:973027014686699550>";

pub(super) async fn on_ready(
    ctx: &SerenityContext,
    ready: &Ready,
    framework: &poise::Framework<Data, Error>,
    db: database::Database,
) -> Result<Data, Error> {
    ctx.set_presence(
        Some(Activity::playing("the washing machine")),
        OnlineStatus::DoNotDisturb,
    )
    .await;
    log::info!(
        "Logged into Discord as {}#{:04}",
        ready.user.name,
        ready.user.discriminator
    );

    // Register commands
    let builder = poise::builtins::create_application_commands(&framework.options().commands);
    let commands = serenity::Command::set_global_application_commands(ctx, |b| {
        *b = builder;
        b
    })
    .await
    .map_err(Error::RegisterCommands)?;

    log::info!("Registered {} slash commands globally.", commands.len());

    let data = crate::Data::new(db);

    Ok(data)
}

// TODO: put my own errors back
pub(super) async fn on_error(e: FrameworkError<'_, Data, Error>) {
    let result = match e {
        FrameworkError::Command { ctx, error } => ctx.reply(error.to_string()).await.map(|_| ()),
        FrameworkError::UnknownCommand {
            msg,
            ctx,
            msg_content,
            ..
        } => {
            if let Some(cmd_name) = msg_content.split_ascii_whitespace().next() {
                msg.reply(
                    ctx,
                    format!(
                        "unrecognised command: {}. use /help to see list of commands",
                        cmd_name
                    ),
                )
                .await
                .map(|_| ())
            } else {
                Ok(())
            }
        }
        e => poise::builtins::on_error(e).await,
    };
    if let Err(e) = result {
        log::error!("failed to send msg: {:#}", e);
    }
}

pub(super) async fn pre_command(ctx: Context<'_>) {
    log::trace!("{} - {}", &ctx.command().name, &ctx.author().name);
}

pub(super) async fn post_command(ctx: Context<'_>) {
    if let Context::Application(ctx) = ctx {
        if !ctx
            .has_sent_initial_response
            .load(std::sync::atomic::Ordering::Acquire)
        {
            if let Err(e) = ctx.send(|cr| cr.ephemeral(true).content(TEXT_OK)).await {
                log::warn!(
                    "error sending acknowledgement for command `{}` ({:?}): {}",
                    &ctx.command.name,
                    ctx.args,
                    e,
                );
            }
        }
    }
}

pub(super) async fn on_event(
    ctx: &SerenityContext,
    event: &Event<'_>,
    _framework: FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        Event::Message { new_message } => on_message(ctx, new_message, data).await?,
        Event::VoiceStateUpdate { old, new } => on_voice_state_change(ctx, old, new).await?,
        _ => (),
    }
    Ok(())
}

async fn on_message(ctx: &SerenityContext, msg: &Message, data: &Data) -> Result<(), Error> {
    let now = std::time::Instant::now();
    // Check song artist
    let quiz = &data.quiz;
    // Check if in right channel
    if quiz.channel_id() != Some(msg.channel_id) {
        return Ok(());
    }

    if let Some(result) = quiz.handle_guess(&msg.content, now).ok().flatten() {
        match result.song_guess {
            Some(Guess::Incorrect(_g, p)) => msg
                .reply(&ctx.http, format!("{:.1}%", p * 100.0))
                .await
                .map(|_| ()),
            Some(Guess::Correct(_g, t)) => msg
                .reply(&ctx.http, format!("✅ {:.1}s", t))
                .await
                .map(|_| ()),
            None => Ok(()),
        }
        .unwrap_or_else(|e| log::warn!("failed to send message: {}", e));
        let correct = result.num_correct_artists;
        let total = result.total_artists;
        for g in result.artist_guesses {
            match g {
                Guess::Incorrect(_g, p) => {
                    msg.reply(
                        &ctx.http,
                        format!("{:.1}% [{}/{}]", p * 100.0, correct, total),
                    )
                    .await
                }
                Guess::Correct(_g, t) => {
                    msg.reply(&ctx.http, format!("✅ {:.1}s [{}/{}]", t, correct, total))
                        .await
                }
            }
            .map(|_| ())
            .unwrap_or_else(|e| log::warn!("failed to send message: {}", e));
        }
    }

    Ok(())
}

async fn on_voice_state_change(
    ctx: &SerenityContext,
    old: &Option<serenity::VoiceState>,
    new: &serenity::VoiceState,
) -> Result<(), Error> {
    log::trace!("Voice state change: {:?} -> {:?}", old, new);
    // Check if we should leave voice chat
    if let Some(voice) = old {
        // If they didn't actually change channel, don't do anything
        if voice.channel_id == new.channel_id {
            return Ok(());
        }
        if let Some((guild_id, channel_id)) = voice.guild_id.zip(voice.channel_id) {
            // Ignore errors - either songbird not initialised or we don't care
            if let Ok(call) = crate::voice::call_handler(ctx, guild_id).await {
                let mut guard = call.lock().await;
                // Check if it was from our channel
                if guard.current_channel() == Some(channel_id.into()) {
                    let channel = channel_id
                        .to_channel(&ctx.http)
                        .await
                        .map_err(|e| Error::GetChannelErr(channel_id, e))?;
                    // should only be a guild channel in theory
                    if let Channel::Guild(g) = channel {
                        let members = g
                            .members(&ctx.cache)
                            .await
                            .map_err(|e| Error::GetMembersErr(channel_id, e))?;
                        // Leave if there are only bots (including us) in the call
                        //  or possibly including the old member (in case of stale data)
                        // We have already checked that their channel has changed so this
                        //  wont make us leave in case the only other member changes status otherwise
                        if members
                            .iter()
                            .all(|m| m.user.bot || (m.user.id == voice.user_id))
                        {
                            log::debug!("Leaving voice channel {}", channel_id);
                            guard.leave().await.map_err(Error::LeaveCallErr)?;
                        } else {
                            log::debug!(
                                "Remaining members in this voice channel: {:?}",
                                members.iter().map(|m| m.display_name()).collect::<Vec<_>>()
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
