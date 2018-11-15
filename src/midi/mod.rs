extern crate bus;
extern crate midir;
extern crate time_calc;

use std::thread;

use self::bus::{Bus, BusReader};
use self::midir::os::unix::VirtualInput;
use self::midir::MidiInput;
use self::time_calc::{Ppqn, Ticks};

const PPQN: Ppqn = 24;

// midi commands (for oth threads)
#[derive(Clone)]
pub enum CommandMessage {
  Playback(PlaybackMessage),
  // Volume(u8, f32),     //  (track_num, vol)
  // Distortion(u8, f32), //  (track_num, level)
}

// midi sync messages
// composite type for outer world
#[derive(Clone)]
pub struct PlaybackMessage {
  pub sync: SyncMessage,
  pub time: MidiTime,
}

// inner midi sync messages
#[derive(Clone)]
pub enum SyncMessage {
  Start(),
  Stop(),
  Tick(u64),
}

// keeps time with midi and calculate useful values
#[derive(Clone)]
pub struct MidiTime {
  pub tempo: f64,
  pub ticks: u64, // tick counter
  pub beats: f64,
  last_timecode: u64,
} // implem
impl MidiTime {
  // restart midi time
  fn restart(&mut self) {
    self.ticks = 0;
    self.last_timecode = 0;
  }

  // compute BPM, bars at each tick
  fn tick(&mut self, tcode: u64) {
    // calculate bpm form midi timecode
    if self.last_timecode == 0 {
      self.last_timecode = tcode;
    } else {
      let bpm = (tcode - self.last_timecode) as f64;
      let bpm = (bpm / 1000.0) * PPQN as f64;
      let bpm = 60_000.0 / bpm;
      self.tempo = bpm.round();
      self.last_timecode = tcode;
    }
    // update tick counter
    self.ticks += 1;

    // how many beats from the start
    self.beats = Ticks(self.ticks as i64).beats(PPQN);

    if (self.beats / 4.0) % 1.0 == 0.0 {
      println!("midi: BAR at beat {}", self.beats);
    }
    // calcu
  }
}

// midi sync callback,
// passing the sender to send data back to the main midi thread
fn midi_sync_cb(tcode: u64, mid_data: &[u8], tx: &mut Bus<SyncMessage>) {
  match mid_data[0] {
    242 => {
      tx.broadcast(SyncMessage::Start());
    }
    252 => {
      tx.broadcast(SyncMessage::Stop());
    }
    248 => {
      tx.broadcast(SyncMessage::Tick(tcode));
    }
    _ => (), // nothing
  }
}

fn broadcast_sync(bus: &mut Bus<CommandMessage>, message: SyncMessage, time: MidiTime) {
    // send to audio tracks
    bus.broadcast(CommandMessage::Playback(PlaybackMessage {
      sync: message,
      time: time,
    }));
}

// initialize midi machinery
pub fn initialize_inputs() -> (thread::JoinHandle<()>, BusReader<CommandMessage>) {
  // bus channel to communicate from the midi callback to audio tracks
  let mut outer_bus = Bus::new(6);
  let outer_rx = outer_bus.add_rx();

  // initialize in its own thread
  let midi_thread = thread::spawn(move || {
    // bus channel to communicate from the midi callback to this thread
    let mut inner_bus = Bus::new(1);
    let mut inner_rx = inner_bus.add_rx();

    // mutable midi time
    let mut midi_time = MidiTime {
      tempo: 120.0,
      ticks: 0,
      beats: 0.0,
      last_timecode: 0,
    };

    // open midi input
    let input = MidiInput::new("Smplr").expect("midi: Couldn't open midi input");

    // take first port
    // let port_name = input.port_name(0).expect("Couldn't get midi port");

    // open connection on virtual port
    let _connection = input
      .create_virtual("midi: Rust Smplr Input", midi_sync_cb, inner_bus)
      .expect("midi: Couldn't open connection");

    // ->
    println!("midi: Listen to midi on port: {}", "Rust Smplr Input");
    println!("midi: Initial Tempo: {}", midi_time.tempo);

    // infinite loop in this thread, blocked by channel receiver
    loop {
      // receive form channel
      let message = inner_rx.recv().unwrap();
      match message {
        // start received
        SyncMessage::Start() => {
          println!("midi: start");
          midi_time.restart();
          // send to audio tracks
          broadcast_sync(& mut outer_bus, message, midi_time.clone());
        }
        // stop received
        SyncMessage::Stop() => {
          println!("midi: stop");
          midi_time.restart();
          // send to audio tracks
          broadcast_sync(& mut outer_bus, message, midi_time.clone());
        }
        // tick received
        SyncMessage::Tick(tcode) => {
          midi_time.tick(tcode);
          // send to audio tracks
          broadcast_sync(& mut outer_bus, message, midi_time.clone());
        }
      }
    }
  });

  // return thread
  return (midi_thread, outer_rx);
}
