use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;

use serenity::framework::StandardFramework;
use serenity::model::prelude::ChannelId;
use serenity::prelude::{GatewayIntents, TypeMapKey};
use songbird::SerenityInit;
use songbird::tracks::TrackHandle;

use crate::Error;
use crate::quiz::SongArtistQuiz;

mod commands;
mod messages;
mod hooks;


pub struct Client {
    inner: Arc<Mutex<ClientInner>>,
}

enum ClientInner {
    // NotBuilt(serenity::ClientBuilder),
    NotStarted(serenity::Client),
    Started,
    BuildFailed(serenity::Error),
}

impl Client {
    pub async fn new(token: impl AsRef<str>, db: database::Database) -> Self {
        let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
        let framework = StandardFramework::new()
            .configure(|c| c.prefixes(["~!", "s!", "!@"]))
            .unrecognised_command(hooks::unrecognised_command_hook)
            .after(hooks::after_hook)
            .on_dispatch_error(hooks::dispatch_error_hook)
            .group(&commands::GENERAL_GROUP)
            .group(&commands::SONG_GROUP)
            .group(&commands::QUIZ_GROUP);

        let result = serenity::Client::builder(&token, intents)
            .event_handler(crate::Bot)
            .framework(framework)
            .register_songbird()
            .await;

        let inner = match result {
            Ok(v) => {
                insert_global_data(&v, db).await;
                ClientInner::NotStarted(v)
            }
            Err(e) => ClientInner::BuildFailed(e),
        };

        Client {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub async fn start(&self) -> Result<(), Error> {
        let old = {
            let mut guard = self.inner.lock().expect("mutex poisoned");
            let old = std::mem::replace(&mut *guard, ClientInner::Started);
            if let ClientInner::BuildFailed(ref e) = old {
                let err = Error::ClientBuildError(e.to_string());
                *guard = old;
                return Err(err);
            }
            old
        };
        match old {
            ClientInner::NotStarted(mut c) => {
                log::info!("Starting chokotan bot...");
                c.start().await?;
                log::info!("Exiting chokotan bot...");
                Ok(())
            },
            ClientInner::Started => Err(Error::AlreadyStarted),
            ClientInner::BuildFailed(_) => unreachable!("chokotan bot client build failed"),
        }
    }
}




// TODO: this only works for one server right now, 
//  can't have more than one track playing at a time
struct CurrentTrack {
    // the channel that the command to play the track was sent from
    channel_id: ChannelId,
    // the command that was used to play this track
    command: PlayTrackCommand,
    // the track might not have been created yet
    // but we still want to set the data to stop others from taking it
    handle: Option<TrackHandle>,
    cancel_token: Option<CancellationToken>,
}

enum PlayTrackCommand {
    Play,
    StartQuiz,
}

impl Drop for CurrentTrack {
    fn drop(&mut self) {
        if let Some(ref token) = self.cancel_token {
            token.cancel();
        }
    }
}

impl TypeMapKey for CurrentTrack {
    type Value = Option<Self>;
}

impl TypeMapKey for SongArtistQuiz {
    type Value = Self;
}

// Inserts the global data
async fn insert_global_data(client: &serenity::Client, db: database::Database) {
    let mut data: tokio::sync::RwLockWriteGuard<'_, serenity::prelude::TypeMap> = client.data.write().await;
    data.insert::<CurrentTrack>(None);
    data.insert::<SongArtistQuiz>(SongArtistQuiz::new(db));
}
