use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use futures_util::{StreamExt, TryStreamExt};
use serde_json::Value as JsonValue;
use stream_throttle::{ThrottlePool, ThrottledStream};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;
use socketio::{Client as Socket, ClientState};

use amq_types::GameData;
use crate::error::Error;

mod events;
// commands
mod social;
mod room_browser;
mod lobby;
mod quiz;

pub use social::ListType;

pub trait Callback: FnMut(JsonValue) -> Result<(), Error> + Send + 'static {}
impl<T> Callback for T where T: FnMut(JsonValue) -> Result<(), Error> + Send + 'static {}

pub enum LoginCompleteCallback {
    Set(Box<dyn FnMut(&GameData, Client) + Send>),
    NotSet(bool),
}

#[derive(Clone)]
pub struct Client {
    // TODO: can use a socket.io 4.0 library now
    socket: Socket,
    session_id: Arc<AtomicI64>,
    command_handlers: Arc<Mutex<HashMap<String, Box<dyn Callback>>>>,
    fallback_command_handler: Arc<Mutex<Option<Box<dyn FnMut(&str, JsonValue) + Send>>>>,
    game_data: Arc<Mutex<Option<GameData>>>,
    on_login_complete: Arc<Mutex<LoginCompleteCallback>>,
    // list, username, setter
    list_update: Arc<Mutex<Option<(ListType, Option<String>, Option<String>)>>>,

    //
    chat_queue_tx: Sender<JsonValue>,

}

impl Client {
    pub fn new() -> Self {
        let socket = Socket::new();
        let session_id = Arc::new(AtomicI64::new(0));
        let command_handlers = Arc::new(Mutex::new(HashMap::new()));
        let fallback_command_handler = Arc::new(Mutex::new(None));
        let game_data = Arc::new(Mutex::new(None));
        let on_login_complete = Arc::new(Mutex::new(LoginCompleteCallback::NotSet(false)));
        let list_update = Arc::new(Mutex::new(None));
        let (tx, rx) = mpsc::channel(128);
        let client = Client {
            socket,
            session_id,
            command_handlers,
            fallback_command_handler,
            game_data,
            on_login_complete,
            list_update,
            chat_queue_tx: tx,
        };
        let session_id = client.session_id.clone();
        client.socket.on("sessionId", move |v, _s| {
            match get_arg(v).and_then(|v| v.as_i64().ok_or(v)) {
                Ok(sid) => session_id.store(sid, Ordering::Release),
                Err(v) => log::warn!("unexpected sessionId format: {}", v),
            }
        });
        let c = client.clone();
        client.socket.on("command", move |v, _s| {
            let arg = match get_arg(v) {
                Ok(arg) => arg,
                Err(v) => {
                    log::warn!("unexpected command format: {}", v);
                    return;
                },
            };

            if let Err(e) = c.handle_command(arg.clone()) {
                log::error!("error handling command: {} ({})", e, arg);
            }
        });

        // handle login data in the client
        let game_data = client.game_data.clone();
        let on_login_complete = client.on_login_complete.clone();
        let c = client.clone();
        client.set_command_handler("login complete", move |data| {
            let login_data = serde_json::from_value(data).map_err(Error::DeserializeError)?;

            let client = c.clone();
            let mut guard = on_login_complete.lock().expect("poisoned mutex");
            match &mut *guard {
                LoginCompleteCallback::Set(ref mut cb) => cb(&login_data, client),
                LoginCompleteCallback::NotSet(ref mut wait) => *wait = true,
            }

            let mut guard = game_data.lock().expect("poisoned mutex");
            *guard = Some(login_data);
            Ok(())
        });
        let c = client.clone();
        client.set_command_handler("malLastUpdate", move |data| {
            let mut guard = c.game_data.lock().expect("mutex poisoned");
            if let Some(game_data) = guard.as_mut() {
                game_data.mal_last_update = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            }
            Ok(())
        });
        let c = client.clone();
        client.set_command_handler("aniListLastUpdate", move |data| {
            let mut guard = c.game_data.lock().expect("mutex poisoned");
            if let Some(game_data) = guard.as_mut() {
                game_data.ani_list_last_update = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            }
            Ok(())
        });
        let c = client.clone();
        client.set_command_handler("kitsuLastUpdate", move |data| {
            let mut guard = c.game_data.lock().expect("mutex poisoned");
            if let Some(game_data) = guard.as_mut() {
                game_data.kitsu_last_update = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            }
            Ok(())
        });
        let c = client.clone();
        client.set_command_handler("anime list update result", move |data| {
            #[derive(serde::Deserialize)]
            struct Result {
                message: String,
                success: bool,
            }
            let result: Result = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            let (list_type, username, setter) = c.list_update.lock().expect("mutex poisoned")
                .take()
                .ok_or(Error::NoListUpdateSent)?;
            let mut guard = c.game_data.lock().expect("mutex poisoned");
            if let Some(game_data) = guard.as_mut() {
                if result.success {
                    match list_type {
                        ListType::Mal => game_data.mal_name = username,
                        ListType::Anilist => game_data.ani_list = username,
                        ListType::Kitsu => game_data.kitsu = username,
                    }
                }
                if let Some(setter) = setter {
                    c.send_chat_message(format!("@{} {}", setter, result.message));
                }
            }
            Ok(())
        });
        // TODO: also handle other changes in state

        // chat manager
        //  ege: 15 messages per second - do 100 ms just in case
        let msg_interval = Duration::from_millis(1000);
        let throttle_rate = stream_throttle::ThrottleRate::new(10, msg_interval);
        let c = client.clone();
        tokio::spawn(async move {
            let stream = ReceiverStream::new(rx);
            let pool = ThrottlePool::new(throttle_rate);
            if let Err(e) = stream.throttle(pool).map(|msg| c.raw_send_chat(msg)).try_for_each(|_| async { Ok(()) }).await {
                log::error!("chat manager errored: {}", e);
            }
        });

        client
    }

