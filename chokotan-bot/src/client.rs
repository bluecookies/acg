use std::sync::{Arc, Mutex};

use poise::serenity_prelude as serenity;
use serenity::GatewayIntents;
use songbird::SerenityInit;

use crate::{Data, Error};

mod commands;
mod hooks;
mod messages;

pub struct Client {
    inner: Arc<Mutex<ClientInner>>,
}

enum ClientInner {
    // NotBuilt(serenity::ClientBuilder),
    NotStarted(Arc<poise::Framework<Data, Error>>),
    Started,
    BuildFailed(serenity::Error),
}

impl Client {
    pub async fn new(token: impl AsRef<str>, db: database::Database) -> Self {
        let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
        use poise::structs::Prefix;
        let prefix_options = poise::PrefixFrameworkOptions {
            prefix: Some(String::from("s!")),
            additional_prefixes: vec![Prefix::Literal("~!"), Prefix::Literal("!@")],
            mention_as_prefix: true,
            case_insensitive_commands: true,
            ..Default::default()
        };
        let result = poise::Framework::builder()
            .client_settings(|builder| builder.register_songbird())
            .options(poise::FrameworkOptions {
                prefix_options,
                on_error: |err| Box::pin(hooks::on_error(err)),
                pre_command: |ctx| Box::pin(hooks::pre_command(ctx)),
                post_command: |ctx| Box::pin(hooks::post_command(ctx)),
                event_handler: |ctx, event, f, data| Box::pin(hooks::on_event(ctx, event, f, data)),
                commands: commands::commands(),
                ..Default::default()
            })
            .token(token.as_ref())
            .intents(intents)
            .setup(|ctx, ready, framework| Box::pin(hooks::on_ready(ctx, ready, framework, db)))
            .build()
            .await;

        let inner = match result {
            Ok(v) => ClientInner::NotStarted(v),
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
            ClientInner::NotStarted(c) => {
                log::info!("Starting chokotan bot...");
                poise::Framework::start(c).await?;
                log::info!("Exiting chokotan bot...");
                Ok(())
            }
            ClientInner::Started => Err(Error::AlreadyStarted),
            ClientInner::BuildFailed(_) => unreachable!("chokotan bot client build failed"),
        }
    }
}
