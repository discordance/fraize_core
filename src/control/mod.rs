extern crate bus;
extern crate serde;

use std::mem;
use serde::{Deserialize};
use self::bus::{Bus};



/// ControlMessage Enum is the main message for the control bus
#[derive(Clone, Debug, Deserialize)]
pub enum ControlMessage {
  /// Playback message that is always dispatched globally (for all tracks, effects, ui)
  Playback(PlaybackMessage),

  /// Track Gain
  TrackGain{ tcode: u64, val: f32, track_num: usize },
  /// Track Pan
  TrackPan{ tcode: u64, val: f32, track_num: usize },
  /// Track sample select, inside the bank
  TrackSampleSelect{ tcode: u64, val: f32, track_num: usize }
}

/// PlaybackMessage have all data used for sync
#[derive(Clone, Debug, Deserialize)]
pub struct PlaybackMessage {
  pub sync: SyncMessage,
  // @TODO should be more generic struct for time
  pub time: ::midi::MidiTime,
}

/// Clock sync message
#[derive(Clone, Debug, Deserialize)]
pub enum SyncMessage {
  Start(),
  Stop(),
  Tick(u64),
}

/// Enum that indicates a direction (for DirectionalParam)
#[derive(Clone, Debug)]
pub enum Direction {
  Up(f32),
  Down(f32),
  Stable(f32)
}

/// DirectionalParam is an helper for memorizing the direction (up/down) of a param
pub struct DirectionalParam {
  /// keep track of old value
  prev_val: f32,
  /// keep track of next value
  next_val: f32,
}

impl DirectionalParam {
  /// constructor
  pub fn new(pv: f32, nv: f32) -> Self {
    return DirectionalParam { prev_val: pv, next_val: nv }
  }

  /// Set next value
  pub fn new_value(&mut self, v: f32) {
    self.prev_val = self.next_val;
    self.next_val = v;
  }

  pub fn get_param(&self) -> Direction {
    if self.next_val > self.prev_val {
      return Direction::Up(self.next_val)
    }
    if self.next_val < self.prev_val {
      return Direction::Down(self.next_val)
    }
    return Direction::Stable(self.next_val)
  }
}

/// SmoothParam is an helper for parameter smoothing
pub struct SmoothParam {
  /// keep track of old value
  prev_val: f32,
  /// keep track of next value
  next_val: f32,
  /// memorize the ramp
  t: usize,
}

impl SmoothParam {

  /// constructor
  pub fn new(pv: f32, nv: f32) -> Self {
    return SmoothParam{prev_val:pv, next_val:nv, t: 0}
  }

  /// set the next value but scaled
  pub fn new_value_scaled(&mut self, v: f32, new_start: f32, new_end: f32) {
    // scale
    let nv = new_start + (new_end - new_start) * ((v - 0.0) / (1.0 - 0.0));
    self.new_value(nv);
  }

  /// set next value
  pub fn new_value(&mut self, v: f32) {
    self.prev_val = self.next_val;
    self.next_val = v;
    // reset t
    self.t = 0;
  }

  /// lin interp between previous and next value, keeping ramp state
  pub fn get_param(&mut self, len: usize) -> f32 {
    let rt = self.t as f32/len as f32;
    let smoothed = (1.0 - rt) * self.prev_val + rt * self.next_val;
    // inc the time if the buffer isnt complete
    if self.t < len {
      self.t += 1;
    }
    return smoothed;
  }
}

/// Initialize the control bus
/// Returns a writable ControlMessageBus
pub fn initialize_control() -> Bus<ControlMessage> {
  println!("Size of a Control Message {:?} bytes", mem::size_of::<ControlMessage>());
 return Bus::new(1024);
}
