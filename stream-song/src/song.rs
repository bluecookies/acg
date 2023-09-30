use std::io;
use std::sync::{Arc, RwLock};

// TODO: put this behind tokio feature
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::sync::CancellationToken;

use symphonia::core::audio::{AudioBuffer, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use rubato::{FftFixedIn, Resampler};

use crate::{extrait, Cancellable, Error, SamplePosition};

// TODO: max size - this uses 22MB per minute of audio?
//  consider saving to a file like stream download does
//  turn it into an enum, use file if greater than max size passed in?

// float pcm, stereo interleaved channels
#[derive(Clone)]
pub struct SongReader {
    inner: Arc<SongInner>,
    pos: u64,
}

struct SongWriter {
    inner: Arc<SongInner>,
    buffer: Vec<u8>,
}

struct SongInner {
    data: RwLock<Vec<u8>>,
}

impl io::Read for SongReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let num_bytes: usize = {
            let guard = self.inner.data.read().expect("lock poisoned");
            // pos could be set from a seek past the bounds
            let pos = self.pos.min(guard.len() as u64);
            let mut data = &guard[(pos as usize)..];
            data.read(buf)?
        };
        // if num_bytes is zero, then we can either return some zeroes
        // or don't return anything (and let the track end)
        self.pos += num_bytes as u64;
        Ok(num_bytes)
    }
}

impl io::Seek for SongReader {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        let (base_pos, offset) = match pos {
            io::SeekFrom::Start(n) => {
                self.pos = n;
                return Ok(n);
            }
            io::SeekFrom::End(n) => {
                let guard = self.inner.data.read().expect("lock poisoned");
                (guard.len() as u64, n)
            }
            io::SeekFrom::Current(n) => (self.pos, n),
        };
        match base_pos.checked_add_signed(offset) {
            Some(n) => {
                self.pos = n;
                Ok(n)
            }
            None => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid seek to a negative or overflowing position",
            )),
        }
    }
}

impl MediaSource for SongReader {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        let guard = self.inner.data.read().expect("lock poisoned");
        Some(guard.len() as u64)
    }
}

impl io::Write for SongWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut guard = self.inner.data.write().expect("lock poisoned");
        guard.append(&mut self.buffer);
        Ok(())
    }
}

impl SongReader {
    pub fn from_source<R: MediaSource + Cancellable + 'static>(
        source: R,
        sample: extrait::Sample,
    ) -> (Self, Receiver<Message>, CancellationToken) {
        let (reader, writer) = create_song();
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let cancel_token = source.cancel_token();
        // spawn a thread to decode/wait for the data
        std::thread::spawn(move || {
            if let Err(e) = decode_data(source, writer, tx.clone(), sample) {
                if let Err(e) = tx.blocking_send(Message::DecodeError(e)) {
                    log::warn!("song decode error ignored: {}", e);
                }
            }
        });
        (reader, rx, cancel_token)
    }
}

pub enum Message {
    StartSample(symphonia::core::units::Time),
    TotalDuration(symphonia::core::units::Time),
    Update(symphonia::core::units::Time),
    DecodeError(Error),
}

fn create_song() -> (SongReader, SongWriter) {
    let inner = Arc::new(SongInner {
        data: RwLock::new(Vec::new()),
    });
    let reader = SongReader {
        pos: 0,
        inner: inner.clone(),
    };
    let writer = SongWriter {
        buffer: Vec::new(),
        inner,
    };
    (reader, writer)
}

enum TargetTime {
    // timestamp, current loop
    Current(u64),
    // timestamp, next loop
    Next(u64),
    // no target time
    None,
}

// Maximum number of samples to resample at a time
const CHUNK_SIZE: usize = 4096;

