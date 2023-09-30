use crate::quiz::SongArtistQuiz;
use itertools::Either;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone)]
pub struct GuessSettings {
    pub answer_threshold: f32,
    pub display_threshold: f32,
    split_re: regex::Regex,
    pub space_sensitive: bool,
    pub case_sensitive: bool,
    pub punc_sensitive: bool,
}

impl Default for GuessSettings {
    fn default() -> Self {
        let split_re = regex::Regex::new(r",| feat\. | to | with |・|&|×| + | featuring ")
            .expect("regex compilation failed");
        GuessSettings {
            answer_threshold: 0.995,
            display_threshold: 0.50,
            split_re,
            space_sensitive: false,
            case_sensitive: false,
            punc_sensitive: false,
        }
    }
}

impl GuessSettings {
    pub fn check_sn(&self, guess: &str, target: &str) -> f32 {
        let guess = guess.trim();
        let target = target.trim();
        strsim::normalized_damerau_levenshtein(guess, target) as f32
    }

    pub fn check_artist(&self, guess: &str, target: &(String, Option<String>)) -> f32 {
        let guess = guess.trim();
        let s1 = strsim::normalized_damerau_levenshtein(guess, target.0.trim()) as f32;
        if let Some(ref t2) = target.1 {
            let s2 = strsim::normalized_damerau_levenshtein(guess, t2.trim()) as f32;
            s1.max(s2)
        } else {
            s1
        }
    }

    fn normalise(&self, it: impl Iterator<Item = char>) -> String {
        let it = it.nfd();
        let it = if self.case_sensitive {
            Either::Left(it)
        } else {
            Either::Right(it.flat_map(|c| c.to_lowercase()))
        };
        let condition = |c: &char| -> bool {
            (if self.punc_sensitive {
                c.is_ascii()
            } else {
                c.is_ascii_alphanumeric()
            }) && (self.space_sensitive || !c.is_whitespace())
        };
        it.filter(condition).collect::<String>()
    }

    pub(crate) fn normalise_sn(&self, sn: &str) -> String {
        self.normalise(sn.chars())
    }

    pub(crate) fn normalise_artists(&self, artist: &str) -> Vec<(String, Option<String>)> {
        self.split_re
            .split(artist)
            .map(|a| self.normalise_artist(a))
            .collect()
    }

    fn normalise_artist(&self, artist: &str) -> (String, Option<String>) {
        let reversed = if let Some((first, last)) = artist.rsplit_once(' ') {
            let (first, last) = (first.trim(), last.trim());
            let name = self.normalise(last.nfd().chain(std::iter::once(' ')).chain(first.nfd()));
            Some(name)
        } else {
            None
        };
        let name = self.normalise(artist.nfd());
        (name, reversed)
    }
}

impl SongArtistQuiz {
    pub fn set_case_sensitive(&self, status: bool) {
        let mut guard = self.guess_settings.lock().expect("mutex poisoned");
        guard.case_sensitive = status;
    }

    pub fn set_space_sensitive(&self, status: bool) {
        let mut guard = self.guess_settings.lock().expect("mutex poisoned");
        guard.space_sensitive = status;
    }

    pub fn set_punctuation_sensitive(&self, status: bool) {
        let mut guard = self.guess_settings.lock().expect("mutex poisoned");
        guard.punc_sensitive = status;
    }

    pub fn toggle_case_sensitive(&self) -> bool {
        let mut guard = self.guess_settings.lock().expect("mutex poisoned");
        guard.case_sensitive = !guard.case_sensitive;
        return guard.case_sensitive;
    }

    pub fn toggle_space_sensitive(&self) -> bool {
        let mut guard = self.guess_settings.lock().expect("mutex poisoned");
        guard.space_sensitive = !guard.space_sensitive;
        return guard.space_sensitive;
    }

    pub fn toggle_punctuation_sensitive(&self) -> bool {
        let mut guard = self.guess_settings.lock().expect("mutex poisoned");
        guard.punc_sensitive = !guard.punc_sensitive;
        return guard.punc_sensitive;
    }
}
