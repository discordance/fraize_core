extern crate bus;
extern crate hound;
extern crate sample;

use self::bus::BusReader;
use self::hound::WavReader;
use self::sample::frame::Stereo;
use self::sample::interpolate::Linear;
use self::sample::{signal, Sample, Signal, Frame};


type FramedSignal = signal::FromInterleavedSamplesIterator<std::vec::IntoIter<f32>, Stereo<f32>>;

// an audio track
pub struct AudioTrack {
  // commands rx
  command_rx: BusReader<::midi::CommandMessage>,
  // original tempo of the loaded audio
  original_tempo: f64,
  // the track is playring ?
  playing: bool,
  // volume of the track
  volume: f32,
  // how many frames have passed
  elasped_frames: i64,
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
      original_tempo: 120.0,
      playing: false,
      volume: 0.5,
      elasped_frames: 0,
      signal: signal::from_interleaved_samples_iter(Vec::new()),
      signal_it: Box::new(sample::signal::equilibrium().until_exhausted()),
    }
  }

  // load audio file
  pub fn load_file(&mut self, path: &str) {
    // load some audio
    let reader = WavReader::open(path).unwrap();

    // samples preparation
    let samples: Vec<f32> = reader
      .into_samples::<i16>()
      .filter_map(Result::ok)
      .map(i16::to_sample::<f32>)
      .collect();

    // original signal, stereo framed, we keep it
    self.signal = signal::from_interleaved_samples_iter(samples);
  }

  // reloop
  fn reloop(&mut self) {
    // reset frame count
    self.elasped_frames = 0;

    // for interpolation
    let interp = Linear::from_source(&mut self.signal);
    // iterator
    self.signal_it = Box::new(self.signal.clone().scale_hz(interp, 0.9).until_exhausted());
  }

  // fetch commands from rx
  fn fetch_commands(&mut self){
    match self.command_rx.try_recv() {
      Ok(command) => {
        match command {
          ::midi::CommandMessage::Playback(playback_message) => {
            match playback_message.sync {
              ::midi::SyncMessage::Start() => {
                self.reloop();
                self.playing = true;
              },
              ::midi::SyncMessage::Stop() => {
                self.playing = false;
                self.reloop();
              },
              _ => ()
            }
          }
        }
      },
      _ => ()
    };
  }
}

// Implement `Iterator` for `AudioTrack`.
impl Iterator for AudioTrack {
  type Item = Stereo<f32>;

  // next!
  fn next(&mut self) -> Option<Self::Item> {
    
    // non blocking command fetch
    self.fetch_commands();
    
    // doesnt consume if not playing
    if !self.playing{
      return Some(Stereo::<f32>::equilibrium());
    }
    // audio thread !!!
    match self.signal_it.next() {
      Some(frame) => {
         self.elasped_frames += 1;
         return Some(frame.scale_amp(self.volume));
      },
      None => {
        // init
        self.reloop();
        return None;
      }
    }
  }
}
