#![allow(dead_code)]
extern crate cpal;
extern crate sample;
extern crate crossbeam_channel;

mod filters;
mod mixer;

use self::cpal::{EventLoop, SampleFormat, StreamData, UnknownTypeOutputBuffer};
use self::sample::frame::Stereo;
use self::sample::ToFrameSliceMut;
use control::ControlMessage;
use std::thread;

use config::Config;

/// Loudness, per block
pub fn loudness(block_out: &[Stereo<f32>]) -> f32 {
    let mut sum = 0.0f32;
    for s in block_out {
        let a = (s[0] + s[1]) / 2.0;
        sum += a * a;
    }

    let rms = f32::sqrt(sum / block_out.len() as f32);
    //  let decibel = 20.0 * f32::log10(rms);
    return rms;
}

/// Initialize audio machinery
pub fn initialize_audio(
    conf: Config,
    hub_rx: crossbeam_channel::Receiver<ControlMessage>,
) -> thread::JoinHandle<()> {
    // init mixer
    let mut mixer = mixer::AudioMixer::new(conf, hub_rx);

    // enumerate all devices
    //  enumerate_all_devices();

    // init audio with CPAL !
    // creates event loop
    let event_loop = EventLoop::new();

    // audio out device
    let device = cpal::default_output_device().expect("audio: no output device available");

    // get the current default out format
    let mut format = device
        .default_output_format()
        .expect("should have a default format");

    if format.sample_rate != cpal::SampleRate(44100) {
        println!("Unsupported Device SampleRate, should be 44100");
        ::std::process::exit(1);
    }

    // force the sample rate
    format.sample_rate = cpal::SampleRate(44100);

    // force the number of channel
    format.channels = 2;

    // display some info
    println!("audio device: {}", device.name());
    println!("audio: Fixed OUTPUT Samplerate: {}", format.sample_rate.0);

    match format.data_type {
        SampleFormat::U16 => println!("audio: Supported sample type is U16"),
        SampleFormat::I16 => println!("audio: Supported sample type is I16"),
        SampleFormat::F32 => println!("audio: Supported sample type is F32"),
    }

    // creates the stream
    let stream_id = event_loop
        .build_output_stream(&device, &format, &mut cpal::BufferSize::Fixed(128))
        .unwrap();

    // add stream
    event_loop.play_stream(stream_id);

//    let mut _max_rms = 0.0;

    // initialize in its own thread
    let audio_thread = thread::spawn(move || {
        // audio callback
        event_loop.run(move |_stream_id, stream_data| {
            match stream_data {
                StreamData::Output {
                    buffer: UnknownTypeOutputBuffer::F32(mut buffer),
                } => {
                    // here we implement the trait sample::ToFrameSliceMut;
                    // we can take a mutable buffer from the audio callback, but framed in stereo !!
                    let buffer: &mut [Stereo<f32>] = buffer.to_frame_slice_mut().unwrap();

                    // write audio from the mixer
                    mixer.next_block(buffer);

                    // calculate output volume
                    // let loud = loudness(buffer);
                    // if loud > max_rms {
                    //  max_rms = loud;
                    // }
                    // println!("lourd {}", max_rms);
                }
                _ => (),
            }
        });
    });

    // return handle
    audio_thread
}

// enumerate devices
fn enumerate_all_devices() {
    let devices = cpal::devices();
    println!("Devices: ");
    for (device_index, device) in devices.enumerate() {
        println!("{}. \"{}\"", device_index + 1, device.name());

        // Input formats
        if let Ok(fmt) = device.default_input_format() {
            println!("  Default input stream format:\n    {:?}", fmt);
        }
        let mut input_formats = match device.supported_input_formats() {
            Ok(f) => f.peekable(),
            Err(e) => {
                println!("Error: {:?}", e);
                continue;
            }
        };
        if input_formats.peek().is_some() {
            println!("  All supported input stream formats:");
            for (format_index, format) in input_formats.enumerate() {
                println!(
                    "    {}.{}. {:?}",
                    device_index + 1,
                    format_index + 1,
                    format
                );
            }
        }

        // Output formats
        if let Ok(fmt) = device.default_output_format() {
            println!("  Default output stream format:\n    {:?}", fmt);
        }
        let mut output_formats = match device.supported_output_formats() {
            Ok(f) => f.peekable(),
            Err(e) => {
                println!("Error: {:?}", e);
                continue;
            }
        };
        if output_formats.peek().is_some() {
            println!("  All supported output stream formats:");
            for (format_index, format) in output_formats.enumerate() {
                println!(
                    "    {}.{}. {:?}",
                    device_index + 1,
                    format_index + 1,
                    format
                );
            }
        }
    }
}
