use bytes::Bytes;

use super::ClientError;

#[derive(Debug)]
pub struct Packet {
    pub packet_type: PacketId,
    pub nsp: String,
    pub data: PacketData,
    pub attachment_count: u8,
    pub id: Option<i32>,
}

impl Default for Packet {
    fn default() -> Self {
        Packet {
            packet_type: PacketId::Event,
            nsp: String::from("/"),
            data: PacketData::None,
            attachment_count: 0,
            id: None,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PacketId {
    Connect = 0,
    Disconnect = 1,
    Event = 2,
    Ack = 3,
    ConnectError = 4,
    BinaryEvent = 5,
    BinaryAck = 6,
}

impl TryFrom<char> for PacketId {
    type Error = ClientError;
    fn try_from(b: char) -> Result<Self, ClientError> {
        match b {
            '0' => Ok(PacketId::Connect),
            '1' => Ok(PacketId::Disconnect),
            '2' => Ok(PacketId::Event),
            '3' => Ok(PacketId::Ack),
            '4' => Ok(PacketId::ConnectError),
            '5' => Ok(PacketId::BinaryEvent),
            '6' => Ok(PacketId::BinaryAck),
            _ => Err(ClientError::InvalidPacketId(b)),
        }
    }
}


#[derive(Debug)]
pub enum PacketData {
    None,
    Json(serde_json::Value),
}

impl TryFrom<Bytes> for Packet {
    type Error = ClientError;
    fn try_from(value: Bytes) -> Result<Self, ClientError> {
        let mut packet = Packet::default();
        let packet_string = std::str::from_utf8(&value)?;
        let mut chars_iter = packet_string.chars();
        let mut utf8_iter = chars_iter.by_ref().peekable();
        let mut next_utf8;
        let mut char_buf: Vec<char> = vec![];

        // packet_type
        packet.packet_type = PacketId::try_from(utf8_iter.next().ok_or(ClientError::IncompletePacket)?)?;

        // attachment_count
        if let PacketId::BinaryAck | PacketId::BinaryEvent = packet.packet_type {
            loop {
                next_utf8 = utf8_iter.peek().ok_or(ClientError::IncompletePacket)?;
                if *next_utf8 == '-' {
                    let _ = utf8_iter.next(); // consume '-' char
                    break;
                }
                char_buf.push(utf8_iter.next().unwrap()); // SAFETY: already peeked
            }
        }
        let count_str: String = char_buf.iter().collect();
        if let Ok(count) = count_str.parse::<u8>() {
            packet.attachment_count = count;
        }

        char_buf.clear();
        next_utf8 = match utf8_iter.peek() {
            Some(c) => c,
            None => return Ok(packet),
        };

        if *next_utf8 == '/' {
            char_buf.push(utf8_iter.next().unwrap()); // SAFETY: already peeked
            loop {
                next_utf8 = utf8_iter.peek().ok_or(ClientError::IncompletePacket)?;
                if *next_utf8 == ',' {
                    let _ = utf8_iter.next(); // consume ','
                    break;
                }
                char_buf.push(utf8_iter.next().unwrap()); // SAFETY: already peeked
            }
        }
        if !char_buf.is_empty() {
            packet.nsp = char_buf.iter().collect();
        }

        // id
        char_buf.clear();
        next_utf8 = match utf8_iter.peek() {
            None => return Ok(packet),
            Some(c) => c,
        };

        loop {
            if !next_utf8.is_ascii_digit() {
                break;
            }
            char_buf.push(utf8_iter.next().unwrap()); // SAFETY: already peeked
            next_utf8 = match utf8_iter.peek() {
                None => return Ok(packet),
                Some(c) => c,
            };
        }

        let count_str: String = char_buf.iter().collect();
        if let Ok(count) = count_str.parse::<i32>() {
            packet.id = Some(count);
        }

        // data
        // sort of hacky workaround to get the rest of the string
        let json_str = {
            let last_char_len = next_utf8.len_utf8();
            let remainder = chars_iter.as_str();
            let length = remainder.len() + last_char_len;
            let start = packet_string.len() - length;
            &packet_string[start..]
        };
        let json_data: serde_json::Value = serde_json::from_str(json_str)?;
        packet.data = PacketData::Json(json_data);

        // TODO: binary attachments
        Ok(packet)
    }
}
