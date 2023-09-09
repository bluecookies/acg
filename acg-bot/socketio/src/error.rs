use std::num::ParseIntError;
use std::str::Utf8Error;
use thiserror::Error;
use tokio::sync::mpsc::error::TrySendError;
use tokio_tungstenite::tungstenite::{Message, Error as WsError};
use crate::engineio::Packet;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ClientError {
    #[error("Client is not in a disconnected state")]
    NotDisconnected,
    #[error("Invalid url: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Failed to connect to server: {}(code: {1})", if let Some(v) = .0 { v.to_string() } else { String::new() } )]
    ConnectionError(Option<serde_json::Value>, reqwest::StatusCode),
    #[error("Error response from server: {}(code: {1})", if let Some(v) = .0 { v.to_string() } else { String::new() } )]
    ResponseError(Option<serde_json::Value>, reqwest::StatusCode),
    #[error("Handshake failed")]
    HandshakeError,
    #[error("Incomplete packet")]
    IncompletePacket,
    #[error("Failed to decode base64: {0}")]
    Base64Error(#[from] base64::DecodeError),
    #[error("Invalid packet id: {0}")]
    InvalidPacketId(char),
    #[error("Failed to decode utf-8: {0}")]
    InvalidUtf8(#[from] Utf8Error),
    #[error("Failed to decode integer: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("Failed to set URL scheme")]
    UrlSchemeError,
    #[error("Failed to set URL path")]
    UrlPathError,
    #[error("Failed to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Received unexpected packet: {0:?}")]
    UnexpectedPacket(Packet),
    #[error("No base url")]
    NoBaseUrl,
    #[error("Pong not received")]
    PongNotReceived,
    #[error("Failed to send packet: {0:?}")]
    SendPacketError(#[from] TrySendError<Packet>),
    #[error("Unexpected websocket message type: {0:?}")]
    InvalidWebsocketPacket(Message),
    #[error("Websocket error: {0}")]
    WebsocketError(#[from] WsError),
    #[error("Websocket closed")]
    WebsocketClosed,
    #[error("Websocket upgrade failed")]
    WebsocketUpgradeFailed,
    #[error("Failed to write to websocket: {0}")]
    WebsocketSinkError(WsError),
}
