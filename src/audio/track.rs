extern crate bus;
extern crate elapsed;
extern crate hound;
extern crate num;
extern crate sample;
extern crate time_calc;

use std::path::Path;

use self::bus::BusReader;
use self::hound::WavReader;
use self::num::ToPrimitive;
use self::sample::frame::Stereo;
use self::sample::interpolate::{Converter, Linear};
use self::sample::{signal, Frame, Sample, Signal};
use self::time_calc::Samples;

use audio::filters::{FilterOp, FilterType, BiquadFilter};

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
  let ms = Samples((num_samples as i64) / 2).to_ms(44_100.0);
  let secs = ms.to_f64().unwrap() / 1000.0;
  return 60.0 / (secs / beats as f64);
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
  // original samples
  samples: Vec<f32>,
  // iterator / converter
  sample_converter: Converter<FramedSignal, Linear<Stereo<f32>>>,
  // filter bank
  filter_bank: BiquadFilter
}
impl AudioTrack {
  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> AudioTrack {
    
    // init dummy
    let mut signal = signal::from_interleaved_samples_iter::<Vec<f32>, Stereo<f32>>(Vec::new());
    let interp = Linear::from_source(&mut signal);
    let conv = signal.scale_hz(interp, 1.0);

    // filter
    let filter = BiquadFilter::create_filter(
      FilterType::LowPass(),
      FilterOp::UseQ(),
      44_100.0, // rate
      1000.0, // cutoff
      1.0, // db gain
      1.0, // q
      1.0, // bw
      1.0 //slope
    );

    AudioTrack {
      command_rx,
      original_tempo: 120.0,
      playback_rate: 1.0,
      playing: false,
      volume: 0.5,
      samples: Vec::new(),
      sample_converter: conv,
      filter_bank: filter
    }
  }

  // returns a buffer insead of frames one by one
  pub fn next_block(&mut self, size: usize) -> Vec<Stereo<f32>> {
    let mut audio_buffer = self.take(size).collect();
    // process this malaka
    return audio_buffer;
  }

  // load audio file
  pub fn load_file(&mut self, path: &str) {
    // load some audio
    let reader = WavReader::open(path).unwrap();

    // samples preparation
    self.samples = reader
      .into_samples::<i16>()
      .filter_map(Result::ok)
      .map(i16::to_sample::<f32>)
      .collect();

    // parse and set original tempo
    let orig_tempo = parse_original_tempo(path, self.samples.len());
    self.original_tempo = orig_tempo;

    // reloop to avoid clicks
    self.reloop();
  }

  // change playback speed
  fn respeed(&mut self) {
    self
      .sample_converter
      .set_sample_hz_scale(1.0 / self.playback_rate);
  }

  // reloop rewind the conv
  fn reloop(&mut self) {
    // cook it
    // efficent way to copy !??
    let mut signal =
      signal::from_interleaved_samples_iter::<Vec<f32>, Stereo<f32>>(self.samples.clone());

    // for interpolation
    let interp = Linear::from_source(&mut signal);

    let scaled = signal.scale_hz(interp, self.playback_rate);
    self.sample_converter = scaled;
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
            let rate = playback_message.time.tempo / self.original_tempo;
            // changed tempo
            if self.playback_rate != rate {
              self.playback_rate = rate;
              self.respeed();
            }
          }
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

    // check if is exhansted
    if self.sample_converter.is_exhausted() {
      self.reloop();
      return Some(Stereo::<f32>::equilibrium());
    }

    // else next
    let frame = self.sample_converter.next();

    // filter pass
    return Some(self.filter_bank.process(frame));
  }
}
