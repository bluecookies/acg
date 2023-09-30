// use a oncecell and env
static DATA_PATH: &str = "/app-data/sappho-bot";
static COOKIE_PATH: &str = "/app-data/sappho-bot/cookie.txt";

pub fn read_cookie() -> Option<String> {
    std::fs::read_to_string(COOKIE_PATH)
        .map_err(|e| log::warn!("couldn't read cookie: {}", e))
        .ok()
}

pub fn save_cookie(c: Option<String>) {
    if let Some(cookie) = c {
        let result =
            std::fs::create_dir_all(DATA_PATH).and_then(|_| std::fs::write(COOKIE_PATH, cookie));
        if let Err(e) = result {
            log::error!("failed to write cookie to file: {}", e);
        }
    } else {
        log::warn!("cookie not a valid string");
    }
}
