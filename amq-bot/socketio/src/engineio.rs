use std::fmt;
use std::fmt::Formatter;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use futures::{Sink, SinkExt, Stream, StreamExt};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use url::Url;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

mod transport;
mod packet;

use super::error::ClientError;
use transport::TransportType;
pub use packet::{Packet, Payload};

const ENGINE_IO_VER: &str = "3";

pub trait Callback: FnMut(bytes::Bytes) -> Result<(), ClientError> + Send + 'static {}
impl<T> Callback for T where T: FnMut(bytes::Bytes) -> Result<(), ClientError> + Send + 'static {}
type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Clone)]
pub struct Client {
    inner: Arc<Mutex<ClientInner>>,
    http: reqwest::Client,
    on_message_handler: Arc<Mutex<Option<Box<dyn Callback>>>>,
    pong_received: Arc<AtomicBool>,
    packet_queue: Arc<RwLock<Sender<Packet>>>,
}

#[derive(Copy, Clone)]
pub enum ClientState {
    NotConnected,
    Connecting,
    Connected,
}

impl fmt::Display for ClientState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConnected => write!(f, "Not connected"),
            Self::Connecting => write!(f, "Connecting"),
            Self::Connected => write!(f, "Connected"),
        }
    }
}

// TODO: drop logic
struct ClientInner {
    state: ClientState,
    base_url: Option<Url>,
    handshake: Handshake,
    disconnect_token: CancellationToken,
    task_handle: Option<JoinHandle<Result<(), ClientError>>>,
}

#[derive(serde::Deserialize, Default, Clone, Debug)]
struct Handshake {
    pub sid: String,
    pub upgrades: Vec<String>,
    #[serde(rename = "pingInterval")]
    pub ping_interval: u64,
    #[serde(rename = "pingTimeout")]
    pub ping_timeout: u64,
}

const MAX_PACKETS: usize = 64;

impl Client {
    pub fn new() -> Self {
        let (_tx, _rx) = tokio::sync::mpsc::channel(MAX_PACKETS);
        Client {
            inner: Arc::new(Mutex::new(ClientInner {
                state: ClientState::NotConnected,
                base_url: None,
                handshake: Handshake::default(),
                disconnect_token: CancellationToken::new(),
                task_handle: None,
            })),
            http: reqwest::Client::new(),
            on_message_handler: Arc::new(Mutex::new(None)),
            pong_received: Arc::new(AtomicBool::new(false)),
            packet_queue: Arc::new(RwLock::new(_tx))
        }
    }

    pub fn state(&self) -> ClientState {
        let guard = self.inner.lock().expect("poisoned mutex");
        guard.state
    }

    pub async fn wait(&self) {
        let token = {
            let guard = self.inner.lock().expect("poisoned mutex");
            if let ClientState::Connected = guard.state {
                guard.disconnect_token.clone()
            } else {
                return;
            }
        };
        token.cancelled().await;
    }

