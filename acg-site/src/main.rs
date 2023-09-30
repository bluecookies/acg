mod error;
mod filters;
mod reply;
mod song;
mod stats;

use error::Error;

#[tokio::main]
async fn main() {
    let start_time = time::Instant::now();
    // TODO: log to web dashboard too
    env_logger::init();
    log::info!("Logger initialised");

    let db = match std::env::var("DATABASE_URL") {
        Ok(c) => database::Database::new(c),
        Err(e) => {
            log::error!("connection string DATABASE_URL env variable error: {}", e);
            database::Database::error("no connection string")
        }
    };

    let uptime = filters::uptime(start_time);

    // database access
    let song_data_post = filters::song_data_post(db.clone());
    let song_stats = filters::song_stats(db.clone());
    let song_search = filters::song_search(db);

    let files = filters::files();

    // TODO: move this to filters
    use warp::Filter;
    let routes = uptime
        .or(song_data_post)
        .or(song_stats)
        .or(song_search)
        .or(files)
        .with(warp::compression::gzip());

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
}
