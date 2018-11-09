extern crate bus;
extern crate hound;
extern crate sample;


use self::bus::{BusReader};
use self::hound::WavReader;
use self::sample::{signal, Signal, Frame, Sample};
use self::sample::frame::Stereo;

// an audio track
pub struct AudioTrack {
  // command rx
  command_rx: BusReader<::midi::CommandMessage>,

  // iterator
  signal: Box<Iterator<Item = Stereo<f32>> + Send>,
}
impl AudioTrack {

  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> AudioTrack {
    AudioTrack {
      command_rx,
      signal: Box::new(sample::signal::equilibrium().until_exhausted())
    }
  }

  // load audio file
  pub fn load_file(&mut self, path: &str) {

    // load some audio
    let reader = WavReader::open(path).unwrap();

    // samples are an iterator
    // Read the interleaved samples and convert them to a signal.
    let samples: Vec<f32> = reader.into_samples::<i16>().filter_map(Result::ok).map(i16::to_sample::<f32>).collect();
    self.signal = Box::new(signal::from_interleaved_samples_iter(samples).until_exhausted().cycle());
  }
}

// Implement `Iterator` for `AudioTrack`.
impl Iterator for AudioTrack {
    type Item = Stereo<f32>;

    // next!
    fn next(&mut self) -> Option<Self::Item> {
      // audio thread !!!
      match self.signal.next() {
        Some(sample) => {
          return Some(sample)
        },
        None => {
          return None
        }
      }
    }
}    