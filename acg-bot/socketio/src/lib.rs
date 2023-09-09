mod error;
mod engineio;
mod packet;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde_json::Value;
pub use error::ClientError;
pub use engineio::{Client as EngineClient, ClientState};
use packet::{Packet, PacketId};
use crate::packet::PacketData;

type Callback = Box<dyn FnMut(serde_json::Value, Client) + Send + 'static>;

#[derive(Clone)]
pub struct Client {
    engine: EngineClient,
    on: Arc<Mutex<HashMap<Event, Callback>>>,
}


impl Client {
    pub fn new() -> Self {
        let engine = EngineClient::new();
        let client = Client {
            engine,
            on: Arc::new(Mutex::new(HashMap::new())),
        };
        client.engine.on_message({
            let client = client.clone();
            move |b| {
                let packet = Packet::try_from(b)?;
                match packet.packet_type {
                    PacketId::Connect => (),
                    PacketId::Event => match packet.data {
                        PacketData::Json(j) => {
                            match j {
                                Value::Array(ref v) => {
                                    if let Some(head) = v.first() {
                                        let e = Event::from_json(head).unwrap_or(Event::Any);
                                        client.on_event(e, j);
                                    } else {
                                        client.on_event(Event::Any, j);
                                    }
                                },
                                _ => client.on_event(Event::Any, j),
                            }
                        },
                        PacketData::None => client.on_event(Event::Message, Value::Null)
                    },
                    _ => { dbg!(&packet); },
                }
                Ok(())
            }
        });
        client
    }

    pub fn state(&self) -> ClientState {
        self.engine.state()
    }

    pub async fn wait(&self) {
        self.engine.wait().await;
    }

    pub async fn connect<S: AsRef<str> + Clone>(&self, url: S) -> Result<(), ClientError> {
        self.engine.connect(url, Some(String::from("socket.io"))).await?;

        Ok(())
    }

    pub fn on<E: Into<Event>>(&self, event: E, cb: impl FnMut(Value, Client) + Send + 'static) {
        let mut on = self.on.lock().expect("poisoned mutex");
        on.insert(event.into(), Box::new(cb));
    }

    pub fn emit(&self, event: &str, data: impl IntoIterator<Item=Value>) -> Result<(), ClientError> {
        // TODO: clean this up
        let mut v = vec![Value::String(event.to_string())];
        v.extend(data);
        let json_string = Value::Array(v).to_string();
        // 2 for EVENT
        let packet_string = format!("2{}", json_string);

        let bytes = bytes::Bytes::from(packet_string.into_bytes());
        let packet = engineio::Packet::Message(bytes);
        self.engine.send_packet(packet)?;
        Ok(())
    }

    fn on_event(&self, e: Event, j: Value) {
        let mut on = self.on.lock().expect("poisoned mutex");
        if let Event::Message | Event::Custom(_) = e {
            if let Some(cb) = on.get_mut(&Event::Any) {
                cb(j.clone(), self.clone());
            }
        }
        if let Some(cb) = on.get_mut(&e) {
            cb(j, self.clone());
        } else {
            if let Event::Custom(_) = e {
                if let Some(cb) = on.get_mut(&Event::AnyElse) {
                    cb(j, self.clone());
                }
            }
        }
    }
}

#[derive(Eq, Hash, PartialEq)]
pub enum Event {
    Message,
    Custom(String),
    Any,
    AnyElse,
}

impl Event {
    fn from_json(j: &Value) -> Option<Self> {
        if let Value::String(ref s) = j {
            Some(Event::Custom(s.to_string()))
        } else {
            None
        }
    }
}

impl From<&str> for Event {
    fn from(value: &str) -> Self {
        match value {
            "message" => Event::Message,
            "*" => Event::Any,
            _ => Event::Custom(value.to_string()),
        }
    }
}