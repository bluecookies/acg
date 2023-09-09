use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, Instant};

use crate::Error;

pub(crate) mod guess;
mod settings;

pub use guess::Guess;
use settings::GuessSettings;

// TODO: maybe consider removing the Arcs/Mutexes from here
//  and making the user care about that
#[derive(Clone)]
pub struct SongArtistQuiz {
    song_number: Arc<AtomicI64>,
    song_info: Arc<Mutex<BTreeMap<i64, SongInfo>>>,
    curr_info: Arc<Mutex<Option<GuessInfo>>>,
    guess_settings: Arc<Mutex<GuessSettings>>,
    song_start_time: Arc<Mutex<Instant>>,
}

impl SongArtistQuiz {
    pub fn new() -> Self {
        SongArtistQuiz {
            song_number: Arc::new(AtomicI64::new(0)),
            song_info: Arc::new(Mutex::new(BTreeMap::new())),
            curr_info: Arc::new(Mutex::new(None)),
            guess_settings: Arc::new(Mutex::new(GuessSettings::default())),
            song_start_time: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn clear(&self) {
        let mut guard = self.curr_info.lock().expect("mutex poisoned");
        *guard = None;
        let mut guard = self.song_info.lock().expect("mutex poisoned");
        guard.clear();
        self.song_number.store(0, Ordering::Release);
    }

    pub fn curr_song_number(&self) -> i64 {
        self.song_number.load(Ordering::Acquire)
    }

    pub fn set_song_number(&self, num: i64) -> Option<SongInfo> {
        let map = self.song_info.lock().expect("mutex poisoned");
        let info = map.get(&num);
        let mut guard = self.curr_info.lock().expect("mutex poisoned");
        let settings = self.guess_settings.lock().expect("mutex poisoned").clone();
        *guard = info.and_then(|s| GuessInfo::from_song_info(s, settings));
        self.song_number.store(num, Ordering::Release);
        info.cloned()
    }

    pub fn set_info(&self, song_number: i64, info: SongInfo) -> Result<(), Error> {
        // TODO: allow more than 100 songs, depending on where used
        if song_number < 0 || song_number > 100 { 
            return Err(Error::SongNumberOutOfRange(song_number));
        }
        let mut map = self.song_info.lock().expect("mutex poisoned");
        if let Some(old) = map.insert(song_number, info) {
            return Err(Error::DuplicateSongInfoReceived(old));
        }

        Ok(())
    }

    pub fn set_song_timer(&self, time: f64) {
        let now = Instant::now();
        let mut guard = self.song_start_time.lock().expect("mutex poisoned");
        let dur = Duration::from_secs_f64(time);
        if let Some(t) = now.checked_sub(dur) {
            *guard = t;
        }
    }

    pub fn curr_song_timer(&self) -> f64 {
        let guard = self.song_start_time.lock().expect("mutex poisoned");
        guard.elapsed().as_secs_f64()
    }
}

#[derive(Debug, Clone)]
pub enum SongInfo {
    Info {
        song_name: String,
        artist: String,
    },
    Undefined,
    NoCatboxLinks,
}

impl From<Option<database::SongInfo>> for SongInfo {
    fn from(value: Option<database::SongInfo>) -> Self {
        if let Some(v) = value {
            SongInfo::Info {
                song_name: v.song_name,
                artist: v.artist,
            }
        } else {
            SongInfo::Undefined
        }
    }
}

// Current best guess for song name and each artist
struct BestGuess {
    song_name: Guess,
    artists: Vec<Guess>,
}

struct GuessInfo {
    song_name: String,
    // artist order (last first, first last) - TODO: extend this to nicknames, just have a list of names that work
    artists: Vec<(String, Option<String>)>,
    // Combined best guess from anyone for song name/artist
    global_best_guess: BestGuess,
    // player_guesses: HashMap<String, BestGuess>,
    settings: GuessSettings,
}

impl GuessInfo {
    fn from_song_info(info: &SongInfo, settings: GuessSettings) -> Option<GuessInfo> {
        if let SongInfo::Info { ref song_name, ref artist } = info {
            let song_name = settings.normalise_sn(song_name);

            let artists = settings.normalise_artists(artist);

            let global_best_guess = BestGuess {
                song_name: Guess::default(),
                artists: vec![Guess::default(); artists.len()],
            };

            Some(GuessInfo {
                song_name,
                artists,
                global_best_guess,
                settings,
            })
        } else {
            None
        }
    }
}
