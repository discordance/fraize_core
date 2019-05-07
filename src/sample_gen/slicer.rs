extern crate sample;
extern crate time_calc;

use self::sample::frame::Stereo;
use self::sample::Frame;
use self::time_calc::Ticks;
use super::{SampleGen, SampleGenerator, SmartBuffer, PPQN};

/// Slicer sample generator.
/// Use a method inspired by Beat Slicers.
pub struct SlicerGen {
  /// parent SampleGen struct, as struct composition.
  sample_gen: SampleGen,
  /// Keeps track of previous slice
  pslice: usize,
  /// Cursor is the playhead relative to the current slice
  cursor: i64,
  /// SliceMode define which kind of positions to use in the slicer
  slicer_mode: super::SliceMode,
}

/// Specific sub SampleGen implementation
impl SlicerGen {
  /// Inits and return a new SlicerGen sample generator
  pub fn new() -> Self {
    SlicerGen {
      sample_gen: SampleGen {
        playback_rate: 1.0,
        frame_index: 0,
        playback_mult: 0,
        loop_div: 1,
        loop_offset: 0,
        playing: false,
        smartbuf: SmartBuffer::new_empty(),
        sync_cursor: 0,
        sync_next_frame_index: 0,
      },
      pslice: 0,
      cursor: 0,
      slicer_mode: super::SliceMode::Bar16Mode(),
    }
  }

  /// Main Logic of Slicer computing the nextframe
  fn slicer_next_frame(&mut self) -> Stereo<f32> {
    // slice positions ref
    // depends on slicer mode
    let positions = &self.sample_gen.smartbuf.slices[&self.slicer_mode];
    // all frames
    let frames = &self.sample_gen.smartbuf.frames;
    // total number of frames in the buffer
    let num_frames = frames.len() as i64;
    // number of slices
    let num_slices = positions.len() as i64;
    // how many frames elapsed from the clock point of view
    let clock_frames = self.sample_gen.frame_index as i64;
    // current cycle, i.e n rotations of full smart buffer
    let cycle = (clock_frames as f32 / num_frames as f32) as i64;

    // compute next slice
    let next_slice = match positions
      .iter()
      .position(|&x| x as i64 + (cycle * num_frames) > clock_frames)
      {
        Some(idx) => idx,
        None => 0,
      };

    // compute curr slice
    let curr_slice = (next_slice as i64 - 1) % num_slices;

    // we just suddently jumped to the next slice :)
    if self.pslice != curr_slice as usize {
      // reset cursor
      self.cursor = 0;
      // update the previous slice
      self.pslice = curr_slice as usize;
    }

    // compute this current slice len in samples
    let slice_len = positions[next_slice as usize] - positions[curr_slice as usize];

    // init nextframe with silence
    let mut next_frame = Stereo::<f32>::equilibrium();

    // checj if we have still samples to read in this slice ?
    if (slice_len as i64 - self.cursor) > 0 {
      // get the right index in buffer
      let mut findex = self.cursor as u64 + positions[curr_slice as usize];

      // dont overflow the buffer with wrapping
      findex = findex % num_frames as u64;

      // get next frame, apply fade in/out slopes
      next_frame = frames[findex as usize]
        .scale_amp(super::gen_utils::fade_in(self.cursor, 64))
        .scale_amp(super::gen_utils::fade_out(
          self.cursor,
          1024 * 2,
          slice_len as i64,
        ))
        .scale_amp(1.45); // factor that balance with other sample gen types

      self.cursor += 1;
    }

    return next_frame;
  }
}

/// SampleGenerator implementation for SlicerGen
impl SampleGenerator for SlicerGen {
  /// Yields processed block out of the samplegen.
  /// This lazy method trigger all the processing.
  fn next_block(&mut self, block_out: &mut [Stereo<f32>]) {
    // println!("block call {}", self.sample_gen.playing);
    // just write zero stero frames
    if !self.sample_gen.playing {
      for frame_out in block_out.iter_mut() {
        *frame_out = Stereo::<f32>::equilibrium();
      }
      return;
    }

    // playing, simply use the iterator
    for frame_out in block_out.iter_mut() {
      // can safely be unwrapped because always return something
      *frame_out = self.next().unwrap();
    }
  }

  /// Loads a SmartBuffer, moving it
  fn load_buffer(&mut self, smartbuf: &SmartBuffer) {
    // simply move in the buffer
    self.sample_gen.smartbuf = smartbuf.clone();
    // init the previous slice
    self.pslice = self.sample_gen.smartbuf.slices[&self.slicer_mode].len() - 1;
  }

  /// Sync the slicer according to global values
  fn sync(&mut self, global_tempo: u64, tick: u64) {
    // calculate elapsed clock frames according to the original tempo
    let original_tempo = self.sample_gen.smartbuf.original_tempo;
    let clock_frames = Ticks(tick as i64).samples(original_tempo, PPQN, 44_100.0) as u64;

    // ALWAYS set the frameindex relative to the mixer ticks
    self.sample_gen.frame_index = clock_frames;

    // calculates the new playback rate
    let new_rate = global_tempo as f64 / original_tempo;

    // has the tempo changed ? update accordingly
    if self.sample_gen.playback_rate != new_rate {
      // simple update
      self.sample_gen.playback_rate = new_rate;
    }
  }

  /// sets play
  /// @TODO Notify Error if no frame sto read.println.
  fn play(&mut self) {
    // check if the smart buffer is ready
    if self.sample_gen.smartbuf.frames.len() > 0 {
      self.sample_gen.playing = true;
    }
  }

  /// sets stop
  fn stop(&mut self) {
    self.reset();
    self.sample_gen.playing = false;
  }

  /// sets the playback multiplicator
  fn set_playback_mult(&mut self, playback_mult: u64) {
    self.sample_gen.playback_mult = playback_mult;
  }

  /// resets Sample Generator to start position.
  fn reset(&mut self) {
    // here is useless to reset the frame index as it closely follows the mixer ticks
    // self.sample_gen.frame_index = 0;
    self.pslice = self.sample_gen.smartbuf.slices[&self.slicer_mode].len() - 1;
    self.cursor = 0;
  }
}

/// Implement `Iterator` for `RePitchGen`.
impl Iterator for SlicerGen {
  /// returns stereo frames
  type Item = Stereo<f32>;

  /// Next computes the next frame and returns a Stereo<f32>
  fn next(&mut self) -> Option<Self::Item> {
    // compute next frame
    let next_frame = self.slicer_next_frame();

    // return to iter
    return Some(next_frame);
  }
}
