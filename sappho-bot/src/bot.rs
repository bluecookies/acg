use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use amq_bot::Client;

mod commands;
use commands::Command;

use crate::Error;
use crate::quiz::{SongArtistQuiz, SongInfo, Guess};

// name used in database for s/a user
static BOT_DB_NAME: &str = "sa";

#[derive(Clone)]
pub struct Bot {
    client: Client,
    quiz: SongArtistQuiz,
    commands: Arc<Mutex<HashMap<String, Command>>>,
}

impl Bot {
    pub fn new(db: database::Database) -> Self {
        let client = Client::new();
        let quiz = SongArtistQuiz::new();
        let bot = Bot {
            client,
            quiz,
            commands: Arc::new(Mutex::new(HashMap::new())),
        };

        let client = bot.client.clone();
        // ignore
        client.on_player_count_change(|_count, _c| {});
        client.on_quiz_overlay_msg(|_msg, _c| {});
        client.on_quiz_skip_msg(|_msg, _c| {});
        client.on_player_answers(|_, _c| {});
        client.on_player_answered(|_, _c| {});
        client.on_answered(|_, _c| {});
        // accept all friend requests
        client.on_friend_request(|sender, c| {
            c.answer_friend_request(&sender, true).unwrap_or_else(
                |e| log::error!("failed to answer friend request: {}", e)
            )
        });
        // accept all game invites
        client.on_game_invite(|game_id, _sender, c| {
            c.leave_game()
                .and_then(|_| c.spectate_quiz(game_id, None, Some(true)))
                .unwrap_or_else(
                    |e| log::error!("failed to accept game invite: {}", e)
                )
        });

        // s/a stuff
        let quiz = bot.quiz.clone();

        client.on_game_starting({
            let quiz = quiz.clone();
            move |_, _c| quiz.clear()
        });
        client.on_spectate_game({
            let quiz = quiz.clone();
            let db = db.clone();
            move |data, c| {
                let quiz = quiz.clone();
                let db = db.clone();
                let c = c.clone();
                tokio::spawn(async move {
                    let data = if let Some(state) = data.quiz_state { state } else { return };
                    let song_num = data.song_number;
                    let song_timer = data.song_timer;
                    quiz.set_song_timer(song_timer);
                    if let Err(e) = quiz.set_song_info(song_num, &db, data.song_info).await {
                        log::error!("failed to set song info for song {}: {}", song_num, e);
                    }
                    if let Err(e) = quiz.set_song_info(song_num + 1, &db, data.next_video_info).await {
                        log::error!("failed to set song info for song {}: {}", song_num, e);
                    }
                    set_song_number(&c, song_num, &quiz);
                });
            }
        });
        client.on_next_video_info({
            let quiz = quiz.clone();
            let db = db.clone();
            move |data, _c| {
                let quiz = quiz.clone();
                let db = db.clone();
                tokio::spawn(async move {
                    let next_song_num = quiz.curr_song_number() + 1;
                    if let Err(e) = quiz.set_song_info(next_song_num, &db, data).await {
                        log::error!("failed to set song info for song {}: {}", next_song_num, e);
                    }
                });
            }
        });
        client.on_play_next_song({
            let quiz = quiz.clone();
            move |data, c| {
                // for now just ignore most of it and set the song number and time
                quiz.set_song_timer(0.0);
                set_song_number(c, data.song_number, &quiz);
            }
        });
        client.on_chat_message({
            let quiz = quiz.clone();
            let bot = bot.clone();
            move |msg, c| {
                // ignore own messages
                if c.self_name().map(|s| &s == &msg.sender).unwrap_or(true) {
                    return;
                }
                let content = msg.message;
                // handle commands
                if let Some(text) = content.strip_prefix('/') {
                    let mut it = text.split(' ');
                    if let Some(cmd) = it.next() {
                        if let Err(e) = bot.handle_command(cmd, it, &msg.sender) {
                            log::error!("error while handling user command ({}): {}", cmd, e);
                        }
                    }
                }
                // handle guess - TODO: only do if s/a mode is turned on
                let time = quiz.curr_song_timer() as f32;
                if let Some(guess_result) = quiz.handle_guess(&content, time) {
                    if let Some(guess) = guess_result.song_guess {
                        match guess {
                            Guess::Incorrect(s, p) => c.send_chat_message(format!("S❌: {} ({:.2}%)", s, p * 100.0)),
                            Guess::Correct(s, t) => c.send_chat_message(format!("S✅: {} ({:.1}s)", s, t)),
                        }
                    }
                    for guess in guess_result.artist_guesses {
                        match guess {
                            Guess::Incorrect(s, p) => c.send_chat_message(format!("A❌: {} ({}/{}) ({:.2}%)", s, guess_result.num_correct_artists, guess_result.total_artists, p * 100.0)),
                            Guess::Correct(s, t) => c.send_chat_message(format!("A✅: {} ({}/{}) ({:.1}s)", s, guess_result.num_correct_artists, guess_result.total_artists, t)),
                        }
                    }
                    //
                    if quiz.correct() {
                        // TODO: check if bot is playing
                        // TODO: submit the right answer
                        if let Err(e) = c.submit_answer("pripara").and_then(|_| c.vote_skip()) {
                            log::error!("error submitting answer: {}", e);
                        }
                    }
                }
            }
        });
        client.on_answer_results({
            let quiz = quiz.clone();
            let db = db.clone();
            move |data, c| {
                // TODO: check if s/a actually on
                let correct = Some(quiz.correct());
                let song_data = database::SongData {
                    correct,
                    player_name: Some(BOT_DB_NAME.into()),
                    video_length: None,
                    song_info: data.song_info,
                };
                let c = c.clone();
                let db = db.clone();
                // TODO: change amq-bot to be async so don't need tokio (and remove all)
                tokio::spawn(async move {
                    if let Err(e) = db.update_song_data(song_data).await {
                        log::error!("Error saving song info to database: {}", e);
                        c.send_chat_message("Error saving song info to database");
                    }
                });
            }
        });

        // log everything else - TODO: log this to web dashboard
        client.fallback_command(|cmd, v| println!("{}: {}", cmd, v));

        bot.register_commands();

        bot
    }

    pub async fn connect(&self, cookie: Option<String>) -> Result<Option<String>, Error> {
        // TODO: pass in either cookie or username + password
        let (token, cookie) = amq_bot::get_amq_token(cookie).await?;
        self.client.connect(token).await?;
        Ok(cookie)
    }

    pub fn status(&self) -> String {
        let status = self.client.status();
        format!(
            "Session ID: {}\n\
            Socket.IO: {}\n\
            Logged in: {}\n",
            status.sid, status.socket_status, status.logged_in
        )
    }

    fn handle_command<'a>(&self, name: &'a str, mut args: impl Iterator<Item=&'a str>, sender: &str) -> Result<(), Error> {
        // need to clone the handler to avoid deadlock on commands
        let cb = {
            let guard = self.commands.lock().expect("mutex poisoned");
            guard.get(name).map(|cmd| cmd.handler.clone())
        };
        if let Some(cb) = cb {
            cb(&mut args, sender, &self)?;
        }
        Ok(())
    }
}

fn set_song_number(client: &Client, song_num: i64, quiz: &SongArtistQuiz) {
    match quiz.set_song_number(song_num) {
        Some(SongInfo::Info { .. }) => {},
        Some(SongInfo::NoCatboxLinks) => client.send_chat_message("No catbox links"),
        Some(SongInfo::Undefined) => client.send_chat_message("Song not in database"),
        None => client.send_chat_message("No song data received"),
    }
}