    // TODO: set state to not connected on disconnect and also handle reconnection
    pub async fn connect<S: AsRef<str> + Clone>(&self, url: S, engineio_path: Option<String>) -> Result<(), ClientError> {
        // check if connected and set state to connecting
        let packet_queue = {
            let mut guard = self.inner.lock().expect("poisoned mutex");
            if let ClientState::NotConnected = guard.state {} else {
                return Err(ClientError::NotDisconnected);
            }
            guard.state = ClientState::Connecting;

            let (tx, rx) = tokio::sync::mpsc::channel(MAX_PACKETS);
            let mut guard = self.packet_queue.write().expect("poisoned lock");
            *guard = tx;
            rx
        };

        // start connecting (polling)
        let base_url = get_engineio_url(url.clone(), engineio_path.clone(), TransportType::Polling)?;
        let (handshake, packets) = {
            let mut url = base_url.clone();
            url.query_pairs_mut().append_pair("t", &timestamp());
            let response = self.http.get(url).send().await?;
            let status_code = response.status();
            if !status_code.is_success() {
                let json = response.json().await.ok();
                return Err(ClientError::ConnectionError(json, status_code));
            }

            // read the handshake
            let content = response.bytes().await?;
            let payload = Payload::try_from(content)?;
            let mut packets = payload.into_iter();
            let handshake_packet = packets.next();
            let handshake = if let Some(Packet::Open(data)) = handshake_packet {
                let value: Handshake = serde_json::from_slice(&data)?;
                value
            } else {
                return Err(ClientError::HandshakeError);
            };
            (handshake, packets)
        };

        // set state to connected
        let disconnect_token = CancellationToken::new();
        {
            let mut guard = self.inner.lock().expect("poisoned mutex");
            guard.state = ClientState::Connected;
            guard.base_url = Some(base_url);
            guard.handshake = handshake.clone();
            guard.disconnect_token = disconnect_token.clone();
        }
        //
        // self.trigger_event(EngineEvent::Connect);

        // handle the rest of the packets
        for packet in packets {
            self.handle_packet(packet)?;
        }

        // upgrade to websocket
        if let Some(_) = handshake.upgrades.iter().find(|x| *x == "websocket") {
            log::trace!("attempting upgrade to websocket");
            let mut websocket_url = get_engineio_url(url, engineio_path, TransportType::Websocket)?;
            websocket_url.query_pairs_mut().append_pair("sid", &handshake.sid);
            match self.upgrade_websocket(websocket_url).await {
                Ok(ws) => {
                    let (write, read) = ws.split();
                    let write = write.sink_map_err(|e| ClientError::WebsocketSinkError(e));
                    // set up background read/write tasks
                    let read_loop = self.clone().create_read_loop_websocket(read, disconnect_token.clone());
                    let write_loop = self.clone().create_write_loop_websocket(packet_queue, write, disconnect_token.clone());
                    let ping_loop = self.clone().create_ping_loop(disconnect_token.clone());

                    let task_handle = tokio::spawn(async move {
                        if let Err(e) = tokio::try_join!(read_loop, write_loop, ping_loop) {
                            log::error!("tasks closed with error: {}", e);
                            disconnect_token.cancel();
                            Err(e)
                        } else {
                            Ok(())
                        }
                    });
                    {
                        let mut guard = self.inner.lock().expect("poisoned mutex");
                        guard.task_handle = Some(task_handle);
                    }

                    return Ok(());
                },
                Err(e) => {
                    log::warn!("could not upgrade to websocket: {}", e);
                }
            }
        }

        // set up background read/write tasks
        let read_loop = self.clone().create_read_loop_polling(disconnect_token.clone());
        let write_loop = self.clone().create_write_loop_polling(packet_queue, disconnect_token.clone());
        let ping_loop = self.clone().create_ping_loop(disconnect_token.clone());

        let task_handle = tokio::spawn(async move {
            if let Err(e) = tokio::try_join!(read_loop, write_loop, ping_loop) {
                log::error!("tasks closed with error: {}", e);
                disconnect_token.cancel();
                Err(e)
            } else {
                Ok(())
            }
        });
        {
            let mut guard = self.inner.lock().expect("poisoned mutex");
            guard.task_handle = Some(task_handle);
        }

        Ok(())
    }

    pub fn on_message(&self, cb: impl Callback) {
        let mut guard = self.on_message_handler.lock().expect("poisoned mutex");
        *guard = Some(Box::new(cb));
    }

    fn handle_packet(&self, packet: Packet) -> Result<(), ClientError> {
        match packet {
            Packet::Message(b) | Packet::MessageBinary(b) => self.call_on_message(b)?,
            Packet::Pong(_) => self.pong_received.store(true, Ordering::Release),
            Packet::Close(_) => self.disconnect(),
            p => return Err(ClientError::UnexpectedPacket(p)),
        }
        Ok(())
    }

    pub fn send_packet(&self, packet: Packet) -> Result<(), ClientError> {
        let read = self.packet_queue.read().expect("poisoned lock");
        read.try_send(packet)?;
        Ok(())
    }

    fn call_on_message(&self, data: bytes::Bytes) -> Result<(), ClientError> {
        let mut guard = self.on_message_handler.lock().expect("poisoned mutex");
        if let Some(cb) = &mut *guard {
            cb(data)?;
        }
        Ok(())
    }

    fn disconnect(&self) {
        let guard = self.inner.lock().expect("poisoned mutex");
        guard.disconnect_token.cancel();
    }

    fn get_request_info(&self) -> Result<(Url, Duration), ClientError> {
        let (base_url, handshake) = {
            let (url, handshake) = {
                let guard = self.inner.lock().expect("poisoned mutex");
                let handshake = guard.handshake.clone();
                let url = guard.base_url.clone();
                (url, handshake)
            };
            let mut url = url.ok_or(ClientError::NoBaseUrl)?;
            url.query_pairs_mut()
                .append_pair("sid", &handshake.sid);
            (url, handshake)
        };
        let timeout_millis = std::cmp::max(handshake.ping_interval, handshake.ping_timeout) + 5000;
        let timeout = Duration::from_millis(timeout_millis);
        Ok((base_url, timeout))
    }

    async fn create_read_loop_polling(self, disconnect_token: CancellationToken) -> Result<(), ClientError> {
        let (base_url, timeout) = self.get_request_info()?;

        loop {
            let mut url = base_url.clone();
            url.query_pairs_mut()
                .append_pair("t", &timestamp());
            let response = tokio::select! {
                _ = disconnect_token.cancelled() => break,
                response = self.http.get(url).timeout(timeout).send() => response?,
            };

            let status_code = response.status();
            if !status_code.is_success() {
                let json = response.json().await.ok();
                return Err(ClientError::ResponseError(json, status_code));
            }

            let content = response.bytes().await?;
            let payload = Payload::try_from(content)?;
            for packet in payload.into_iter() {
                self.handle_packet(packet)?;
            }
        }
        Ok(())
    }

