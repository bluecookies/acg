use serenity::client::Context;
use serenity::framework::standard::{CommandError, DispatchError};
use serenity::model::prelude::Message;
use serenity::framework::standard::macros::hook;

use crate::Error;


#[hook]
pub(crate) async fn unrecognised_command_hook(
    ctx: &Context,
    msg: &Message,
    cmd_name: &str,
) {
    let payload = format!("unrecognised command: {}", cmd_name);
    if let Err(e) = msg.reply(&ctx.http, &payload).await {
        log::error!("failed to send message ({}): {}", payload, e);
    }
}

#[hook]
pub(crate) async fn after_hook(
    ctx: &Context,
    msg: &Message,
    cmd_name: &str,
    error: Result<(), CommandError>,
) {
    if let Err(e) = error {
        let payload = match e.downcast::<Error>().as_deref() {
            Ok(e @ (
                Error::NotInGuild | 
                Error::NotInVoiceChannel |
                Error::QuizAlreadyStarted
            )) => e.to_string(),
            Ok(e) => format!("command `{}` ({}) failed: {:#}", cmd_name, &msg.content, e),
            Err(e) => format!("command `{}` ({}) failed: {:#}", cmd_name, &msg.content, e),
        };
        if let Err(e) = msg.reply(&ctx.http, &payload).await {
            log::error!("failed to send message ({}): {}", payload, e);
        }
    }
}

#[hook]
pub(crate) async fn dispatch_error_hook(
    ctx: &Context,
    msg: &Message,
    err: DispatchError,
    command: &str
) {
    if let Err(e) = msg.reply(&ctx.http, format!("{}: {:?}", command, err)).await {
        log::error!("failed to send msg: {:#}", e);
    }
}


