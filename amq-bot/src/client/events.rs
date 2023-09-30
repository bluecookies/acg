use amq_types::{QuizSongInfo, PlaySongInfo, Message, SpectateGameData, AnswerResults};
use crate::{Client, Error};

mod events {
    pub static PLAYER_COUNT: &str = "online player count change";
    pub static QUIZ_OVERLAY_MSG: &str = "quiz overlay message";
    pub static QUIZ_SKIP_MSG: &str = "quiz skip message";
    pub static GAME_INVITE: &str = "game invite";
    pub static FRIEND_REQUEST: &str = "new friend request recived";
    pub static NEXT_VIDEO_INFO: &str = "quiz next video info";
    pub static PLAY_NEXT_SONG: &str = "play next song";
    pub static GAME_CHAT_UPDATE: &str = "game chat update";
    pub static GAME_STARTING: &str = "Game Starting";
    pub static SPECTATE_GAME: &str = "Spectate Game";
    pub static PLAYER_ANSWERS: &str = "player answers";
    pub static PLAYER_ANSWERED: &str = "player answered";
    pub static QUIZ_ANSWER: &str = "quiz answer";
    pub static ANSWER_RESULTS: &str = "answer results";
}


impl Client {
    // count
    pub fn on_player_count_change(&self, mut cb: impl FnMut(i64, &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::PLAYER_COUNT, move |data| {
            #[derive(serde::Deserialize)]
            struct PlayerCount {
                count: i64,
            }

            let data: PlayerCount = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb(data.count, &client);
            Ok(())
        });
    }

    pub fn on_quiz_overlay_msg(&self, mut cb: impl FnMut(String, &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::QUIZ_OVERLAY_MSG, move |data| {
            let data: String = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb(data, &client);
            Ok(())
        });
    }

    pub fn on_quiz_skip_msg(&self, mut cb: impl FnMut(String, &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::QUIZ_SKIP_MSG, move |data| {
            let data: String = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb(data, &client);
            Ok(())
        });
    }

    // gameId, sender
    pub fn on_game_invite(&self, mut cb: impl FnMut(i64, String, &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::GAME_INVITE, move |data| {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct GameInvite {
                game_id: i64,
                sender: String,
            }

            let data: GameInvite = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb(data.game_id, data.sender, &client);
            Ok(())
        });
    }

    // name
    pub fn on_friend_request(&self, mut cb: impl FnMut(String, &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::FRIEND_REQUEST, move |data| {
            #[derive(serde::Deserialize)]
            struct FriendRequest {
                name: String,
            }

            let data: FriendRequest = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb(data.name, &client);
            Ok(())
        });
    }

    pub fn on_next_video_info(&self, mut cb: impl FnMut(QuizSongInfo, &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::NEXT_VIDEO_INFO, move |data| {
            // TODO: check if client in game right now
            let data: QuizSongInfo = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            if let Some(ref info) = &data.video_info {
                client.video_ready(info.id)?;
            }
            cb(data, &client);
            Ok(())
        });
    }

    pub fn on_play_next_song(&self, mut cb: impl FnMut(PlaySongInfo, &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::PLAY_NEXT_SONG, move |data| {
            let data: PlaySongInfo = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb(data, &client);
            Ok(())
        });
    }

    pub fn on_chat_message(&self, mut cb: impl FnMut(Message, &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::GAME_CHAT_UPDATE, move |data| {
            #[derive(serde::Deserialize)]
            struct GameChatUpdate {
                // bubles: Vec<_>,
                messages: Vec<Message>,
            }
            let data: GameChatUpdate = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            for message in data.messages {
                cb(message, &client);
            }
            Ok(())
        });
    }

    pub fn on_game_starting(&self, mut cb: impl FnMut((), &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::GAME_STARTING, move |_data| {
            // let data = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb((), &client);
            Ok(())
        });
    }

    pub fn on_spectate_game(&self, mut cb: impl FnMut(SpectateGameData, &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::SPECTATE_GAME, move |data| {
            let data: SpectateGameData = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb(data, &client);
            Ok(())
        });
    }

    pub fn on_player_answers(&self, mut cb: impl FnMut((), &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::PLAYER_ANSWERS, move |_data| {
            // {"answers":[{"answer":"","gamePlayerId":3,"pose":6}],"progressBarState":null}
            // let data: () = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb((), &client);
            Ok(())
        });
    }

    pub fn on_player_answered(&self, mut cb: impl FnMut((), &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::PLAYER_ANSWERED, move |_data| {
            // [3] , [1,1,1], etc
            // let data: () = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb((), &client);
            Ok(())
        });
    }

    pub fn on_answered(&self, mut cb: impl FnMut((), &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::QUIZ_ANSWER, move |_data| {
            // {"answer":"pripara","success":true}
            // let data: () = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb((), &client);
            Ok(())
        });
    }

    pub fn on_answer_results(&self, mut cb: impl FnMut(AnswerResults, &Self) + Send + 'static) {
        let client = self.clone();
        self.set_command_handler(events::ANSWER_RESULTS, move |data| {
            let data: AnswerResults = serde_json::from_value(data).map_err(Error::DeserializeError)?;
            cb(data, &client);
            Ok(())
        });
    }
}