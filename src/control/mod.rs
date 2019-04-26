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
  /// timecode, track_num, gain
  TrackGain{ tcode: u64, val: f32, track_num: usize }
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

/// Initialize the control bus
/// Returns a writable ControlMessageBus
pub fn initialize_control() -> Bus<ControlMessage> {
  println!("Size of a Control Message {:?} bytes", mem::size_of::<ControlMessage>());
 return Bus::new(1024);
}