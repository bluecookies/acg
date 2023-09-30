use warp::{Filter, Rejection, Reply};

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
        return format!(
            "{} days, {} hours, {} minutes, {} seconds, {} milliseconds",
            days, hours, minutes, seconds, millis
        );
    }
    if hours != 0 {
        return format!(
            "{} hours, {} minutes, {} seconds, {} milliseconds",
            hours, minutes, seconds, millis
        );
    }
    if minutes != 0 {
        return format!(
            "{} minutes, {} seconds, {} milliseconds",
            minutes, seconds, millis
        );
    }
    if seconds != 0 {
        return format!("{} seconds, {} milliseconds", seconds, millis);
    }
    return format!("{} milliseconds", millis);
}

#[cfg(feature = "sappho")]
fn with_bot(
    bot: sappho_bot::Bot,
) -> impl Filter<Extract = (sappho_bot::Bot,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || bot.clone())
}

// TODO: no cache filter wrap
//  for uptime, status, connect_get

pub fn uptime(
    start_time: time::Instant,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::path!("uptime")
        .and(warp::get())
        .map(move || format_duration(start_time.elapsed()))
}

pub fn status(
    #[cfg(feature = "sappho")] bot: sappho_bot::Bot,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let f = warp::path!("status").and(warp::get()).map(|| "Status\n");
    #[cfg(feature = "sappho")]
    let f = f
        .and(with_bot(bot))
        .map(|bot: sappho_bot::Bot| bot.status());
    f
}

#[cfg(feature = "sappho")]
pub fn connect_sa_bot(
    bot: sappho_bot::Bot,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let connect_get = warp::path!("connect")
        .and(warp::get())
        .and(warp::filters::fs::file("public/connect.html"));

    #[derive(serde::Deserialize)]
    struct Form {
        action: String,
    }

    let connect_post = warp::path!("connect")
        .and(warp::post())
        .and(with_bot(bot))
        .and(warp::body::form())
        .then(|bot: sappho_bot::Bot, form: Form| async move {
            let cookie = match form.action.as_str() {
                "connect" => crate::bot::read_cookie(),
                "connect_no_cookie" => None,
                _ => {
                    let error = "bad action";
                    let code = warp::http::StatusCode::BAD_REQUEST;
                    return warp::reply::with_status(error, code).into_response();
                }
            };
            match bot.connect(cookie).await {
                Ok(c) => {
                    // save the cookie
                    crate::bot::save_cookie(c);
                    warp::redirect::see_other(warp::http::Uri::from_static("/status"))
                        .into_response()
                }
                Err(e) => {
                    log::error!("Failed to connect: {}", e);
                    let error = e.to_string();
                    let code = warp::http::StatusCode::INTERNAL_SERVER_ERROR;
                    warp::reply::with_status(error, code).into_response()
                }
            }
        });
    connect_get.or(connect_post)
}
