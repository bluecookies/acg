use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Instant,
};

use poise::serenity_prelude as serenity;
use serenity::ChannelId;

use song_artist::GuessSettings;
use stream_song::Message;
use tokio_util::sync::CancellationToken;

mod config;
mod settings;

use crate::Error;
pub(crate) use config::{
    Error as LoadConfigError, ParamError, QuizConfig, QuizInfoField, QuizParameters,
};

// TODO: make this something that handles all quizzes across servers
#[derive(Clone)]
pub(crate) struct SongArtistQuiz {
    inner: Arc<Mutex<QuizInner>>,
    db: database::Database,
}

struct QuizInner {
    state: QuizState,
    // settings
    settings: song_artist::GuessSettings,
    configs: HashMap<Box<str>, Arc<QuizConfig>>,
    // other stuff - todo: these should go into the quiz state too
    channel_id: Option<ChannelId>,
    cancel_token: CancellationToken,
    task: Option<QuizTask>,
}

impl QuizInner {
    fn new(configs: HashMap<Box<str>, Arc<QuizConfig>>) -> Self {
        QuizInner {
            state: QuizState::NotStarted,
            channel_id: None,
            cancel_token: CancellationToken::new(),
            task: None,

            settings: song_artist::GuessSettings::default(),
            configs,
        }
    }

    fn get_quiz(&mut self) -> Result<&mut Quiz, Error> {
        match self.state {
            QuizState::NotStarted => Err(Error::NoQuizRunning),
            QuizState::Started(ref mut q) => Ok(q),
        }
    }
}

// TODO: this isnt structured very well
// channel_id, token should only exist when quiz is started
enum QuizState {
    NotStarted,
    Started(Quiz),
}

struct Quiz {
    song_num: i64,
    // TODO: the way this is structured sort of masks any errors I have the song number out of sync
    song_info: VecDeque<database::SongInfo>,
    song_start_time: Instant,
    song_token: CancellationToken,
    quiz: song_artist::SongArtistQuiz,
    //
    config: Arc<QuizConfig>,
    params: QuizParameters,
}

type SongInfo = (database::SongInfo, Arc<[QuizInfoField]>);

impl Quiz {
    fn new(config: Arc<QuizConfig>, params: QuizParameters, settings: GuessSettings) -> Self {
        Quiz {
            song_num: 0,
            song_info: VecDeque::new(),
            song_start_time: Instant::now(),
            song_token: CancellationToken::new(),
            quiz: song_artist::SongArtistQuiz::new_with_settings(settings),
            config,
            params,
        }
    }
}

type QuizTask = tokio::task::JoinHandle<Result<(), Error>>;

impl SongArtistQuiz {
    pub(crate) fn new(db: database::Database) -> Self {
        // TODO: have a default for each user id in a file

        // read config files for song types
        let quiz_configs = match config::read_configs() {
            Ok(v) => {
                for (name, path) in v.duplicates {
                    log::warn!("Read duplicate config for `{}` at {}", name, path);
                }
                for (e, path) in v.errors {
                    log::error!("Error reading config at {}: {}", path, e);
                }
                v.success
                    .into_iter()
                    .map(|(k, v)| (k, Arc::new(v)))
                    .collect()
            }
            Err(e) => {
                log::error!("Failed to read quiz queries: {}", e);
                HashMap::new()
            }
        };

        SongArtistQuiz {
            inner: Arc::new(Mutex::new(QuizInner::new(quiz_configs))),
            db,
        }
    }

    pub(crate) fn start(
        &self,
        channel_id: ChannelId,
        quiz_type: String,
        params: HashMap<String, String>,
    ) -> Result<(), Error> {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        log::debug!(
            "Starting quiz of type `{}` in channel `{}`",
            quiz_type,
            channel_id
        );
        let config = guard
            .configs
            .get(quiz_type.as_str())
            .ok_or(Error::InvalidQuizType(quiz_type))?
            .clone();

        let params = config.parse_params(params)?;

        match guard.state {
            QuizState::NotStarted => {
                guard.cancel_token = CancellationToken::new();
                guard.channel_id = Some(channel_id);
                guard.state = QuizState::Started(Quiz::new(config, params, guard.settings.clone()));
            }
            QuizState::Started { .. } => return Err(Error::QuizAlreadyStarted),
        }
        Ok(())
    }

    pub(crate) fn stop(&self) -> Result<Option<QuizTask>, Error> {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        let task;
        match guard.state {
            QuizState::NotStarted => return Err(Error::NoQuizRunning),
            QuizState::Started { .. } => {
                guard.cancel_token.cancel();
                guard.channel_id = None;
                task = guard.task.take();
                guard.state = QuizState::NotStarted;
            }
        }
        Ok(task)
    }

