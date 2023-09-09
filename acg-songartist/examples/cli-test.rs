use std::sync::{Arc, Mutex};

use byteorder::ReadBytesExt;
use cpal::{Stream, StreamConfig, traits::StreamTrait};
use indicatif::{ProgressStyle, ProgressBar};
use rodio::DeviceTrait;
use song_artist::Guess;
use stream_song::{SongReader, Message};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() {
    // Init db connection
    let database = match std::env::var("DATABASE_URL") {
        Ok(connection_string) => database::Database::new(connection_string),
        Err(e) => {
            println!("Failed to get database url: {}", e);
            return;
        }
    };
    println!("Database initialised...");
    // Create the quiz - we feed the info and the song number to this
    let quiz = song_artist::SongArtistQuiz::new();
    println!("Starting quiz...");
    let mut song_num = 0;

    let stdin = std::io::stdin();
    let mut input = String::new();

    let reader = Arc::new(Mutex::new(None));
    let stream = match create_output_stream(reader.clone()) {
        Ok(v) => v,
        Err(e) => {
            println!("Failed to create output stream: {}", e);
            return;
        }
    };
    stream.play().expect("Failed to play stream");

    let mut task_handle = None;
    let mut cancel_token: Option<tokio_util::sync::CancellationToken> = None;
    loop {
        println!("Fetching song {} data", song_num + 1);
        // Set the data for the next song
        let song_info = match database.get_random_song().await {
            Ok(Some(v)) => v,
            Ok(None) => {
                println!("No matching songs in database");
                break;
            }
            Err(e) => {
                println!("Failed to get song: {}", e);
                break;
            }
        };
        let info = Some(song_info.clone()).into();
        if let Err(e) = quiz.set_info(song_num + 1, info) {
            println!("Failed to set song info for song number {}: {}", song_num + 1, e);
            break;
        }
        song_num += 1;
        println!("Song {}:", song_num);
        let url = if let Some(ref url) = song_info.url {
            let url = format!("https://files.catbox.moe/{}", url);
            println!("{}", &url);
            url
        } else {
            println!("Error: No url for song");
            print!("Song name: {}", song_info.song_name);
            print!("Artist: {}", song_info.artist);
            continue;
        };
        // Cancel previous song
        if let Some(token) = cancel_token.take() {
            token.cancel();
        }
        // Download and play the song 
        let file = match stream_song::StreamDownloadFile::from_url(url).await { 
            Ok(v) => v,
            Err(e) => {
                println!("Failed to download song: {}", e);
                continue;
            }
        };
        let (song_reader, mut rx, token) = stream_song::SongReader::from_source(file);
        cancel_token = Some(token);

        let pb = ProgressBar::new(0);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn({
            let pb = pb.clone(); async move {
            pb.set_style(ProgressStyle::default_bar()
                .template("{msg} [{elapsed}] [{wide_bar:.cyan/blue}] {percent}% ({eta})")
                .expect("progress template error")
                .progress_chars("#>-"));
            pb.set_message("Buffering song...");
            let mut sender = Some(sender);
            while let Some(msg) = rx.recv().await {
                match msg {
                    Message::TotalDuration(t) => {
                        let pos = t.seconds * 1000 + (t.frac * 1000.0) as u64;
                        pb.set_length(pos);
                    }
                    Message::Update(t) => {
                        let pos = t.seconds * 1000 + (t.frac * 1000.0) as u64;
                        pb.set_position(pos);

                        // just do 15 seconds of buffer for now - TODO: change this
                        if t.seconds >= 15 {
                            if let Some(tx) = sender.take() {
                                let _ = tx.send(());
                                pb.set_message("Playing song...");
                            }
                        }
                    }
                    Message::DecodeError(e) => {
                        pb.abandon_with_message(format!("Decode error: {}", e));
                        return;
                    }
                }
            }
            // start playing if haven't already
            if let Some(tx) = sender.take() {
                let _ = tx.send(());
                pb.set_message("Playing song...");
            }
            pb.finish_with_message("Finished downloading!");
        }});
        if let Some(old) = task_handle.replace(handle) {
            old.abort();
        }

        // wait for enough to be buffered
        //  if error that means decode error
        if let Ok(_) = receiver.await {
            let mut guard = reader.lock().expect("poisoned lock");
            *guard = Some(song_reader);
        } else {
            continue;
        }

        // Handle 
        quiz.set_song_number(song_num);
        let start_time = std::time::Instant::now();
        loop {
            // Read line of input
            input.clear();
            // TODO: fix this - find a way to still show the bar
            pb.suspend(|| stdin.read_line(&mut input).expect("failed to read input"));
            let guess = input.trim();
            if guess.is_empty() {
                break;
            } else {
                pb.println(format!("Guess: {}", guess));
            }
            let time = start_time.elapsed().as_secs_f32();
            if let Some(result) = quiz.handle_guess(guess, time) {
                match result.song_guess {
                    Some(Guess::Incorrect(g, p)) => pb.println(format!("S: {} ({:.1})", g, p * 100.0)),
                    Some(Guess::Correct(g, t)) => pb.println(format!("S: {} ({:.1}s)", g, t)),
                    None => (),
                }
                let correct = result.num_correct_artists;
                let total = result.total_artists;
                for g in result.artist_guesses {
                    match g {
                        Guess::Incorrect(g, p) => pb.println(format!("A: {} ({:.1}) [{}/{}]", g, p * 100.0, correct, total)),
                        Guess::Correct(g, t) => pb.println(format!("A: {} ({:.1}s) [{}/{}]", g, t, correct, total)),
                    }
                }
            }
            if quiz.correct() {
                break;
            }
        }
        
        println!("Song name: {}", song_info.song_name);
        println!("Artist: {}", song_info.artist);
    }
}

// expects the input to produce i16 stereo
// TODO: loop this?
fn create_output_stream(input: Arc<Mutex<Option<SongReader>>>) -> Result<Stream> {
    use cpal::Sample;
    use cpal::traits::HostTrait;

    let host = cpal::default_host();
    let device = match host.default_output_device() {
        Some(v) => v,
        None => {
            return Err(String::from("Failed to get output device").into());
        }
    };
    let mut supported_configs_range = device.supported_output_configs()
        .map_err(|e| format!("Error querying configs: {}", e))?;
    let config: StreamConfig = supported_configs_range.next()
        .ok_or_else(|| "No supported configs")?
        .with_max_sample_rate()
        .into();
    // TODO: sample rate is wack
    if config.channels != 2 {
        return Err(format!("Stream has wrong number of channels ({})", config.channels).into());
    }

    let stream = device.build_output_stream(
        &config,
        move |mut data: &mut [i16], _| {
            let mut guard = input.lock().expect("poisoned lock");
            if let Some(reader) = guard.as_mut() {
                while !data.is_empty() {
                    let sample = match reader.read_i16::<byteorder::LittleEndian>() {
                        Ok(v) => v,
                        Err(_) => break,
                    };
                    data[0] = sample;
                    data = &mut data[1..];
                }
                for sample in data.iter_mut() {
                    *sample = i16::EQUILIBRIUM;
                }
            }
        }, 
        |err| println!("Error on audio stream: {}", err), 
        None,   // blocking - is this correct?
    ).map_err(|e| format!("Failed to create output stream: {}", e))?;

    Ok(stream)
}


