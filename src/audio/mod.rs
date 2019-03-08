#![allow(dead_code)]

extern crate bus;
extern crate cpal;
extern crate sample;

mod repitch_track;
mod sliced_track;
mod pvoc_track;

mod analytics;
mod track_utils;
mod filters;

use self::bus::BusReader;
use self::cpal::{EventLoop, SampleFormat, StreamData, UnknownTypeOutputBuffer};
use self::sample::frame::{Frame, Stereo};
use self::sample::ToFrameSliceMut;
use self::repitch_track::RepitchAudioTrack;
use self::sliced_track::SlicedAudioTrack;
use self::pvoc_track::PvocAudioTrack;

// initialize audio machinery
pub fn initialize_audio(midi_rx: BusReader<::midi::CommandMessage>) {

  // init our beautiful test audiotrack
  // let mut track = PvocAudioTrack::new(midi_rx);
  // track.load_file("/Users/nunja/Documents/Audiolib/smplr/tech_16.wav");

  // test sliced
  let mut track = SlicedAudioTrack::new(midi_rx);
  // track.load_file("/Users/nunja/Documents/Audiolib/smplr/loop_8.wav");
  track.load_file("/Users/nunja/Documents/Audiolib/smplr/tech_16.wav");

  // let mut track = RepitchAudioTrack::new(midi_rx);
  // track.load_file("/Users/nunja/Documents/Audiolib/smplr/tech_16.wav");
  // track.load_file("/Users/nunja/Documents/Audiolib/smplr/loop_8.wav");

  // init audio with CPAL !
  // creates event loop
  let event_loop = EventLoop::new();

  // audio out device
  let device = cpal::default_output_device().expect("audio: no output device available");

  // supported formats is an iterator
  let mut supported_formats_range = device
    .supported_output_formats()
    .expect("audio: error while querying formats");

  let format = supported_formats_range
    .next()
    .expect("audio: No supported format.")
    .with_max_sample_rate();

  // display some info
  println!("audio: Default OUTPUT Samplerate: {}", format.sample_rate.0);
  match format.data_type {
    SampleFormat::U16 => println!("audio: Supported sample type is U16"),
    SampleFormat::I16 => println!("audio: Supported sample type is I16"),
    SampleFormat::F32 => println!("audio: Supported sample type is F32"),
  }

  // creates the stream
  let stream_id = event_loop.build_output_stream(&device, &format).unwrap();

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
        // audio tracks can be requested by block of buffer len
        let size = buffer.len();

        // let re = &sliced_track;
        // sliced_track;
        let next_block = track.next_block(size);
        // // create a mutable iterator
        let mut it = next_block.iter();
        // // inject the frames out
        for out_frame in buffer {
          // *out_frame = Stereo::<f32>::equilibrium(); // DEBUG
          match it.next() {
            Some(frame) => *out_frame = *frame,
            None => {
              *out_frame = Stereo::<f32>::equilibrium();
            }
          }
        }
      }
      _ => (),
    }
  });
}