    // TODO: pass in a cancel token?
    pub(crate) async fn load_next_song(
        &self,
        song_data: &mut QuizSongData,
    ) -> Result<songbird::input::Input, Error> {
        // Get the data for the next song
        let (song_info, fields) = self.next_song_info().await?;
        let url = song_info.url().ok_or(Error::NoCatboxUrl)?;
        let url = format!("https://files.catbox.moe/{}", url);

        song_data.song_info = Some(song_info.clone());
        song_data.display_fields = fields;

        // Fetch the next song
        // TODO: allow start samples again
        let sample = stream_song::Sample {
            start_pos: stream_song::SamplePosition::Random,
        };
        let (source, mut loader_rx, cancel_token) =
            crate::audio::create_input(&url, sample).await?;

        let info = Some(song_info).into();

        // Set the song info for the next song
        {
            let mut guard = self.inner.lock().expect("poisoned mutex");
            let quiz = guard.get_quiz()?;
            quiz.quiz
                .set_info(quiz.song_num + 1, info)
                .map_err(Error::SetSongInfo)?;
            log::debug!("Loading song {}: {}", quiz.song_num + 1, &url);
        }

        // Wait for 15 seconds (TODO - make variable) to be buffered
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => break,
                Some(x) = loader_rx.recv() => {
                    match x {
                        Message::StartSample(t) => {
                            song_data.sample = Some(t.seconds as f64 + t.frac);
                        }
                        Message::TotalDuration(t) => {
                            song_data.duration = Some(t.seconds as f64 + t.frac);
                        }
                        Message::Update(t) => if t.seconds >= 15 { break },
                        Message::DecodeError(e) => return Err(Error::DecodeSongError(e)),
                    }
                }
                else => break,
            }
        }
        Ok(source)
    }

    async fn next_song_info(&self) -> Result<SongInfo, Error> {
        // if we already have song info loaded just return that
        let (config, params) = {
            let mut guard = self.inner.lock().expect("poisoned mutex");
            let quiz = guard.get_quiz()?;

            if let Some(info) = quiz.song_info.pop_front() {
                let fields = quiz.config.fields().clone();
                return Ok((info, fields));
            }

            let config = quiz.config.clone();
            let params = quiz.params.clone();
            (config, params)
        };

        // otherwise make a database query
        let fields = config.fields().clone();
        log::debug!(
            "Making database request for song info for quiz {}",
            config.name()
        );
        let next_batch = load_song_info(&self.db, config, params).await?;
        let song_info = {
            let mut guard = self.inner.lock().expect("poisoned mutex");
            let quiz = guard.get_quiz()?;

            quiz.song_info.extend(next_batch);
            // song_info was not empty or we would have returned an error
            let info = quiz.song_info.pop_front().expect("song info pop failed");
            (info, fields)
        };
        Ok(song_info)
    }

    pub(crate) fn set_song_number(&self, song_num: i64) -> Result<(), Error> {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        let quiz = guard.get_quiz()?;
        quiz.song_num = song_num;
        quiz.song_start_time = Instant::now();
        quiz.quiz.set_song_number(quiz.song_num);

        Ok(())
    }

    pub(crate) fn skip_song(&self) -> Result<(), Error> {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        let quiz = guard.get_quiz()?;
        log::debug!("Cancelling - skipping song {}", quiz.song_num);
        quiz.song_token.cancel();
        Ok(())
    }

    pub(crate) fn handle_guess(
        &self,
        guess: &str,
        time: std::time::Instant,
    ) -> Result<Option<song_artist::GuessResult>, Error> {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        let quiz = guard.get_quiz()?;

        let time = time - quiz.song_start_time;

        let result = quiz.quiz.handle_guess(guess, time.as_secs_f32());

        // TODO: do this outside cos the quiz result shows up before
        if quiz.quiz.correct() {
            quiz.song_token.cancel();
        }

        Ok(result)
    }

    pub(crate) fn channel_id(&self) -> Option<ChannelId> {
        let guard = self.inner.lock().expect("poisoned mutex");
        guard.channel_id.clone()
    }

    pub(crate) fn cancel_token(&self) -> CancellationToken {
        let guard = self.inner.lock().expect("poisoned mutex");
        guard.cancel_token.clone()
    }

    pub(crate) fn set_song_token(&self, token: CancellationToken) -> Result<(), Error> {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        let quiz = guard.get_quiz()?;

        quiz.song_token.cancel();
        quiz.song_token = token;

        Ok(())
    }

    pub(crate) fn set_task(&self, task: QuizTask) {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        guard.task = Some(task);
    }

    pub(crate) fn reload_config(&self) -> Result<config::LoadedConfigsResult, config::Error> {
        let loaded = config::read_configs()?;
        let quiz_configs: HashMap<_, _> = loaded
            .success
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect();
        let num_configs = quiz_configs.len();
        let mut guard = self.inner.lock().expect("poisoned mutex");
        guard.configs = quiz_configs;
        Ok(config::LoadedConfigsResult {
            num_configs,
            num_dupes: loaded.duplicates.len(),
            errors: loaded.errors,
        })
    }

    pub(crate) fn configs(&self) -> HashMap<Box<str>, Arc<QuizConfig>> {
        let guard = self.inner.lock().expect("poisoned mutex");
        guard.configs.clone()
    }
}

pub struct QuizSongData {
    pub song_info: Option<database::SongInfo>,
    // fields to show
    //  (name, field name)
    pub display_fields: Arc<[QuizInfoField]>,
    // start time in seconds
    pub sample: Option<f64>,
    // duration of the entire song in seconds
    pub duration: Option<f64>,
}

impl Default for QuizSongData {
    fn default() -> Self {
        Self {
            song_info: Default::default(),
            display_fields: Arc::new([]),
            sample: Default::default(),
            duration: Default::default(),
        }
    }
}

async fn load_song_info(
    db: &database::Database,
    config: Arc<QuizConfig>,
    params: QuizParameters,
) -> Result<Vec<database::SongInfo>, Error> {
    let song_info = db
        .fetch_song_info(config.query(), &params, config.db_types())
        .await
        .map_err(Error::GetSongErr)?;

    if song_info.is_empty() {
        return Err(Error::NoDatabaseSongs);
    }

    Ok(song_info)
}
