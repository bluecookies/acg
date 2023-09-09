use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::framework::standard::macros::{command, group};
use serenity::model::channel::Message;
use serenity::model::Timestamp;

mod songs;
mod quiz;

#[group]
#[commands(ping)]
struct General;
pub(crate) use songs::SONG_GROUP;
pub(crate) use quiz::QUIZ_GROUP;

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    let now: i64 = Timestamp::now().timestamp_millis();
    let sent: i64 = msg.timestamp.timestamp_millis();
    msg.reply(&ctx.http, format!("pong ({} ms)", now - sent)).await?;
    Ok(())
}
