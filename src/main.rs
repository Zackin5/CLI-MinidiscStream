use std::{io, fs, path::PathBuf, fs::File, ffi::OsStr, time, thread, ops::Range, cmp};
use rodio::{Decoder, OutputStream, Sink, cpal::{self, traits::HostTrait, traits::DeviceTrait}};
use clap::Parser;
use chrono::{Local};

/// Simple program to play a folder of songs to an audio device
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct InputArgs {
    /// Album directory to play files from
    #[arg(short, long)]
    album_path: String,

    /// Delay between songs (useful to automatically add track markers, rec 4 seconds)
    #[arg(short, long, default_value_t = 0)]
    delay: u8,

    /// Track range selector (use {skip_n}:{take_n}. Either {} can be empty. {take_n} supports negative values to count from end)
    #[arg(short, long, default_value_t = String::new())]
    track_select: String
}


fn parse_track_ranges(track_select_string: &String) -> std::ops::Range<Option<isize>>{
    if track_select_string.is_empty() {
        return Range{ start: None, end: None}
    }

    let range_strs: Vec<String> = track_select_string.split(':').map(str::to_string).collect();

    // Parse lower bound
    let mut lower_bound: Option<isize> = None;

    if !range_strs[0].is_empty() {
        lower_bound = match range_strs[0].trim().parse::<isize>() {
            Ok(i) => Some(i),
            Err(_) => {
                panic!("Lower range input \"{}\" could not be parsed to an integer", range_strs[0].trim());
            }
        };
    }

    // Parse upper bound (if any)
    if range_strs.len() < 2 || range_strs[1].is_empty() {
        return Range { start: lower_bound, end: None};
    }

    let upper_bound = match range_strs[1].trim().parse::<isize>() {
        Ok(i) => i,
        Err(_) => {
            panic!("Upper range input \"{}\" could not be parsed to an integer", range_strs[1].trim());
        }
    };

    return Range { start: lower_bound, end: Some(upper_bound) }
}


fn get_audio_paths(album_path: &String, track_range: std::ops::Range<Option<isize>>) -> Vec<PathBuf> {
    // Convert track ranges to valid counters
    let lower_bound = match track_range.start {
        Some(i) => i,
        None => 0
    };

    let mut upper_bound = match track_range.end {
        Some(i) => i,
        None => 0
    };

    // Read directory contents
    let valid_exts = vec![OsStr::new("mp3"), OsStr::new("wav"), OsStr::new("flac")];  // Filter list for file selector

    let mut folder_song_contents: Vec<PathBuf> = Vec::new();  // Result list
    for item in fs::read_dir(album_path).expect("Failed to read path") {
        if let Ok(item) = item {
            if item.path().extension().is_some() && valid_exts.contains(&item.path().extension().unwrap()) {
                folder_song_contents.push(item.path())
            }
        }
    }

    // Calculate and print selection range
    if lower_bound > 0 {
        println!("Skipping {} songs", lower_bound);
    }

    if upper_bound > 1 {
        println!("Taking {} songs", lower_bound);
    }
    else if upper_bound < 0 {
        let track_count = folder_song_contents.len();
        let cached_negative_bound = upper_bound.abs();
        upper_bound = cmp::max(track_count as isize - cached_negative_bound, 0);
        println!("Taking {} ({} - {}) songs", upper_bound, track_count, cached_negative_bound);
    }

    // Select track range from folder results
    let mut selected_songs: Vec<PathBuf> = Vec::new();
    for (i, path) in folder_song_contents.iter().enumerate() {
        if i < lower_bound as usize {
            continue;
        }

        if upper_bound != 0 && i >= upper_bound as usize {
            break;
        }

        println!("{:?}", path);
        selected_songs.push(path.to_path_buf());
    }

    return selected_songs;
}


fn get_devices() -> rodio::Device {
    // List available devices for output
    println!("Available devices:");
    let devices:Vec<rodio::Device> = cpal::default_host().output_devices().unwrap().collect();

    for (i, device) in devices.iter().enumerate() {
        println!(" {}: {}", i, device.name().unwrap());
    }

    // Get user choice
    println!("Input choice: ");

    loop {
        let mut choice_input = String::new();
        io::stdin().read_line(&mut choice_input).expect("Please provide an input");

        match choice_input.trim().parse::<usize>() {
            Ok(i) => {
                // Return result
                match devices.get(i) {
                    Some(_device) => {
                        // Doing a warcrime because i cannot figure out borrow checking atm
                        let hack_device = cpal::default_host().output_devices().unwrap().nth(i).unwrap();
                        println!("Selected \"{}\"", hack_device.name().unwrap());

                        return hack_device;
                    },
                    None => {
                        println!("Invalid device index");
                        continue;
                    },
                };
            },

            Err(..) => {
                println!("Invalid input");
                continue;
            }
        }
    }
}


fn pipe_audio(output_device: &rodio::Device, output_file_path: PathBuf) {
    // Get a output stream handle to the output physical sound device
    let (_stream, stream_handle) = OutputStream::try_from_device(&output_device).unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    // Load a sound from a file, using a path relative to Cargo.toml
    let file = io::BufReader::new(File::open(output_file_path.clone()).unwrap());
    // Decode that sound file into a source
    let source = Decoder::new(file).unwrap();

    // Play audio and wait
    println!("[{}] Playing {}...", Local::now().format("%H:%M"), output_file_path.file_name().unwrap().to_string_lossy());
    sink.append(source);
    sink.sleep_until_end();
}


fn main() {
    let args = InputArgs::parse();

    // Prase read range
    let track_range = parse_track_ranges(&args.track_select);

    // Get album song paths
    println!("Album contents to be played:");
    let song_paths = get_audio_paths(&args.album_path, track_range);

    if args.delay > 0 {
        println!("{} second delay in-between tracks", args.delay);
    }

    // Get audio device
    println!("");
    let output_device = get_devices();

    for song in song_paths {
        pipe_audio(&output_device, song);

        // Apply audio playback delay
        if args.delay > 0 {
            let n_seconds = time::Duration::from_secs(args.delay.into());
            thread::sleep(n_seconds);
        }

    }

    println!("Done!");
}
