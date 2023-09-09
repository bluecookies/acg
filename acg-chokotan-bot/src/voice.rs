use std::sync::Arc;

use serenity::{prelude::Context, model::prelude::Message};
use tokio::sync::Mutex;

use crate::Error;

pub(crate) async fn get_voice_call_handler(ctx: &Context, msg: &Message) -> Result<Arc<Mutex<songbird::Call>>, Error> {
    let guild_id = msg.guild_id.ok_or(Error::NotInGuild)?;
    
    let manager = songbird::get(ctx).await
        .ok_or(Error::SongbirdNotInitialised)?;
    
    // TODO: try join the voice channel if user is in it
    // TODO: also this could exist even if not in a channel, so use current_channel to check
    let handler = manager.get(guild_id).ok_or(Error::NotInVoiceChannel)?;
    Ok(handler)
}