    async fn create_write_loop_polling(
        self,
        mut packet_queue: Receiver<Packet>,
        disconnect_token: CancellationToken
    ) -> Result<(), ClientError>
    {
        let (url, timeout) = self.get_request_info()?;
        loop {
            let packet = tokio::select! {
                _ = disconnect_token.cancelled() => break,
                r = packet_queue.recv() => if let Some(p) = r { p } else { break },
            };
            let mut payload = Payload::new(packet);
            if let Ok(p) = packet_queue.try_recv() {
                payload.push(p);
                while let Ok(p) = packet_queue.try_recv() {
                    payload.push(p);
                }
            }
            // write a payload of 1 packet - TODO: could batch
            let response = self.http
                .post(url.clone())
                .body(payload.to_bytes())
                .timeout(timeout)
                .send()
                .await?;

            let status_code = response.status();
            if !status_code.is_success() {
                let json = response.json().await.ok();
                return Err(ClientError::ResponseError(json, status_code));
            }
        }
        Ok(())
    }

    async fn create_ping_loop(self, disconnect_token: CancellationToken) -> Result<(), ClientError> {
        let ping_interval = {
            let guard = self.inner.lock().expect("poisoned mutex");
            let millis = guard.handshake.ping_interval;
            std::time::Duration::from_millis(millis)
        };
        let mut interval = tokio::time::interval(ping_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        self.pong_received.store(true, Ordering::Release);
        loop {
            tokio::select! {
                _ = disconnect_token.cancelled() => break,
                _ = interval.tick() => (),
            }
            if !self.pong_received.swap(false, Ordering::AcqRel) {
                return Err(ClientError::PongNotReceived);
            }
            self.send_packet(Packet::ping(""))?;
        }
        Ok(())
    }

    async fn upgrade_websocket(&self, mut url: Url) -> Result<WsStream, ClientError> {
        // TODO: preserve cookies
        url.query_pairs_mut()
            .append_pair("t", &timestamp());
        let (mut ws, _response) = tokio_tungstenite::connect_async(url).await?;
        // dbg!(_response);

        // send the probe
        let probe = Packet::ping("probe");
        ws.send(probe.to_ws_message()).await?;

        let response = ws.next().await.ok_or(ClientError::WebsocketClosed)??;
        let probe_response = Packet::from_ws_message(response)?;
        let mut success = false;
        if let Packet::Pong(s) = probe_response {
            if &*s == &*b"probe" {
                ws.send(Packet::upgrade().to_ws_message()).await?;
                // successfully upgraded
                success = true;
            }
        }
        if success {
            Ok(ws)
        } else {
            Err(ClientError::WebsocketUpgradeFailed)
        }
    }

    async fn create_read_loop_websocket(
        self,
        mut read: impl Stream<Item=Result<Message, WsError>> + Unpin,
        disconnect_token: CancellationToken
    ) -> Result<(), ClientError> {
        loop {
            tokio::select! {
                _ = disconnect_token.cancelled() => break,
                msg = read.next() => if let Some(m) = msg {
                    if let Ok(packet) = Packet::from_ws_message(m?) {
                        self.handle_packet(packet)?;
                    }
                } else {
                    return Err(ClientError::WebsocketClosed);
                },
            };
        }
        Ok(())
    }

    async fn create_write_loop_websocket(
        self,
        packet_queue: Receiver<Packet>,
        write: impl Sink<Message, Error = ClientError>,
        disconnect_token: CancellationToken
    ) -> Result<(), ClientError>
    {
        let forward = ReceiverStream::new(packet_queue)
            .map(|packet| Ok::<_, _>(packet.to_ws_message()))
            .forward(write);
        let _ = tokio::select! {
            _ = disconnect_token.cancelled() => (),
            res = forward => res?,
        };
        Ok(())
    }
}

fn get_engineio_url<S: AsRef<str>>(
    url_string: S,
    engineio_path: Option<String>,
    transport: TransportType,
) -> Result<Url, ClientError> {
    let mut url = Url::parse(url_string.as_ref())?;
    let base_scheme = url.scheme();
    let secure = base_scheme == "https" || base_scheme == "wss";
    url.set_scheme(transport.scheme(secure)).map_err(|_| ClientError::UrlSchemeError)?;
    {
        // this should never happen because the scheme is either http or ws
        let mut segments = url.path_segments_mut().map_err(|_| ClientError::UrlPathError)?;
        segments.clear();
        segments.push(engineio_path.as_deref().unwrap_or("engine.io"));
        segments.push("/");
    }
    url.query_pairs_mut()
        .append_pair("transport", transport.transport())
        .append_pair("EIO", ENGINE_IO_VER);
    Ok(url)
}

// gets timestamp string as milliseconds since epoch
//  js implementation uses yeast, but as long as this is unique it's fine
fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let start = SystemTime::now();
    let dur = start
        .duration_since(UNIX_EPOCH)
        .expect("time before epoch");
    let timestamp = dur.as_millis();
    timestamp.to_string()
}
