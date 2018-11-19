extern crate bus;
extern crate hound;
extern crate sample;

use self::bus::BusReader;
use self::hound::WavReader;
use self::sample::{signal, Sample, Frame};
use self::sample::frame::Stereo;

use audio::track_utils;
use audio::analytics;

// a slicer track
pub struct SlicedAudioTrack {
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
  // filter bank
  // filter_bank: BiquadFilter
}
impl SlicedAudioTrack {

  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> SlicedAudioTrack {
    SlicedAudioTrack {
      command_rx,
      original_tempo: 120.0,
      playback_rate: 1.0,
      playing: false,
      volume: 0.5,
      samples: Vec::new(),
    }
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
    let orig_tempo = track_utils::parse_original_tempo(path, self.samples.len());
    self.original_tempo = orig_tempo;

    // send for analytics
    analytics::detect_onsets(self.samples.clone());
  }

   // returns a buffer insead of frames one by one
  pub fn next_block(&mut self, size: usize) -> Vec<Stereo<f32>> {

    let dummy_vec : Vec<Stereo<f32>> = (0..size).map(|_it| Stereo::<f32>::equilibrium()).collect();
    /*
     * HERE WE CAN PROCESS BY CHUNK
     */
    // send full buffer
    return dummy_vec;
  }
}