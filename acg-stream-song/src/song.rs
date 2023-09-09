use std::io;
use std::sync::{Arc, RwLock};

// TODO: put this behind tokio feature
use tokio::sync::mpsc::{Sender, Receiver};
use tokio_util::sync::CancellationToken;

use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::audio::{AudioBuffer, Signal};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::errors::Error as SymphoniaError;

use crate::{Error, Cancellable, SamplePosition, extrait};

// TODO: max size - this uses 11MB per minute of audio?
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
    pub fn from_source<R: MediaSource + Cancellable + 'static>(source: R, sample: extrait::Sample) -> (Self, Receiver<Message>, CancellationToken) {
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


// TODO: pass in cancellation token here to stop decoding
fn decode_data<R: MediaSource + 'static, W: io::Write>(
    reader: R,
    mut writer: W,
    tx: Sender<Message>,
    sample: extrait::Sample,
) -> Result<(), Error> {
    let mss = MediaSourceStream::new(Box::new(reader), Default::default());

    let mut hint = Hint::new();
    hint.with_extension("mp3");  // TODO: actually pass in the extension

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

    // Use the default options for the decoder.
    let dec_opts: DecoderOptions = Default::default();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make(codec_params, &dec_opts)?;

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
            let seeked_to = format.seek(SeekMode::Coarse, SeekTo::TimeStamp {
                ts: start_ts,
                track_id,
            }).map_err(Error::SeekError)?;

            // set the target to the next loop around
            //  TODO: set it to the requested time if it is after the actual
            target_ts = TargetTime::Next(seeked_to.actual_ts);
        }
    }

    // TODO: if we are before the requested seeked timestamp
    //  skip some frames until we get to it

    // TODO: this is incorrect right now because of the sample rate
    //  songbird expects it to be 48000Hz - need to use rubato to resample
    loop {
        let packet = match format.next_packet() {
            Ok(v) => v,
            Err(SymphoniaError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                // if we had start sample, then just exit the loop
                match target_ts {
                    TargetTime::None => break,
                    TargetTime::Next(ts) => {
                        // seek to start
                        format.seek(SeekMode::Coarse, SeekTo::TimeStamp {
                            ts: 0,
                            track_id,
                        }).map_err(Error::SeekError)?;
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

        let decoded = decoder
            .decode(&packet)
            .map_err(Error::DecodeError)?;

        let mut buf: AudioBuffer<i16> = decoded.make_equivalent();
        decoded.convert(&mut buf);
        let num_channels = buf.spec().channels.count();
        if num_channels == 1 {
            // duplicate the mono channel
            for sample in buf.chan(0).iter() {
                writer.write(&sample.to_le_bytes()).map_err(Error::AudioWriteError)?;
                writer.write(&sample.to_le_bytes()).map_err(Error::AudioWriteError)?;
            }
        } else {
            // interleave stereo channels
            let left = buf.chan(0);
            let right = buf.chan(1);
            for (l, r) in left.iter().zip(right.iter()) {
                writer.write(&l.to_le_bytes()).map_err(Error::AudioWriteError)?;
                writer.write(&r.to_le_bytes()).map_err(Error::AudioWriteError)?;
            }
        }
        writer.flush().map_err(Error::AudioWriteError)?;

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
