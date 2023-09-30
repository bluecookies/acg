use song_artist::GuessSettings;

use super::SongArtistQuiz;

impl SongArtistQuiz {
    pub(crate) fn settings(&self) -> GuessSettings {
        let guard = self.inner.lock().expect("poisoned mutex");
        guard.settings.clone()
    }

    // Sets or toggles case sensitivity
    pub(crate) fn set_case_sensitive(&self, status: Option<bool>) -> bool {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        if let Some(b) = status {
            guard.settings.case_sensitive = b;
        } else {
            guard.settings.case_sensitive ^= true;
        }
        let status = guard.settings.case_sensitive;
        if let Ok(q) = guard.get_quiz() {
            q.quiz.set_case_sensitive(status)
        }
        status
    }

    pub(crate) fn set_space_sensitive(&self, status: Option<bool>) -> bool {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        if let Some(b) = status {
            guard.settings.space_sensitive = b;
        } else {
            guard.settings.space_sensitive ^= true;
        }
        let status = guard.settings.space_sensitive;
        if let Ok(q) = guard.get_quiz() {
            q.quiz.set_space_sensitive(status)
        }
        status
    }

    pub(crate) fn set_punctuation_sensitive(&self, status: Option<bool>) -> bool {
        let mut guard = self.inner.lock().expect("poisoned mutex");
        if let Some(b) = status {
            guard.settings.punc_sensitive = b;
        } else {
            guard.settings.punc_sensitive ^= true;
        }
        let status = guard.settings.punc_sensitive;
        if let Ok(q) = guard.get_quiz() {
            q.quiz.set_punctuation_sensitive(status)
        }
        status
    }
}
