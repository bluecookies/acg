#[cfg(feature = "sappho")]
mod bot;
mod error;
mod filters;

#[tokio::main]
async fn main() {
    let start_time = time::Instant::now();
    // TODO: log to web dashboard too
    env_logger::init();
    log::info!("Logger initialised");

    // TODO: this is on if any of the bots are on
    let db = match std::env::var("DATABASE_URL") {
        Ok(c) => database::Database::new(c),
        Err(e) => {
            log::error!("connection string DATABASE_URL env variable error: {}", e);
            database::Database::error("no connection string")
        }
    };

    #[cfg(feature = "sappho")]
    let sa_bot = sappho_bot::Bot::new(db.clone());

    match std::env::var("DISCORD_TOKEN") {
        Ok(c) => {
            tokio::spawn(async move {
                let client = chokotan_bot::Client::new(&c, db).await;
                if let Err(e) = client.start().await {
                    log::error!("chokotan bot error: {}", e);
                }
                Ok::<(), ()>(())
            });
        }
        Err(e) => {
            log::error!("discord token DISCORD_TOKEN env variable error: {}", e);
        }
    };

    let uptime = filters::uptime(start_time);
    // TODO: status of both bots
    let status = filters::status(
        #[cfg(feature = "sappho")]
        sa_bot.clone(),
    );

    #[cfg(feature = "sappho")]
    let bot_connect = filters::connect_sa_bot(sa_bot);

    use warp::Filter;
    let routes = uptime.or(status);

    #[cfg(feature = "sappho")]
    let routes = routes.or(bot_connect);

    // let sigterm = signal::signal(signal::SignalKind::terminate())
    //     .map_err(|e| log::error!("signal listener creation failed: {}", e))
    //     .ok();
    let (_addr, server) = warp::serve(routes).bind_with_graceful_shutdown(
        ([0, 0, 0, 0, 0, 0, 0, 0], 8080),
        async move {
            #[cfg(unix)]
            let result = {
                use tokio::signal::unix as signal;
                signal::signal(signal::SignalKind::terminate())
            };
            #[cfg(windows)]
            let result = {
                use tokio::signal::windows as signal;
                signal::ctrl_c() // what about ctrl+close?
            };
            result.expect("failed to listen for event").recv().await;
        },
    );

    server.await;
    // TODO: disconnect here and also provide route for it
    // TODO: handle signals, disconnect, write data,
    //   bot.disconnect().await;

    log::info!("Exiting...");
}
