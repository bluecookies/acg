use serde_json::json;
use itertools::Itertools;
use crate::{Client, Error};


impl Client {
    pub fn start_game(&self) -> Result<(), Error> {
        self.send_command("lobby", "start game", serde_json::Value::Null)
    }

    pub fn change_to_player(&self) -> Result<(), Error> {
        self.send_command("lobby", "change to player", serde_json::Value::Null)
    }

    pub fn promote_host(&self, host: &str) -> Result<(), Error> {
        self.send_command("lobby", "promote host", json!({"playerName": host}))
    }

    pub fn set_ready(&self, ready: bool) -> Result<(), Error> {
        self.send_command("lobby", "set ready", json!({"ready": ready}))
    }

    pub fn leave_game(&self) -> Result<(), Error> {
        self.send_command("lobby", "leave game", serde_json::Value::Null)
    }

    // TODO: split message on space, check actual max size
    // TODO: see if this actually correct - implement some tests to check failure/success matches
    pub fn send_chat_message<S: AsRef<str>>(&self, message: S) {
        let message = message.as_ref();
        if message.chars().count() > 150 {
            let chunks = message
                .chars()
                .chunks(150);
            for s in chunks.into_iter().map(|c| c.collect::<String>()) {
                if let Err(e) = self.chat_queue_tx.try_send(json!({"msg": s, "teamMessage": false})) {
                    log::error!("failed to send chat: {}", e);
                    return;
                }
            }
        } else {
            if let Err(e) = self.chat_queue_tx.try_send(json!({"msg": message, "teamMessage": false})) {
                log::error!("failed to send chat: {}", e);
            }
        }
    }

    pub(crate) fn raw_send_chat(&self, msg: serde_json::Value) -> Result<(), Error> {
        self.send_command("lobby", "game chat message", msg)?;
        Ok(())
    }
}