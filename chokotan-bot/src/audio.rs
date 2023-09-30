use songbird::input::{Input, Reader};
use tokio::sync::mpsc::Receiver;
use tokio_util::sync::CancellationToken;

pub use stream_song::Message;

use crate::Error;

// Returns source to be played and receiver for messages
pub async fn create_input(
    url: impl AsRef<str>,
    sample: stream_song::Sample,
) -> Result<(Input, Receiver<Message>, CancellationToken), Error> {
    let stream = stream_song::StreamDownloadFile::from_url(url)
        .await
        .map_err(Error::StreamDownloadErr)?;
    let (song_reader, rx, cancel_token) = stream_song::SongReader::from_source(stream, sample);

    let reader = Reader::Extension(Box::new(song_reader));
    let input = Input::new(
        true,
        reader,
        songbird::input::Codec::FloatPcm,
        songbird::input::Container::Raw,
        None,
    );
    Ok((input, rx, cancel_token))
}