// TODO: pass in cancellation token here to stop decoding
// also what is gapless support
fn decode_data<R: MediaSource + 'static, W: io::Write>(
    reader: R,
    mut writer: W,
    tx: Sender<Message>,
    sample: extrait::Sample,
) -> Result<(), Error> {
    let mss = MediaSourceStream::new(Box::new(reader), Default::default());

    let mut hint = Hint::new();
    hint.with_extension("mp3"); // TODO: actually pass in the extension

    // Use the default options for metadata and format readers.
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    // this function blocks to read?
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(Error::ProbeFormatError)?;

    let mut format = probed.format;

    // Find the first audio track with a known (decodeable) codec.
    // should use default_track method?
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(Error::NoAudioTrack)?;
    let codec_params = &track.codec_params;

    // Assumptions:
    //   Assume sample rate does not change,
    //     and if not provided its already at 48000Hz for no good reason
    //   Assume number of channels does not change
    //     and if not provided then it is at least 2
    let sample_rate = codec_params.sample_rate.unwrap_or(48_000) as usize;

    // Only deal with mono or stereo (preferably stereo)
    //  because we will output stereo regardless - TODO: we could allow outputting mono
    let num_channels = codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(2)
        .min(2)
        .max(1);
    let mut resampler: FftFixedIn<f32> =
        FftFixedIn::new(sample_rate, 48_000, CHUNK_SIZE, 2, num_channels)?;
    // For the resampling, just copy the decoded samples into a buffer
    let mut input_buffer = resampler.input_buffer_allocate(false);
    let mut resampled_buffer = resampler.output_buffer_allocate(true);

    // Use the default options for the decoder.
    let dec_opts: DecoderOptions = Default::default();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs().make(codec_params, &dec_opts)?;

    let track_id = track.id;
    let mut target_ts = TargetTime::None;

    // Try to get the total duration and seek to start sample
    let timebase = codec_params.time_base.ok_or(Error::NoTimeBase)?;
    // TODO: also use this to reserve capacity of vec?
    if let Some(count) = codec_params.n_frames {
        let dur = timebase.calc_time(count);
        let _ = tx.try_send(Message::TotalDuration(dur));

        // TODO - allow no wraparound
        // Seek to time
        if let SamplePosition::Random = sample.start_pos {
            use rand::Rng;
            let start_ts = rand::thread_rng().gen_range(0..count);
            let seeked_to = format
                .seek(
                    SeekMode::Coarse,
                    SeekTo::TimeStamp {
                        ts: start_ts,
                        track_id,
                    },
                )
                .map_err(Error::SeekError)?;

            // set the target to the next loop around
            //  TODO: set it to the requested time if it is after the actual
            target_ts = TargetTime::Next(seeked_to.actual_ts);
            let _ = tx.try_send(Message::StartSample(
                timebase.calc_time(seeked_to.actual_ts),
            ));
        }
    }

    // TODO: if we are before the requested seeked timestamp
    //  skip some frames until we get to it
    loop {
        let packet = match format.next_packet() {
            Ok(v) => v,
            Err(SymphoniaError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                // if we had start sample, then just exit the loop
                match target_ts {
                    TargetTime::None => break,
                    TargetTime::Next(ts) => {
                        // seek to start
                        format
                            .seek(SeekMode::Coarse, SeekTo::TimeStamp { ts: 0, track_id })
                            .map_err(Error::SeekError)?;
                        // reset decoder
                        decoder.reset();
                        target_ts = TargetTime::Current(ts);
                        format.next_packet()?
                    }
                    // weird?? we hit end of stream even though we passed this point before
                    TargetTime::Current(_) => return Err(Error::AudioStreamError),
                }
            }
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        // TODO: add actual time and duration to this
        let end_ts = packet.ts + packet.dur;
        let time = timebase.calc_time(end_ts);

        // we don't care if it fails
        // maybe the receiver has been dropped, but proceed anyways
        let _ = tx.try_send(Message::Update(time));

        // Decode the packet into samples
        //  TODO: these errors are actually recoverable
        let decoded = decoder.decode(&packet).map_err(Error::DecodeError)?;

        let mut buf: AudioBuffer<f32> = decoded.make_equivalent();
        decoded.convert(&mut buf);

        // Convert to the right sample rate for discord (48000Hz)

        // For the resampling, the process will go:
        // Try to fill input buffer to CHUNK_SIZE
        // If successful, do a resample
        //   Write the data out
        //   Clear the input buffer
        // Otherwise, we are done with this packet
        //   TODO: check exit flag to see if we need to process these last frames
        //   or we could do this at the end
        // Repeat
        let mut buf_pos = 0;
        let length = buf.frames();
        let input_length = input_buffer[0].len();
        // this should never underflow because the input buffer
        //  should never exceed CHUNK_SIZE
        let mut size = CHUNK_SIZE.saturating_sub(input_length);
        while length > buf_pos {
            let end = (buf_pos + size).min(length);
            size = CHUNK_SIZE;
            for (channel, input) in input_buffer.iter_mut().enumerate() {
                input.extend(&buf.chan(channel)[buf_pos..end]);
            }
            buf_pos = end;
            // Don't try to resample if we don't have enough data
            if input_buffer[0].len() != CHUNK_SIZE {
                break;
            }
            let result = resampler.process_into_buffer(&input_buffer, &mut resampled_buffer, None);

            // TODO: handle size errors?
            let (input_frames, output_frames) = result.map_err(Error::ResampleError)?;
            debug_assert_eq!(input_frames, CHUNK_SIZE);
            for input in input_buffer.iter_mut() {
                input.clear();
            }

            // Interleave the resampled data and write
            let left = &resampled_buffer[0][..output_frames];
            let right = if num_channels == 2 {
                &resampled_buffer[1][..output_frames]
            } else {
                left
            };
            for sample in itertools::interleave(left, right) {
                writer
                    .write(&sample.to_le_bytes())
                    .map_err(Error::AudioWriteError)?;
            }
        }

        writer.flush().map_err(Error::AudioWriteError)?;

        // TODO: i think there is a bug where it doesnt seek back to the beginning

        // TODO: only read enough frames up to the actual timestamp
        //  right now we're just reading the entire last packet
        if let TargetTime::Current(ts) = target_ts {
            if end_ts >= ts {
                break;
            }
        }
    }
    Ok(())
}
