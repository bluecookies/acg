use serde_json::json;
use crate::{Client, Error};

impl Client {
    pub fn video_ready(&self, song_id: i64) -> Result<(), Error> {
        self.send_command("quiz", "video ready", json!({"songId": song_id}))
    }

    pub fn submit_answer<S: serde::Serialize>(&self, answer: S) -> Result<(), Error> {
        self.send_command("quiz", "quiz answer", json!({
            "answer": answer,
            "isPlaying": true,
            "volumeAtMax": false,
        }))
    }

    pub fn vote_skip(&self) -> Result<(), Error> {
        self.send_command("quiz", "skip vote", json!({"skipVote": true}))
    }
}