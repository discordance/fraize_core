extern crate hound;
extern crate cpal;

use self::hound::WavReader;
use self::cpal::{SampleFormat, StreamData, EventLoop, UnknownTypeOutputBuffer};

// initialize audio machinery
pub fn initialize_audio() {

  // load some audio
  let mut reader = WavReader::open("/Users/nunja/Documents/Audiolib/smplr/loop16.wav").unwrap();

  // creates event loop
  let event_loop = EventLoop::new();

  // audio out device
  let device = cpal::default_output_device().expect("audio: no output device available");

  // supported formats is an iterator
  let mut supported_formats_range = device.supported_output_formats()
    .expect("audio: error while querying formats");
  
  let format = supported_formats_range.next()
    .expect("audio: No supported format.")
    .with_max_sample_rate();

  // display some info
  println!("audio: Default OUTPUT Samplerate: {}", format.sample_rate.0);
  match format.data_type {
    SampleFormat::U16 => println!("audio: Sample Type is U16"),
    SampleFormat::I16 => println!("audio: Sample Type is I16"),
    SampleFormat::F32 => println!("audio: Sample Type is F32")
  } 

  // samples are an iterator
  let samples = reader.samples::<i16>();
  
  // buffering the samples in memory
  let smpl_buffer: Vec<_> = samples.map(|x| match x {
    Ok(sample) => {
      let max = i16::max as i16;
      sample as f32 / max as f32
    },
    _ => 0.0
  }).collect();

  // here is magic, make the iter cyclable !!!
  let mut buffer_iter = smpl_buffer.iter().cloned().cycle();

  // creates the stream
  let stream_id = event_loop.build_output_stream(&device, &format).unwrap();

  // add stream
  event_loop.play_stream(stream_id);

  // audio callback
  event_loop.run(move |_stream_id, stream_data| {
      match stream_data {
          StreamData::Output { buffer: UnknownTypeOutputBuffer::F32(mut buffer) } => {
              for elem in buffer.iter_mut() {
                  match buffer_iter.next() {
                    Some(sample) => {
                      *elem = sample * 0.5;
                    },
                    None => {
                      *elem = 0.0; // finish
                    }
                  }
              }
          },
          _ => (),
      }
  });
}
