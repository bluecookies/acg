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

mod text {
    pub static LOADING_TEXT: &str = "Loading...";
    pub static LOADED_SONG_TEXT: &str = "Loaded song";
    pub static PLAYING_TEXT: &str = "Playing...";
    pub static FINISHED_PLAYING_TEXT: &str = "Finished playing";
}
