
use std::io;
use std::rc::Rc;
use std::fs::File;
use std::io::BufReader;
use rodio::{Decoder, OutputStream, Sink, cpal::{self, traits::HostTrait, traits::DeviceTrait}};

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
                        // Doing a warcrime becuase i cannot figure out borrow checking atm
                        return cpal::default_host().output_devices().unwrap().nth(i).unwrap();
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

fn main() {
    println!("Starting...");

    let output_device = get_devices();
    println!("{}", output_device.name().unwrap());

    // Get a output stream handle to the default physical sound device
    let (_stream, stream_handle) = OutputStream::try_from_device(&output_device).unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    // Load a sound from a file, using a path relative to Cargo.toml
    let file = BufReader::new(File::open("D:\\zachi\\Music\\Deftones - Around The Fur\\11 - Deftones - Bong Hit.mp3").unwrap());
    // Decode that sound file into a source
    let source = Decoder::new(file).unwrap();

    sink.append(source);
    sink.sleep_until_end();

    println!("Done!");
}
