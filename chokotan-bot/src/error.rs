use poise::serenity_prelude as serenity;
use serenity::{ChannelId, GuildId};
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to build client: {0}")]
    ClientBuildError(String),
    #[error("client already started")]
    AlreadyStarted,
    #[error("failed to register slash commands: {0}")]
    RegisterCommands(serenity::Error),
    #[error("discord error: {0}")]
    DiscordError(#[from] serenity::Error),
    #[error("failed to get field for server {0}")]
    GuildFieldError(GuildId),
    #[error("failed to get channel from id {0}: {1}")]
    GetChannelErr(ChannelId, serenity::Error),
    #[error("failed to get members for channel id {0}: {1}")]
    GetMembersErr(ChannelId, serenity::Error),
    #[error("error leaving call: {0}")]
    LeaveCallErr(songbird::error::JoinError),

    #[error("songbird not initialised")]
    SongbirdNotInitialised,
    #[error("This command can only be used in a server.")]
    NotInGuild,
    #[error("Not in a voice channel to play in.")]
    NotInVoiceChannel,
    #[error("failed to join channel: {0}")]
    VoiceJoinError(#[from] songbird::error::JoinError),

    #[error("Already a song currently playing")]
    SongAlreadyPlaying,
    #[error("Track error: {0}")]
    TrackError(#[from] songbird::error::TrackError),

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
    #[error("error in quiz task: {0}")]
    QuizTaskErr(tokio::task::JoinError),
    #[error("error loading quiz configs: {0}")]
    QuizLoadConfigErr(#[from] crate::quiz::LoadConfigError),
    #[error("quiz parameter error: {0}")]
    QuizParamErr(#[from] crate::quiz::ParamError),

    #[error("error decoding song: {0}")]
    DecodeSongError(stream_song::Error),
    #[error("failed to download stream for song: {0}")]
    StreamDownloadErr(stream_song::Error),
}
