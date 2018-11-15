extern crate bus;
extern crate hound;
extern crate sample;
extern crate time_calc;
extern crate num;


use std::path::Path;

use self::num::ToPrimitive;
use self::bus::BusReader;
use self::hound::WavReader;
use self::sample::frame::Stereo;
use self::sample::interpolate::Linear;
use self::sample::{signal, Frame, Sample, Signal};
use self::time_calc::{Samples};

// @TODO this is ugly but what to do without generics ?
type FramedSignal = signal::FromInterleavedSamplesIterator<std::vec::IntoIter<f32>, Stereo<f32>>;

// helper that parses the number of beats of an audio sample in the filepath
// @TODO Way to much unwarp here
fn parse_filepath_beats(path: &str) -> i16 {
  // compute path
  let path_obj = Path::new(path);
  let file_stem = path_obj.file_stem().unwrap();
  let file_stem = file_stem.to_str().unwrap();
  let split = file_stem.split("_");
  let split: Vec<&str> = split.collect();
  let beats = split[1].parse::<i16>().unwrap();
  return beats;
}

fn parse_original_tempo(path: &str, num_samples: usize) -> f64 {
  // compute number of beats
  let beats = parse_filepath_beats(path);
  let ms = Samples((num_samples as i64)/2).to_ms(44_100.0);
  let secs = ms.to_f64().unwrap()/1000.0;
  return 60.0/(secs/beats as f64)
}

// an audio track
pub struct AudioTrack {
  // commands rx
  command_rx: BusReader<::midi::CommandMessage>,
  // original tempo of the loaded audio
  original_tempo: f64,
  // playback_rate to match original_tempo
  playback_rate: f64,  
  // the track is playring ?
  playing: bool,
  // volume of the track
  volume: f32,
  // how many frames have passed
  elasped_frames: u64,
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
      playback_rate: 1.0,
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

    // parse and set original tempo
    let orig_tempo = parse_original_tempo(path, samples.len());
    self.original_tempo = orig_tempo;

    // original signal, stereo framed, we keep it
    self.signal = signal::from_interleaved_samples_iter(samples);

    // reloop to avoid clicks
    self.reloop();
  }

  // change playback speed
  fn respeed(&mut self) {
    // for interpolation
    let interp = Linear::from_source(&mut self.signal);
    // iterator
    println!("hehe {}", self.elasped_frames);
    //(self.elasped_frames as f64 * self.playback_rate) as usize
    self.signal_it = Box::new(self.signal.clone().scale_hz(interp, self.playback_rate).until_exhausted().skip((self.elasped_frames as f64 * self.playback_rate) as usize));
  }

  // reloop
  fn reloop(&mut self) {
    // reset frame count
    self.elasped_frames = 0;
    // for interpolation
    let interp = Linear::from_source(&mut self.signal);
    // iterator
    self.signal_it = Box::new(self.signal.clone().scale_hz(interp, self.playback_rate).until_exhausted());
  }

  // fetch commands from rx
  fn fetch_commands(&mut self) {
    match self.command_rx.try_recv() {
      Ok(command) => match command {
        ::midi::CommandMessage::Playback(playback_message) => match playback_message.sync {
          ::midi::SyncMessage::Start() => {
            self.reloop();
            self.playing = true;
          }
          ::midi::SyncMessage::Stop() => {
            self.playing = false;
            self.reloop();
          }
          ::midi::SyncMessage::Tick(tick) => {
            let rate = playback_message.time.tempo/self.original_tempo;
            // changed tempo
            if self.playback_rate != rate {
              // println!("changed {}", playback_message.time.tempo);
              self.playback_rate = rate;
              self.respeed();
            }
          },
        },
      },
      _ => (),
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
    if !self.playing {
      return Some(Stereo::<f32>::equilibrium());
    }
    // audio thread !!!
    match self.signal_it.next() {
      Some(frame) => {
        self.elasped_frames += 1;
        return Some(frame.scale_amp(self.volume));
      }
      None => {
        // init
        self.reloop();
        return None;
      }
    }
  }
}
