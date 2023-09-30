mod play_song;
mod sa_quiz;

pub(crate) use play_song::PlaySongMessage;
pub(crate) use sa_quiz::QuizSongMessage;

mod colours {
    pub const PLAYING_COLOUR: u32 = 0x13F203;
    pub const ERROR_COLOUR: u32 = 0xE90000;
    pub const FINISHED_COLOUR: u32 = 0x1411C2;
    pub const LOADED_COLOUR: u32 = 0x9D00FF;
}

pub mod text {
    pub static LOADING_TEXT: &str = "Loading...";
    pub static LOADED_SONG_TEXT: &str = "Loaded song";
    pub static PLAYING_TEXT: &str = "Playing...";
    pub static FINISHED_PLAYING_TEXT: &str = "Finished playing";
    pub static CANCELLED_SONG_TEXT: &str = "Cancelled loading song";

    // hardcoded emoji heh - TODO: read this from a file once
    pub static CHECKMARK_YES: &str = "✅";
    pub static CHECKMARK_NO: &str = "<:no:1150290822726684712>";
}

// TODO: use a wrapper type and impl display for it
fn format_time(seconds: f64) -> String {
    let seconds = seconds as u64;
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    if minutes >= 60 {
        let (hours, minutes) = (minutes / 60, minutes % 60);
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

use std::borrow::Cow;
fn value_to_string(value: &database::Value) -> Cow<str> {
    use database::Value::*;
    match value {
        None => Cow::default(),
        String(s) => Cow::Borrowed(s.as_str()),
        Bool(b) => Cow::Borrowed(if *b {
            text::CHECKMARK_YES
        } else {
            text::CHECKMARK_NO
        }),
        Integer(i) => Cow::Owned(i.to_string()),
        Float(f) => Cow::Owned(format!("{:.1}", f)),
    }
}
