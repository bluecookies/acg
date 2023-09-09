use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("connection error: {0}")]
    ConnectionError(socketio::ClientError),
    #[error("error sending command: {0}")]
    SendCommandError(socketio::ClientError),
    #[error("invalid command object: {0}")]
    InvalidCommandArg(serde_json::Value),
    #[error("deserialisation error: {0}")]
    DeserializeError(serde_json::Error),
    #[error("serialisation error: {0}")]
    SerializeError(serde_json::Error),
    #[error("received list update result when none sent")]
    NoListUpdateSent,
}