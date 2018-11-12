extern crate bus;
extern crate hound;
extern crate sample;


use self::bus::{BusReader};
use self::hound::WavReader;
use self::sample::{signal, Signal, Sample};
use self::sample::frame::Stereo;
use self::sample::interpolate::Linear;

type FramedSignal = signal::FromInterleavedSamplesIterator<std::vec::IntoIter<f32>, Stereo<f32>>;

// an audio track
pub struct AudioTrack {
  // command rx
  command_rx: BusReader<::midi::CommandMessage>,

  // original signal
  signal: FramedSignal,

  // iterator
  signal_it: Box<Iterator<Item = Stereo<f32>> + Send + 'static>,
}
impl AudioTrack {

  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> AudioTrack {
    AudioTrack {
      command_rx,
      signal: signal::from_interleaved_samples_iter(Vec::new()),
      signal_it: Box::new(sample::signal::equilibrium().until_exhausted())
    }
  }

  // load audio file
  pub fn load_file(&mut self, path: &str) {

    // load some audio
    let reader = WavReader::open(path).unwrap();

    // samples preparation
    let samples : Vec<f32> = reader.into_samples::<i16>().filter_map(Result::ok).map(i16::to_sample::<f32>).collect();

    // original signal, stereo framed, we keep it
    self.signal = signal::from_interleaved_samples_iter(samples);

    // for interpolation
    let interp = Linear::from_source(&mut self.signal);

    // iterator
    self.signal_it = Box::new(self.signal.clone().scale_hz(interp, 0.2).until_exhausted());
  }
}

// Implement `Iterator` for `AudioTrack`.
impl Iterator for AudioTrack {
    type Item = Stereo<f32>;

    // next!
    fn next(&mut self) -> Option<Self::Item> {
      // audio thread !!!
      match self.signal_it.next() {
        Some(frame) => {
          return Some(frame)
        },
        None => {
          // redo
          let interp = Linear::from_source(&mut self.signal);
          self.signal_it = Box::new(self.signal.clone().scale_hz(interp, 1.0).until_exhausted());
          return None
        }
      }
    }
}    