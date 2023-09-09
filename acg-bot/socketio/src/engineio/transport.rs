#[derive(Copy, Clone)]
pub enum TransportType {
    Polling,
    Websocket,
}

impl TransportType {
    pub fn transport(&self) -> &'static str {
        match self {
            TransportType::Polling => "polling",
            TransportType::Websocket => "websocket",
        }
    }

    pub fn scheme(&self, secure: bool) -> &'static str {
        match self {
            TransportType::Polling => if secure { "https" } else { "http" },
            TransportType::Websocket => if secure { "wss" } else { "ws" },
        }
    }
}

