mod error;
mod quiz;

pub use error::Error;
pub use quiz::SongArtistQuiz;
pub use quiz::{guess::GuessResult, Guess, GuessSettings};
