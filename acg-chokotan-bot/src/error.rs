use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to build client: {0}")]
    ClientBuildError(String),
    #[error("client already started")]
    AlreadyStarted,
    #[error("discord error: {0}")]
    DiscordError(#[from] serenity::Error),

    #[error("songbird not initialised")]
    SongbirdNotInitialised,
    #[error("This command can only be used in a server.")]
    NotInGuild,
    #[error("Not in a voice channel to play in.")]
    NotInVoiceChannel,

    #[error("Already a song currently playing")]
    SongAlreadyPlaying,

    #[error("Quiz already started")]
    QuizAlreadyStarted,
    #[error("No current quiz in progress")]
    NoQuizRunning,
    #[error("Invalid quiz type: `{0}`")]
    InvalidQuizType(String),
    #[error("error setting song info: {0}")]
    SetSongInfo(song_artist::Error),
    #[error("No matching songs in database")]
    NoDatabaseSongs,
    #[error("Failed to get song from database: {0}")]
    GetSongErr(database::Error),
    #[error("No Catbox URL returned")]
    NoCatboxUrl,

    #[error("error decoding song: {0}")]
    DecodeSongError(stream_song::Error),
    #[error("failed to download stream for song: {0}")]
    StreamDownloadErr(stream_song::Error)
}
