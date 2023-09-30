use std::sync::Arc;

use poise::serenity_prelude as serenity;
use serenity::{ChannelId, Context, Guild, GuildId, UserId};
use tokio::sync::Mutex;

use crate::Error;

pub(crate) async fn call_handler(
    ctx: &Context,
    guild_id: GuildId,
) -> Result<Arc<Mutex<songbird::Call>>, Error> {
    let manager = songbird::get(ctx)
        .await
        .ok_or(Error::SongbirdNotInitialised)?;

    let handler = manager.get_or_insert(guild_id);
    Ok(handler)
}

// TODO: can try join the voice channel if user is in it
pub(crate) async fn check_in_channel(call: &Mutex<songbird::Call>) -> Result<(), Error> {
    let guard = call.lock().await;
    if guard.current_channel().is_some() {
        Ok(())
    } else {
        Err(Error::NotInVoiceChannel)
    }
}

pub(crate) fn get_voice_channel_id(guild: &Guild, user_id: UserId) -> Result<ChannelId, Error> {
    guild
        .voice_states
        .get(&user_id)
        .and_then(|v| v.channel_id)
        .ok_or(Error::NotInVoiceChannel)
}
