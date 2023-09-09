use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("song number out of range ({0})")]
    SongNumberOutOfRange(i64),
    #[error("setting existing song information: {0:?}")]
    DuplicateSongInfoReceived(crate::quiz::SongInfo),
    #[error("couldn't fetch song info: {0}")]
    FetchSongInfoError(database::Error),
}
