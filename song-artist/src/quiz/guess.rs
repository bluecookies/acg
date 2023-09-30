use crate::quiz;
use crate::quiz::GuessInfo;

mod edit;
use edit::EditCommand;

#[derive(Debug, Clone)]
pub enum Guess {
    // guess, %
    Incorrect(String, f32),
    // TODO: make time optional
    // guess, time in seconds
    Correct(String, f32),
}

impl Guess {
    fn correct(&self) -> bool {
        match self {
            Guess::Correct(..) => true,
            Guess::Incorrect(..) => false,
        }
    }
}

impl Default for Guess {
    fn default() -> Self {
        Guess::Incorrect(String::new(), 0.0)
    }
}

pub struct GuessResult {
    pub song_guess: Option<Guess>,
    pub artist_guesses: Vec<Guess>,
    pub num_correct_artists: usize,
    pub total_artists: usize,
}

impl quiz::SongArtistQuiz {
    pub fn handle_guess(&self, guess: &str, time: f32) -> Option<GuessResult> {
        let mut guard = self.curr_info.lock().expect("mutex poisoned");
        let guess_info = guard.as_mut()?;

        // handle edits
        let edit = EditCommand::new(guess);

        // TODO: move this when handling artists differently
        let guess_norm = guess_info.settings.normalise_sn(guess);

        // check song name
        let song_guess = check_song_name(&guess_norm, &guess, guess_info, edit, time);

        let artists = &guess_info.artists;

        // check artists
        let mut display_artists = vec![false; artists.len()];
        let mut num_correct_artists = 0;

        // TODO: check each artist in split
        // let artists: Vec<_> = settings.split_re
        //     .split(guess)
        //     .map(|a| settings.normalise_artist(a))

        let settings = &guess_info.settings;
        let best_artists = &mut guess_info.global_best_guess.artists;
        for (best_artist_guess, artist, display) in
            itertools::izip!(best_artists, artists, &mut display_artists)
        {
            if let Guess::Incorrect(ref old, percent) = best_artist_guess {
                let mut guess_artist = settings.check_artist(&guess_norm, artist);
                let mut edited = None;
                if let Some(s) = edit.apply(old) {
                    let edit_guess = settings.check_artist(&settings.normalise_sn(&s), artist);
                    if edit_guess > guess_artist {
                        guess_artist = edit_guess;
                        edited = Some(s);
                    }
                }

                if guess_artist > settings.answer_threshold {
                    *best_artist_guess =
                        Guess::Correct(edited.unwrap_or_else(|| guess.to_string()), time);
                    *display = true;
                    num_correct_artists += 1;
                } else if guess_artist > *percent {
                    *best_artist_guess =
                        Guess::Incorrect(edited.unwrap_or_else(|| guess.to_string()), guess_artist);
                    *display = guess_artist > settings.display_threshold;
                }
            } else {
                num_correct_artists += 1;
            }
        }

        let artist_guesses: Vec<Guess> = display_artists
            .into_iter()
            .zip(guess_info.global_best_guess.artists.iter())
            .filter_map(|(display, guess)| if display { Some(guess.clone()) } else { None })
            .collect();

        if song_guess.is_none() && artist_guesses.is_empty() {
            return None;
        }

        Some(GuessResult {
            song_guess,
            artist_guesses,
            num_correct_artists,
            total_artists: artists.len(),
        })
    }

    pub fn correct(&self) -> bool {
        let guard = self.curr_info.lock().expect("mutex poisoned");
        if let Some(info) = guard.as_ref() {
            let best = &info.global_best_guess;
            best.song_name.correct() && best.artists.iter().all(Guess::correct)
        } else {
            false
        }
    }
}

// returns song name guess if it should be displayed
fn check_song_name(
    guess_norm: &str,
    guess_raw: &str,
    guess_info: &mut GuessInfo,
    edit: EditCommand,
    time: f32,
) -> Option<Guess> {
    let song_name = &guess_info.song_name;
    let best_sn = &mut guess_info.global_best_guess.song_name;
    let settings = &guess_info.settings;

    let mut display_song_name = false;
    if let Guess::Incorrect(ref old, percent) = best_sn {
        let mut guess_sn = settings.check_sn(guess_norm, song_name);
        let mut edited = None;
        if let Some(s) = edit.apply(old) {
            let edit_guess = settings.check_sn(&settings.normalise_sn(&s), song_name);
            if edit_guess > guess_sn {
                guess_sn = edit_guess;
                edited = Some(s);
            }
        }
        if guess_sn > settings.answer_threshold {
            *best_sn = Guess::Correct(edited.unwrap_or_else(|| guess_raw.to_string()), time);
            display_song_name = true;
        } else if guess_sn > *percent {
            *best_sn = Guess::Incorrect(edited.unwrap_or_else(|| guess_raw.to_string()), guess_sn);
            display_song_name = guess_sn > settings.display_threshold;
        }
    }
    if display_song_name {
        Some(best_sn.clone())
    } else {
        None
    }
}
