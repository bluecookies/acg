use warp::{Filter, Rejection, Reply};
use database::Database;
use crate::reply::JsonReply;
use crate::stats;
use crate::song::{update_song_data, search_song, query_song};

fn format_duration(dur: time::Duration) -> String {
    let total_days = dur.whole_days();
    let total_hours = dur.whole_hours();
    let total_minutes = dur.whole_minutes();
    let total_seconds = dur.whole_seconds();
    let total_millis = dur.whole_milliseconds();
    let days = total_days;
    let hours = total_hours - total_days * 24;
    let minutes = total_minutes - total_hours * 60;
    let seconds = total_seconds - total_minutes * 60;
    let millis = total_millis - total_seconds as i128 * 1000;
    if days != 0 {
        return format!("{} days, {} hours, {} minutes, {} seconds, {} milliseconds", days, hours, minutes, seconds, millis);
    }
    if hours != 0 {
        return format!("{} hours, {} minutes, {} seconds, {} milliseconds", hours, minutes, seconds, millis);
    }
    if minutes != 0 {
        return format!("{} minutes, {} seconds, {} milliseconds", minutes, seconds, millis);
    }
    if seconds != 0 {
        return format!("{} seconds, {} milliseconds", seconds, millis);
    }
    return format!("{} milliseconds", millis);
}

fn with_db(db: Database) -> impl Filter<Extract = (Database,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db.clone())
}

pub fn uptime(
    start_time: time::Instant
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::path!("uptime")
        .and(warp::get())
        .map(move || format_duration(start_time.elapsed()))
}

pub fn files() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    // serve /static/{path} from /public/{path}
    warp::path("static").and(warp::fs::dir("public"))
}

pub fn song_data_post(db: Database) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::path!("song")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_db(db))
        .then(update_song_data)
        .map(JsonReply::to_response)
}

pub fn song_stats(db: Database) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone  {
    let song_stats_page = warp::path::end()
        .and(warp::filters::fs::file("public/stats.html"));

    let song_stats_vintage = warp::path!("vintage")
        .and(with_db(db.clone()))
        .then(stats::guess_rate_vintage)
        .map(JsonReply::to_response);

    // TODO: look into (a, b) linear method for difficulty
    let song_stats_difficulty = warp::path!("difficulty" / u32 )
        .and(with_db(db.clone()))
        .then(|num_bins: u32, db: Database| stats::guess_rate_difficulty(db, num_bins, 0))
        .map(JsonReply::to_response);
    let song_stats_difficulty2 = warp::path!("difficulty2" / u32 )
        .and(with_db(db))
        .then(|num_bins: u32, db: Database| stats::guess_rate_difficulty(db, num_bins, 1))
        .map(JsonReply::to_response);

    warp::path!("stats" / ..)
        .and(warp::get())
        .and(song_stats_page
            .or(song_stats_vintage)
            .or(song_stats_difficulty)
            .or(song_stats_difficulty2)
        )
}

pub fn song_search(db: Database) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let search_page = warp::path!("search")
        .and(warp::filters::fs::file("public/search.html"));

    let query = warp::path!("query")
        .and(warp::query::<database::SearchQuery>())
        .and(with_db(db.clone()))
        .then(search_song)
        .map(JsonReply::to_response);

    let song_query = warp::path!("songquery" / i32)
        .and(with_db(db))
        .then(query_song)
        .map(JsonReply::to_response);

    warp::get().and(search_page.or(query).or(song_query))
}


