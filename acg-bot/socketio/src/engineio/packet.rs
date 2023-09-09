use bytes::Bytes;
use smallvec::{IntoIter, SmallVec};
use base64::{Engine, prelude::BASE64_STANDARD};
use tokio_tungstenite::tungstenite::Message as WsMessage;

use super::ClientError;

#[derive(Debug)]
pub enum Packet {
    Open(Bytes),
    Close(Bytes),
    Ping(Bytes),
    Pong(Bytes),
    Message(Bytes),
    MessageBinary(Bytes),
    Upgrade(Bytes),
    Noop(Bytes),
}

impl Packet {
    pub fn ping(d: impl Into<Bytes>) -> Self {
        Packet::Ping(d.into())
    }

    pub fn upgrade() -> Self {
        Packet::Upgrade(Bytes::new())
    }

    fn to_vec(&self) -> Vec<u8> {
        let mut result = Vec::new();
        match self {
            Packet::Open(b)           => { result.push(b'0'); result.extend(b)},
            Packet::Close(b)          => { result.push(b'1'); result.extend(b)},
            Packet::Ping(b)           => { result.push(b'2'); result.extend(b)},
            Packet::Pong(b)           => { result.push(b'3'); result.extend(b)},
            Packet::Message(b)        => { result.push(b'4'); result.extend(b)},
            Packet::MessageBinary(b)  => { result.push(b'b'); result.extend(BASE64_STANDARD.encode(b).into_bytes())},
            Packet::Upgrade(b)        => { result.push(b'5'); result.extend(b)},
            Packet::Noop(b)           => { result.push(b'6'); result.extend(b)},
        }
        result
    }

    pub fn to_ws_message(&self) -> WsMessage {
        let v = self.to_vec();
        match String::from_utf8(v) {
            Ok(s) => WsMessage::text(s),
            Err(e) => WsMessage::binary(e.into_bytes())
        }
    }

    pub fn from_ws_message(msg: WsMessage) -> Result<Self, ClientError> {
        match msg {
            WsMessage::Text(s) => {
                let b = Bytes::from(s.into_bytes());
                Ok(Packet::try_from(b)?)
            },
            WsMessage::Binary(b) => Ok(Packet::try_from(Bytes::from(b))?),
            e => Err(ClientError::InvalidWebsocketPacket(e))
        }
    }
}

impl TryFrom<Bytes> for Packet {
    type Error = ClientError;

    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        if bytes.is_empty() {
            return Err(ClientError::IncompletePacket);
        }

        let packet_id = *bytes.first().ok_or(ClientError::IncompletePacket)?;
        let data = bytes.slice(1..);

        // TODO: not matching all binary
        let packet = match packet_id {
            0 | b'0' => Packet::Open(data),
            1 | b'1' => Packet::Close(data),
            2 | b'2' => Packet::Ping(data),
            3 | b'3' => Packet::Pong(data),
            4 | b'4' => Packet::Message(data),
            5 | b'5' => Packet::Upgrade(data),
            6 | b'6' => Packet::Noop(data),
            b'b' => {
                let data = Bytes::from(BASE64_STANDARD.decode(data.as_ref())?);
                Packet::MessageBinary(data)
            },
            _ => return Err(ClientError::InvalidPacketId(packet_id as char)),
        };

        Ok(packet)
    }
}


// payload for long polling packets
pub struct Payload {
    inner: SmallVec::<[Packet; 2]>,
}

impl Payload {
    pub fn new(packet: Packet) -> Self {
        let mut s = Self {
            inner: SmallVec::new()
        };
        s.inner.push(packet);
        s
    }

    pub fn push(&mut self, packet: Packet) {
        self.inner.push(packet);
    }

    // TODO: handle binary data
    pub fn to_bytes(&self) -> Vec<u8> {
        use std::fmt::Write;
        let mut s = String::new();
        for packet in self.inner.iter() {
            let packet_str = String::from_utf8(packet.to_vec()).expect("TODO: handle binary packets");
            let packet_len = packet_str.chars().count();
            write!(s, "{}:{}", packet_len, packet_str).expect("failed to write string");
        }
        s.into_bytes()
    }
}

impl IntoIterator for Payload {
    type Item = Packet;
    type IntoIter = IntoIter<[Packet; 2]>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl TryFrom<Bytes> for Payload {
    type Error = ClientError;

    fn try_from(payload: Bytes) -> Result<Self, Self::Error> {
        let mut vec = SmallVec::<[Packet; 2]>::new();

        // jsonp not supported?
        if payload.is_empty() {
            return Ok(Payload { inner: vec });
        }
        if *payload.first().expect("empty payload") <= 1 {
            // binary data
            let mut data: Bytes = payload.slice(1..);
            while !data.is_empty() {
                // read the length
                let mut packet_length = 0_usize;
                let mut iter = data.into_iter();
                while let Some(byte) = iter.next() {
                    if byte == 0xFF {
                        break;
                    }
                    packet_length = packet_length * 10 + byte as usize;
                }
                data = iter.into_inner();
                if data.len() < packet_length {
                    return Err(ClientError::IncompletePacket);
                }
                let remaining = data.split_off(packet_length);
                vec.push(Packet::try_from(data)?);
                data = remaining;
            }
        } else {
            // text data
            let mut payload_string = std::str::from_utf8(&payload)?;
            // offset of packet start in bytes from beginning of payload data
            let mut packet_start = 0;
            while !payload_string.is_empty() {
                if let Some((a, b)) = payload_string.split_once(':') {
                    // length in codepoints, not bytes
                    let packet_length_chars = a.parse::<usize>()?;
                    packet_start += a.len() + 1;
                    let mut chars = b.chars();
                    for _ in 0..packet_length_chars {
                        if chars.next().is_none() {
                            return Err(ClientError::IncompletePacket);
                        }
                    }
                    let remaining = chars.as_str();
                    let packet_len_bytes = b.len() - remaining.len();
                    let packet_end = packet_start + packet_len_bytes;
                    vec.push(Packet::try_from(payload.slice(packet_start..packet_end))?);
                    packet_start = packet_end;
                    payload_string = remaining;
                } else {
                    return Err(ClientError::IncompletePacket);
                }
            }
        }

        Ok(Payload { inner: vec })
    }
}
