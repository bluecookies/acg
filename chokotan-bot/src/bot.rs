use std::sync::{Arc, RwLock};

use songbird::tracks::TrackHandle;
use tokio_util::sync::CancellationToken;

use crate::quiz::SongArtistQuiz;
pub(crate) struct Data {
    // one global quiz instance for now
    pub quiz: SongArtistQuiz,
    // one global audio track instance for now
    pub track: Arc<RwLock<Option<CurrentTrack>>>,
}

impl Data {
    pub(crate) fn new(db: database::Database) -> Self {
        Data {
            quiz: SongArtistQuiz::new(db),
            track: Arc::new(RwLock::new(None)),
        }
    }
}

// TODO: this only works for one server right now,
//  can't have more than one track playing at a time
pub(crate) struct CurrentTrack {
    // Handle to the actual audio track being played
    pub handle: TrackHandle,
    // Token to stop any download/advance quiz
    pub cancel_token: CancellationToken,
}

impl Drop for CurrentTrack {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}
