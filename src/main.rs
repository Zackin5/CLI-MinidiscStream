use std::{io, fs, path::{PathBuf, Path}, fs::{File, read_to_string}, ffi::OsStr, time, thread, ops::Range, cmp};
use rodio::{Decoder, OutputStream, Sink, SpatialSink, cpal::{self, traits::HostTrait, traits::DeviceTrait}};
use clap::Parser;
use chrono::{Local, Duration};

/// Simple program to play a folder of songs to an audio device
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct InputArgs {
    /// Album directory or playlist to play files from
    #[arg(short, long)]
    input_path: String,

    /// Pause duration between songs. Useful to automatically add track markers on Minidisc, rec 3 seconds
    #[arg(short, long, default_value_t = 0.0)]
    pause: f32,

    /// Pre-playback start delay. Useful for syncing the 5 second blank start on cassette tapes
    #[arg(short, long, default_value_t = 0.0)]
    delay: f32,

    /// Pan modifier for audio playback. Useful for countering bad recording channel balance. Negative values are left-bias, positive right.
    #[arg(short, long, default_value_t = 0.0, allow_hyphen_values = true)]
    stereo_pan: f32,

    /// Track range selector. Use {skip_n}:{take_n}. Either {} can be empty. {take_n} supports negative values to count from end
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

fn parse_playlist(path: &PathBuf, valid_audio_exts: &Vec<&OsStr>) -> Vec<PathBuf> {
    let mut result = Vec::new();

    for line in read_to_string(path).unwrap().lines() {
        // Skip comments
        if line.starts_with("#") {
            continue;
        }

        // Check if file exists
        let file_path = Path::new(line);

        if !file_path.exists() {
            println!("Failed to find file \"{}\" from playlist", line);
            continue;
        }

        // Check if file is an audio file
        if !valid_audio_exts.contains(&file_path.extension().unwrap()) {
            println!("Playlist file \"{}\" is not a supported audio type", line);
            continue;
        }

        // Store file and continue
        result.push(file_path.to_path_buf());
    }

    return result;
}


fn get_audio_paths(album_path: &String, track_range: std::ops::Range<Option<isize>>) -> Vec<PathBuf> {
    let valid_audio_exts = vec![OsStr::new("mp3"), OsStr::new("wav"), OsStr::new("flac")];  // Filter list for file selector
    let playlist_exts = vec![OsStr::new("m3u8")];  // Valid playlists to load

    // Convert track ranges to valid counters
    let lower_bound = match track_range.start {
        Some(i) => i,
        None => 0
    };

    let mut upper_bound = match track_range.end {
        Some(i) => i,
        None => 0
    };

    // Check if provided path is a file, handle special logic if it is
    let attr = fs::metadata(album_path).expect("Failed to read path");
    if attr.is_file() {
        let file_path_buf = PathBuf::from(album_path);
        let file_ext = &file_path_buf.extension().unwrap();

        // Return a single audio file if passed
        if valid_audio_exts.contains(file_ext) {
            return vec![file_path_buf];
        }

        // Parse a playlist if passed
        if playlist_exts.contains(file_ext) {
            return parse_playlist(&file_path_buf, &valid_audio_exts);
        }

        panic!("Unsupported extension \"{}\" for input file \"{}\"", file_ext.to_str().unwrap(), album_path)
    }


    // Read contents of a directory
    let mut folder_song_contents: Vec<PathBuf> = Vec::new();  // Result list
    for item in fs::read_dir(album_path).expect("Failed to read path") {
        if let Ok(item) = item {
            if item.path().extension().is_some() && valid_audio_exts.contains(&item.path().extension().unwrap()) {
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


fn pipe_audio(output_device: &rodio::Device, output_file_path: PathBuf, stereo_pan: f32) {
    // Get a output stream handle to the output physical sound device
    let (_stream, stream_handle) = OutputStream::try_from_device(&output_device).unwrap();
    
    // Load a sound from a file, using a path relative to Cargo.toml
    let file = io::BufReader::new(File::open(output_file_path.clone()).unwrap());
    // Decode that sound file into a source
    let source = Decoder::new(file).unwrap();
    
    // Play audio and wait
    println!("[{}] Playing {}...", Local::now().format("%I:%M %p"), output_file_path.file_name().unwrap().to_string_lossy());
    if stereo_pan.abs() <= f32::EPSILON {
        // Un-panned audio
        let sink = Sink::try_new(&stream_handle).unwrap();

        sink.append(source);
        sink.sleep_until_end();
    }
    else {
        // Panned audio
        let pan_postition = [stereo_pan, 0.0, 0.0];

        let pan_sink = SpatialSink::try_new(&stream_handle, 
            pan_postition,
            [-1.0, 0.0, 0.0], 
            [1.0, 0.0, 0.0])
            .unwrap();

            pan_sink.append(source);
            pan_sink.sleep_until_end();
    }
}


fn println_end_time(duration: i64) {
    let endtime_delta = Local::now() + Duration::minutes(duration);
    println!("Will end at [{}] for [+{}m]", endtime_delta.format("%I:%M %p"), duration);
}


fn main() {
    let args = InputArgs::parse();

    // Prase read range
    let track_range = parse_track_ranges(&args.track_select);

    // Get album song paths
    println!("Album contents to be played:");
    let song_paths = &get_audio_paths(&args.input_path, track_range);

    for path in song_paths {
        println!("{:?}", path);
    }

    if args.pause > 0.0 {
        println!("{} second delay in-between tracks", args.pause);
    }

    // Get audio device
    println!("");
    let output_device = get_devices();

    println!("");
    // Apply playback pause
    if args.delay > 0.0 {
        println!("Waiting {} seconds...", args.delay);

        let n_seconds = time::Duration::from_secs_f32(args.delay.into());

        thread::sleep(n_seconds);
    }

    println_end_time(32);
    println_end_time(45);
    println_end_time(74);
    println_end_time(80);

    // Play songs
    for song in song_paths {
        pipe_audio(&output_device, song.to_path_buf(), args.stereo_pan);

        // Apply audio playback pause delay
        if args.pause > 0.0 {
            println!("Waiting {} seconds...", args.pause);

            let n_seconds = time::Duration::from_secs_f32(args.pause.into());
            thread::sleep(n_seconds);
        }

    }

    println!("[{}] Done!", Local::now().format("%H:%M"));
}
