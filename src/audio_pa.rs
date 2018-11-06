extern crate hound;
extern crate portaudio;
extern crate bus;

use std::thread;
use std::time::Duration;

use self::bus::Bus;
use self::portaudio as pa;
use self::hound::WavReader;

const CHANNELS: i32 = 2;
const SAMPLE_RATE: f64 = 44_100.0;
const FRAMES_PER_BUFFER: u32 = 512;

// initialize audio machinery
pub fn initialize_audio() {
  
  // bus channel
  let mut bus = Bus::new(1);
  let mut rx = bus.add_rx();

  // test thread
  thread::spawn(move || {
    bus.broadcast(1);
    thread::sleep(Duration::from_secs(5)); // block for two seconds
    bus.broadcast(2);
  });  

  // load some audio
  let mut reader = WavReader::open("/Users/nunja/Documents/Audiolib/smplr/loop16.wav").unwrap();

  // samples are an iterator
  let samples = reader.samples::<i16>();
  
  // buffering the samples in memory
  let smpl_buffer: Vec<f32> = samples.map(|x| match x {
    Ok(sample) => {
      sample as f32 / std::i16::MAX as f32
    },
    _ => 0.0
  }).collect();

  // here is magic, make the iter cyclable !!!
  let mut buffer_iter = smpl_buffer.iter().cycle();

  // initialize
  let pa = pa::PortAudio::new()
    .expect("audio: Unable to start Port Audio");

  // pa settings
  let settings: pa::OutputStreamSettings<f32> = pa.default_output_stream_settings(CHANNELS, SAMPLE_RATE, FRAMES_PER_BUFFER)
    .expect("audio: Unable to configure settings");


  // We'll use this function to wait for read/write availability.
  fn wait_for_stream<F>(f: F, name: &str) -> u32
      where F: Fn() -> Result<pa::StreamAvailable, pa::error::Error>
  {
      'waiting_for_stream: loop {
          match f() {
              Ok(available) => match available {
                  pa::StreamAvailable::Frames(frames) => return frames as u32,
                  pa::StreamAvailable::OutputUnderflowed => println!("audio: Output stream has underflowed"),
                  _ => (),
              },
              Err(err) => panic!("audio: An error occurred while waiting for the {} stream: {}", name, err),
          }
      }
  };

  let mut stream = pa.open_blocking_stream(settings).expect("audio: Couldnt open the stream");
  stream.start().expect("audio: Couldn't start the stream");

  // now start the main read/write loop!
  'stream: loop {

    match rx.try_recv() {
      Ok(num) => {
        if num == 2 {
          buffer_iter = smpl_buffer.iter().cycle();
        }
      },
      _ => ()
    };

    // how many frames are available for writing on the output stream?
    let out_frames = wait_for_stream(|| stream.write_available(), "Write");

    if out_frames > 0 {
      let n_write_samples = out_frames as usize * CHANNELS as usize;
      stream.write(out_frames, |output| {
        for i in 0..n_write_samples {
            match buffer_iter.next() {
              Some(sample) => {
                output[i] = sample * 0.5;
              },
              None => {
                output[i] = 0.0; // finish
              }
            }
        }
      }).expect("Stream Write Error");
    } else {
      pa.sleep(1);
    }
  }
  // pa.sleep(10 * 1_000);

  // stream.stop().expect("audio: Couldnt stop the stream");
  // stream.close().expect("audio: Couldnt close the stream");

  // println!("ende");
}