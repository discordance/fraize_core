extern crate bus;
use self::bus::{Bus};

/// ControlMessage Enum is the main message for the control bus
#[derive(Clone)]
pub enum ControlMessage {
  /// Playback message that is always dispatched globally (for all tracks, effects, ui)
  Playback(PlaybackMessage),
}

/// PlaybackMessage have all data used for sync
#[derive(Clone)]
pub struct PlaybackMessage {
  pub sync: SyncMessage,
  // @TODO should be more generic struct for time
  pub time: ::midi::MidiTime,
}

/// Clock sync message
#[derive(Clone)]
pub enum SyncMessage {
  Start(),
  Stop(),
  Tick(u64),
}

/// Initialize the control bus
/// Returns a writable ControlMessageBus
pub fn initialize_control() -> Bus<ControlMessage> {
 return Bus::new(6);
}