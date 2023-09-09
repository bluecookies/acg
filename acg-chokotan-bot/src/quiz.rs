use std::{sync::{Arc, Mutex}, time::Instant, collections::HashMap};

use serenity::model::prelude::ChannelId;
use stream_song::Message;
use tokio_util::sync::CancellationToken;

use crate::Error;

#[derive(Clone)]
pub(crate) struct SongArtistQuiz {
    inner: Arc<Mutex<QuizInner>>,
    db: database::Database,
}

struct QuizInner {
    state: QuizState,
    // settings
    quiz_types: HashMap<String, Arc<SongInfoCallback>>,
    default_quiz_type: Arc<SongInfoCallback>,
    // other stuff
    channel_id: Option<ChannelId>,
    cancel_token: CancellationToken,
    task: Option<QuizTask>,
}

impl QuizInner {
    fn new(
        quiz_types: HashMap<String, Arc<SongInfoCallback>>,
        default_quiz_type: Arc<SongInfoCallback>
    ) -> Self {
        QuizInner {
            state: QuizState::NotStarted,
            channel_id: None,
            cancel_token: CancellationToken::new(),
            task: None,

            quiz_types,
            default_quiz_type,
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
    song_start_time: Instant,
    song_token: CancellationToken,
    quiz: song_artist::SongArtistQuiz,
    //
    song_info_cb: Arc<SongInfoCallback>,
}

// TODO: also allow path to be returned instead of url
type SongInfo = (database::SongInfo, String);
// TODO: i'm too stupid to figure out the right lifetimes for &Database - ask for help on this
type SongInfoCallback = Box<dyn (Fn(database::Database) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<SongInfo, Error>> + Send>>) + Send + Sync>;

impl Quiz {
    fn new(song_info_cb: Arc<SongInfoCallback>) -> Self {
        Quiz {
            song_num: 0,
            song_start_time: Instant::now(),
            song_token: CancellationToken::new(),
            quiz: song_artist::SongArtistQuiz::new(),
            song_info_cb,
        }
    }
}

type QuizTask = tokio::task::JoinHandle<Result<(), Error>>;

impl SongArtistQuiz {
    pub(crate) fn new(db: database::Database) -> Self {
        let default_quiz_type = Arc::new(Box::new(
            box_future(load_amq_song_info)
        ) as SongInfoCallback);

        // TODO: hardcoded for now but can read sql queries from file
        let mut quiz_types = HashMap::new();
        quiz_types.insert(String::from("amq"), default_quiz_type.clone());
        quiz_types.insert(String::from("karaoke"), Arc::new(Box::new(box_future(load_karaoke_song_info)) as SongInfoCallback));

        SongArtistQuiz {
            inner: Arc::new(Mutex::new(QuizInner::new(quiz_types, default_quiz_type))),
            db,
        }
    }

    pub(crate) fn start(&self, channel_id: ChannelId, quiz_type: Option<String>) -> Result<(), Error> {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        let song_info_cb = if let Some(q) = quiz_type {
            guard.quiz_types.get(&q).ok_or(Error::InvalidQuizType(q))?.clone()
        } else {
            guard.default_quiz_type.clone()
        };
        match guard.state {
            QuizState::NotStarted => {
                guard.cancel_token = CancellationToken::new();
                guard.channel_id = Some(channel_id);
                guard.state = QuizState::Started(Quiz::new(song_info_cb));
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

    // TODO: return cancel token from this
    pub(crate) async fn load_next_song(&self, song_data: &mut QuizSongData) -> Result<songbird::input::Input, Error> {
        // Get the data for the next song
        let get_next_song_info = {
            let mut guard = self.inner.lock().expect("poisoned mutex");
            let quiz = guard.get_quiz()?;
            // we don't call the function here because we don't want to hold the lock across an await
            quiz.song_info_cb.clone()
        };
        let (song_info, url) = get_next_song_info(self.db.clone()).await?;

        song_data.song_info = Some(song_info.clone());

        // Fetch the next song
        // TODO: allow start samples again
        let sample = stream_song::Sample {
            start_pos: stream_song::SamplePosition::Random,
        };
        // TODO: see if i can return the sample used
        let (source, mut loader_rx, cancel_token) = crate::audio::create_input(&url, sample).await?;

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
                        Message::TotalDuration(_) => (),
                        Message::Update(t) => if t.seconds >= 15 { break },
                        // TODO: figure out if i can still return the song info
                        Message::DecodeError(e) => return Err(Error::DecodeSongError(e)),
                    }
                }
                else => break,
            }
        }    
        Ok(source)
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

    pub(crate) fn handle_guess(&self, guess: &str, time: std::time::Instant) -> Result<Option<song_artist::GuessResult>, Error> {
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
}


#[derive(Default)]
pub struct QuizSongData {
    pub song_info: Option<database::SongInfo>,
}



async fn load_amq_song_info(db: database::Database) -> Result<SongInfo, Error> {
    let song_info = match db.get_random_song().await {
        Ok(Some(v)) => v,
        Ok(None) => return Err(Error::NoDatabaseSongs),
        Err(e) => return Err(Error::GetSongErr(e)),
    };

    let url = song_info.url.as_ref().ok_or(Error::NoCatboxUrl)?;
    let url: String = format!("https://files.catbox.moe/{}", url);

    Ok((song_info, url))
}

async fn load_karaoke_song_info(db: database::Database) -> Result<SongInfo, Error> {
    let song_info = match db.get_karaoke_song().await {
        Ok(Some(v)) => v,
        Ok(None) => return Err(Error::NoDatabaseSongs),
        Err(e) => return Err(Error::GetSongErr(e)),
    };

    let url = song_info.url.as_ref().ok_or(Error::NoCatboxUrl)?;
    let url: String = format!("https://files.catbox.moe/{}", url);

    Ok((song_info, url))
}

fn box_future<F, T: std::future::Future<Output=F> + Send + 'static>(
    f: impl Fn(database::Database) -> T + Send + Sync + 'static
) -> impl Fn(database::Database) -> std::pin::Pin<Box<dyn std::future::Future<Output = F> + Send>> + Send + Sync {
    move |db| Box::pin(f(db)) as std::pin::Pin<Box<dyn std::future::Future<Output = F> + Send>>
}