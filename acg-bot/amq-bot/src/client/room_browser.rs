use crate::{Client, Error};

impl Client {
    pub fn spectate_quiz(&self, game_id: i64, password: Option<&str>, game_invite: Option<bool>) -> Result<(), Error> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SpectateQuiz<'a> {
            game_id: i64,
            #[serde(skip_serializing_if = "Option::is_none")]
            password: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            game_invite: Option<bool>,
        }

        let data = SpectateQuiz {
            game_id,
            password,
            game_invite,
        };
        let data = serde_json::to_value(data).map_err(Error::SerializeError)?;

        self.send_command("roombrowser", "spectate game", data)?;
        Ok(())
    }
}

