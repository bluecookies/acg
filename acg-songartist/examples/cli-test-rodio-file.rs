use std::{fs::File, io::{Write, Seek, BufReader}};

use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use rodio::{Decoder, OutputStream, Sink};
use song_artist::Guess;

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
    let client = reqwest::Client::new();
    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(v) => v,
        Err(e) => {
            println!("Failed to get output stream handle: {}", e);
            return;
        }
    };
    let sink = match Sink::try_new(&stream_handle) {
        Ok(v) => v,
        Err(e) => {
            println!("Failed to create sink: {}", e);
            return;
        }
    };

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
        // Download and play the song
        let file = match download_file(&client, &url).await {
            Ok(v) => v,
            Err(e) => {
                println!("Failed to download song: {}", e);
                continue;
            }
        };
        
        if let Err(e) = play_file(file, &sink) {
            println!("Failed to play file: {}", e);
            continue;
        }

        // Handle 
        quiz.set_song_number(song_num);
        let start_time = std::time::Instant::now();
        loop {
            // Read line of input
            input.clear();
            stdin.read_line(&mut input).expect("failed to read input");
            let guess = input.trim();
            if guess.is_empty() {
                break;
            } else {
                println!("Guess: {}", guess);
            }
            let time = start_time.elapsed().as_secs_f32();
            if let Some(result) = quiz.handle_guess(guess, time) {
                match result.song_guess {
                    Some(Guess::Incorrect(g, p)) => println!("S: {} ({:.1})", g, p * 100.0),
                    Some(Guess::Correct(g, t)) => println!("S: {} ({:.1}s)", g, t),
                    None => (),
                }
                let correct = result.num_correct_artists;
                let total = result.total_artists;
                for g in result.artist_guesses {
                    match g {
                        Guess::Incorrect(g, p) => println!("A: {} ({:.1}) [{}/{}]", g, p * 100.0, correct, total),
                        Guess::Correct(g, t) => println!("A: {} ({:.1}s) [{}/{}]", g, t, correct, total),
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

async fn download_file(client: &Client, url: &str) -> Result<File> {
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to send GET request: {}", e))?;
    let total_size = res.content_length().ok_or_else(|| format!("No content length for url ({})", url))?;

    // Indicatif setup
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})\n")
        .map_err(|e| format!("Progress template error: {}", e))?
        .progress_chars("#>-"));
    pb.set_message(format!("Downloading {}", url));

    // download chunks
    let mut file = tempfile::tempfile().map_err(|e| format!("Failed to create temp file: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("Error while downloading file: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Error while writing to file: {}", e))?;
        let new = std::cmp::min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);
    }

    pb.finish_with_message("Finished downloading!");
    file.rewind().map_err(|e| format!("Rewind failed: {}", e))?;
    Ok(file)
}

fn play_file(file: File, sink: &Sink) -> Result<()> {
    let reader = BufReader::new(file);
    let source = Decoder::new_looped(reader)
        .map_err(|e| format!("Failed to decode file: {}", e))?;

    sink.stop();
    sink.append(source);
    Ok(())
}
