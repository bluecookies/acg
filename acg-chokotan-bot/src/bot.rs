use serenity::{client::EventHandler, async_trait, prelude::Context, model::{prelude::{Ready, Activity, Message}, user::OnlineStatus}};
use song_artist::Guess;

use crate::quiz::SongArtistQuiz;

pub(crate) struct Bot;

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        ctx.set_presence(Some(Activity::playing("washing machine")), OnlineStatus::Invisible).await;

        log::info!("Logged into Discord as {}#{:04}", ready.user.name, ready.user.discriminator);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let now = std::time::Instant::now();
        // Check song artist
        let quiz = {
            let data = ctx.data.read().await;
            data.get::<SongArtistQuiz>().expect("song artist not initialised").clone()
        };
        // Check if in right channel
        if quiz.channel_id() != Some(msg.channel_id) {
            return;
        }
        
        if let Some(result) = quiz.handle_guess(&msg.content, now).ok().flatten() {
            match result.song_guess {
                Some(Guess::Incorrect(_g, p)) => msg.reply(&ctx.http, format!("{:.1}%", p * 100.0)).await.map(|_| ()),
                Some(Guess::Correct(_g, t)) => msg.reply(&ctx.http, format!("✅ {:.1}s", t)).await.map(|_| ()),
                None => Ok(()),
            }.unwrap_or_else(|e| log::warn!("failed to send message: {}", e));
            let correct = result.num_correct_artists;
            let total = result.total_artists;
            for g in result.artist_guesses {
                match g {
                    Guess::Incorrect(_g, p) => msg.reply(&ctx.http, format!("{:.1}% [{}/{}]", p * 100.0, correct, total)).await,
                    Guess::Correct(_g, t) => msg.reply(&ctx.http, format!("✅ {:.1}s [{}/{}]", t, correct, total)).await,
                }.map(|_| ()).unwrap_or_else(|e| log::warn!("failed to send message: {}", e));
            }
        }
    }
}
