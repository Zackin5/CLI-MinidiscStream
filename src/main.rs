use std::{io, fs, path::PathBuf, fs::File, ffi::OsStr, time, thread};
use rodio::{Decoder, OutputStream, Sink, cpal::{self, traits::HostTrait, traits::DeviceTrait}};
use clap::Parser;


fn get_audio_paths(album_path: &String) -> Vec<PathBuf> {
    let valid_exts = vec![OsStr::new("mp3"), OsStr::new("wav"), OsStr::new("flac")];

    let mut song_paths: Vec<PathBuf> = Vec::new();
    for item in fs::read_dir(album_path).expect("Failed to read path") {
        if let Ok(item) = item {
            if item.path().extension().is_some() && valid_exts.contains(&item.path().extension().unwrap()) {
                println!("{:?}", item.path());
                song_paths.push(item.path())
            }
        }
    }

    return song_paths;
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
    println!("Playing {}...", output_file_path.file_name().unwrap().to_string_lossy());
    sink.append(source);
    sink.sleep_until_end();
}


/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct InputArgs {
    /// Album directory to play files from
    #[arg(short, long)]
    album_path: String,

    /// Delay between songs (useful if not using a digital interface)
    #[arg(short, long, default_value_t = 0)]
    delay: u8,
}


fn main() {
    let args = InputArgs::parse();

    // Get album song paths
    println!("Album contents to be played:");
    let song_paths = get_audio_paths(&args.album_path);

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
