#![allow(dead_code)]

extern crate bus;
extern crate cpal;
extern crate sample;

mod mixer;
mod filters;

use self::bus::BusReader;
use self::cpal::{EventLoop, SampleFormat, StreamData, UnknownTypeOutputBuffer};
use self::sample::frame::{Stereo};
use self::sample::ToFrameSliceMut;

// initialize audio machinery
pub fn initialize_audio(midi_rx: BusReader<::control::ControlMessage>) {

  // test some mixer 
  let mut mixer = mixer::AudioMixer::new_test(midi_rx);

  // init audio with CPAL !
  // creates event loop
  let event_loop = EventLoop::new();

  // audio out device
  let device = cpal::default_output_device().expect("audio: no output device available");

  // supported formats is an iterator
  let mut supported_formats_range = device
    .supported_output_formats()
    .expect("audio: error while querying formats");

  let mut format = supported_formats_range
    .next()
    .expect("audio: No supported format.")
    .with_max_sample_rate();

  // force the sample rate
  format.sample_rate = cpal::SampleRate(44100);

  // display some info
  println!("audio device: {}", device.name());
  println!("audio: Fixed OUTPUT Samplerate: {}", format.sample_rate.0);

  match format.data_type {
    SampleFormat::U16 => println!("audio: Supported sample type is U16"),
    SampleFormat::I16 => println!("audio: Supported sample type is I16"),
    SampleFormat::F32 => println!("audio: Supported sample type is F32"),
  }

  // creates the stream
  let stream_id = event_loop.build_output_stream(&device, &format, &mut cpal::BufferSize::Fixed(128)).unwrap();

  // add stream
  event_loop.play_stream(stream_id);

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
      }
      _ => (),
    }
  });
}
