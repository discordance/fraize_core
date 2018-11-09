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
  samples_iter: Box<Iterator<Item = f32> + Send>,
}
impl AudioTrack {

  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> AudioTrack {
    let is: Vec<f32> = Vec::new();
    let it = is.into_iter();
    AudioTrack {
      command_rx,
      samples_iter: Box::new(it)
    }
  }

  // load audio file
  pub fn load_file(&mut self, path: &str) {

    // load some audio
    let reader = WavReader::open(path).unwrap();

    // samples are an iterator
    // Read the interleaved samples and convert them to a signal.
    let samples: Vec<f32> = reader.into_samples::<i16>().filter_map(Result::ok).map(i16::to_sample::<f32>).collect();
    self.samples_iter = Box::new(samples.into_iter().cycle());
  }
}

// Implement `Iterator` for `AudioTrack`.
impl Iterator for AudioTrack {
    type Item = f32;

    // next!
    fn next(&mut self) -> Option<Self::Item> {
      match self.samples_iter.next() {
        Some(sample) => {
          return Some(sample)
        },
        None => {
          return None
        }
      }
    }
}    