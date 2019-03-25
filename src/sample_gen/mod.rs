//! A module for sample generators
//!
//! The `sample_gen` module is used to define sample generators
//! a Sample generator is a unit that produce samples, synced to a clock,
//! given a SmartBuffer using and custom sampling engine.
//! There is many ways to playback audio phrases and snap it to a clock (could be internal / external clock).
//! It invloves some form of time compression / expansion with respect to original tempo in which the phrase was recorded originally.
//!
//! Thoses are researched so far:
//! - RePitch uses a simple linear interpolation. Cubic and Quadratic don't worth the CPU cycles.
//! - Sliced acts more like a beat slicer à la ReCycle.
//! - PVoc uses TimeStretching from the Phase Vocoder implemented in Aubio.
extern crate hound;
extern crate sample;
extern crate time_calc;

// re-publish submodule repitch as a public module;
pub mod analytics;
pub mod gen_utils;
pub mod repitch;
pub mod slicer;
pub mod pvoc;

use self::hound::WavReader;
use self::sample::frame::Stereo;
use self::sample::{Frame, Sample};
use self::time_calc::Ppqn;
use std::collections::HashMap;

// pulse per quarter note
pub const PPQN: Ppqn = 24;

/// SliceMode defines how the slices are cut in a smart buffer.
/// Can be OnsetDetection or fixed BAR divisions.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SliceMode {
  /// Natural detected onsets.
  OnsetMode(),
  /// Quantized detected onsets.
  QonsetMode(),
  /// Bar / 4 precomputed divisions
  Bar4Mode(),
  /// Bar / 8 precomputed divisions
  Bar8Mode(),
  /// Bar / 16 precomputed divisions
  Bar16Mode(),
}

/// Basically an audio buffer (in frame format) with some metadata from analysis.
pub struct SmartBuffer {
  /// Samples in Stereo / float32 format. Use the `sample` Crate for convenience methods.
  /// We only support this format for the moment.
  /// @TODO implement as Generic ?
  frames: Vec<Stereo<f32>>,
  /// Original tempo of the audio phrase (if it's a phrase).
  original_tempo: f64,
  /// Number of beats analyzed in audio.
  num_beats: u64,
  /// Precomputed slices positions. Contains detected Onsets positions and fixed divisions.
  /// Useful in the slicer.
  slices: HashMap<SliceMode, Vec<u64>>,
}

/// Implementation
impl SmartBuffer {
  /// returns an empty SmartBuffer
  pub fn new_empty() -> Self {
    SmartBuffer {
      frames: Vec::new(),
      original_tempo: 120.0,
      num_beats: 4,
      slices: HashMap::<SliceMode, Vec<u64>>::with_capacity(4),
    }
  }

  /// Loads and analyse a wave file
  pub fn load_wave(&mut self, path: &str) -> Result<bool, &str> {
    // load some audio
    let reader = match WavReader::open(path) {
      Ok(r) => r,
      Err(err) => {
        println!("Load_wave error {}", err);
        return Err(concat!("UnreadablePath"));
      }
    };

    // samples preparation
    // @TODO must check better the wave formats
    let samples: Vec<f32> = reader
      .into_samples::<i16>()
      .filter_map(Result::ok)
      .map(i16::to_sample::<f32>)
      .collect();

    // set in frames format
    let frames: Vec<Stereo<f32>> = samples
      .chunks(2)
      .map(|x| {
        let f = (x.get(0), x.get(1));
        // :D
        match f {
          (Some(l), Some(r)) => [*l, *r],
          (Some(l), None) => [*l, 0.0f32],
          (None, Some(r)) => [0.0f32, *r],
          (None, None) => Stereo::<f32>::equilibrium(),
        }
      })
      .collect();

    // set frames
    self.frames = frames;

    // parse tempo from filename
    let (orig_tempo, beats) = analytics::get_original_tempo(path, samples.len());
    self.original_tempo = orig_tempo;
    self.num_beats = beats;

    // detect tempo via aubio
    let _detected_tempo = analytics::detect_bpm(&samples[..]);

    // compute onset positions and store them in the hashmap
    let onset_positions = analytics::detect_onsets(&samples[..]);
    let quantized = analytics::quantize_pos(
      &onset_positions,
      self.frames.len() as u64 / (16 * beats as u64),
    );
    self.slices.insert(
      SliceMode::QonsetMode(),
      quantized,
    );
    self.slices.insert(
      SliceMode::OnsetMode(),
      onset_positions,
    );
    self.slices.insert(
      SliceMode::Bar4Mode(),
      analytics::slice_onsets(samples.len(), ((beats/4)*4) as usize),
    );
        self.slices.insert(
      SliceMode::Bar8Mode(),
      analytics::slice_onsets(samples.len(), ((beats/4)*8) as usize),
    );
    self.slices.insert(
      SliceMode::Bar16Mode(),
      analytics::slice_onsets(samples.len(), ((beats/4)*16) as usize),
    );

    Ok(true)
  }
}

/// SampleGen, abstract level struct common to all samples generators.
/// Used to store common fields, we use Structural composition to `extend` this.
/// SampleGens are also iterators internally.
pub struct SampleGen {
  /// smartbuf is the main source of samples and metadata.
  /// The gen will directly use underlying frames as a wrapped buffer.
  smartbuf: SmartBuffer,
  /// playback_rate is the ratio of current tempo over original tempo.
  playback_rate: f64,
  /// playback_mult is a factor of the playback_rate that can be twisted for fun and profit.
  playback_mult: u64,
  /// Is the track is `playing` ?
  /// If false, it just write zero samples in the output buffer, saves some CPU cycles.
  playing: bool,
  /// `frame_index` gives the current sample index in the SmartBuffer.
  /// This will be corrected by the clock at any change in the playback rate to snap to the clock.
  frame_index: u64,
}

/// SampleGenerator Trait.
/// Useful to hide the engines complexity.
pub trait SampleGenerator {
  /// Returns a new SampleGen type instance.
  // fn new() -> Self;
  /// Processes the next block of samples, write it in referenced frame slice.
  fn next_block(&mut self, block_out: &mut [Stereo<f32>]);
  /// Loads a SmartBuffer using `move`
  fn load_buffer(&mut self, smartbuf: SmartBuffer);
  /// Sync is used to synchronise the generator according to the global tempo and the current clock ticks elaspsed.
  /// Many operations can append during a sync internally.
  fn sync(&mut self, global_tempo: u64, tick: u64);
  /// Play sets `play` mode to true (unmute the gen).
  fn play(&mut self);
  /// Stop sets `play` mode to false (mute the gen).
  fn stop(&mut self);
  /// Sets a new playback_mult to play with variable speed multiples.
  fn set_playback_mult(&mut self, playback_mult: u64);
  /// Resets the SampleGenerator playback to start position.
  fn reset(&mut self);
}