    pub async fn connect(&self, token: crate::Token) -> Result<(), Error> {
        const WS_URL: &str = "https://socket.animemusicquiz.com";
        let url = format!("{WS_URL}:{port}?token={token}", port = token.port, token = token.token);
        self.socket.connect(url).await.map_err(|e| Error::ConnectionError(e))
    }

    pub fn status(&self) -> ClientStatus {
        let sid = self.session_id.load(Ordering::Acquire);
        let socket_status = self.socket.state();
        let guard = self.game_data.lock().expect("poisoned mutex");
        let logged_in = guard.is_some();
        ClientStatus {
            sid,
            socket_status,
            logged_in,
        }
    }


    pub fn on_login_complete(&self, mut cb: impl FnMut(&GameData, Self) + Send + 'static) {
        let mut guard = self.on_login_complete.lock().expect("poisoned mutex");
        match &mut *guard {
            LoginCompleteCallback::Set(ref mut old) => *old = Box::new(cb),
            LoginCompleteCallback::NotSet(wait) => {
                if *wait {
                    let data_guard = self.game_data.lock().expect("poisoned mutex");
                    if let Some(data) = &*data_guard {
                        cb(data, self.clone());
                    } else {
                        log::error!("error: waiting for callback but login data not set");
                    }
                }
                *guard = LoginCompleteCallback::Set(Box::new(cb));
            }
        }
    }

    pub async fn wait(&self) {
        self.socket.wait().await
    }

    pub fn fallback_command(&self, handler: impl FnMut(&str, JsonValue) + Send + 'static) {
        let mut guard = self.fallback_command_handler.lock().expect("poisoned mutex");
        *guard = Some(Box::new(handler));
    }

    pub fn game_data(&self) -> Option<GameData> {
        let guard = self.game_data.lock().expect("mutex poisoned");
        guard.clone()
    }

    fn set_command_handler(&self, command: &str, handler: impl Callback) {
        let mut guard = self.command_handlers.lock().expect("poisoned mutex");
        guard.insert(command.to_string(), Box::new(handler));
    }

    fn handle_command(&self, mut object: JsonValue) -> Result<(), Error> {
        let command = if let Some(JsonValue::String(cmd)) = object.get_mut("command").map(JsonValue::take) {
            cmd
        } else {
            return Err(Error::InvalidCommandArg(object));
        };
        let data = object.get_mut("data").map(JsonValue::take).unwrap_or(JsonValue::Null);

        {
            let mut guard = self.command_handlers.lock().expect("poisoned mutex");
            if let Some(cb) = guard.get_mut(&command) {
                cb(data)?;
            } else {
                // fallback command handler
                let mut guard = self.fallback_command_handler.lock().expect("poisoned mutex");
                if let Some(ref mut cb) = &mut *guard {
                    cb(&command, data);
                }
            }
        }

        Ok(())
    }

    fn send_command(&self, kind: &str, command: &str, data: JsonValue) -> Result<(), Error> {
        let payload = if let JsonValue::Null = data {
            serde_json::json!({
                "type": kind,
                "command": command,
            })
        } else {
            serde_json::json!({
                "type": kind,
                "command": command,
                "data": data,
            })
        };
        self.socket.emit("command", [payload]).map_err(|e| Error::SendCommandError(e))
    }
}

fn get_arg(v: JsonValue) -> Result<JsonValue, JsonValue> {
    if let JsonValue::Array(v) = v {
        v.into_iter().nth(1).ok_or(JsonValue::Null)
    } else {
        Err(v)
    }
}

pub struct ClientStatus {
    pub sid: i64,
    pub socket_status: ClientState,
    pub logged_in: bool,
}