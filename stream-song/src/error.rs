use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("http client error: {0}")]
    ClientError(stream_download::http::reqwest::Error),
    #[error("invalid url")]
    UrlParseError(url::ParseError),
    #[error("error fetching audio stream: {0}")]
    StreamDownloadErr(std::io::Error),

    #[error("failed to probe audio format: {0}")]
    ProbeFormatError(symphonia::core::errors::Error),
    #[error("failed to decode packet: {0}")]
    DecodeError(symphonia::core::errors::Error),
    #[error("failed to seek audio track: {0}")]
    SeekError(symphonia::core::errors::Error),
    #[error("failed to create resampler: {0}")]
    ResamplerError(#[from] rubato::ResamplerConstructionError),
    #[error("failed to resample: {0}")]
    ResampleError(#[from] rubato::ResampleError),

    #[error("audio write error: {0}")]
    AudioWriteError(std::io::Error),
    #[error("no audio track")]
    NoAudioTrack,
    #[error("no time base")]
    NoTimeBase,

    #[error("audio error: {0}")]
    AudioError(#[from] symphonia::core::errors::Error),
    #[error("audio stream error")]
    AudioStreamError,
}
