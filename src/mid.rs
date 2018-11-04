extern crate midir;
extern crate bus;
extern crate time_calc;

use std::{thread};

use self::midir::MidiInput;
use self::midir::os::unix::VirtualInput;
use self::bus::Bus;
use self::time_calc::{Ppqn, Ticks};

const PPQN: Ppqn = 24;

// sync messages form the callback
#[derive(Clone)]
enum SyncMessage {
  Start(),
  Stop(),
  Tick(u64),
}

// keeps time with midi and calculate useful values 
struct MidiTime {
  tempo: f32,
  ticks: u64, // tick counter
  beats: f64,
  last_timecode: u64,
} // implem
impl MidiTime {

  // restart midi time
  fn restart(& mut self) {
    self.ticks = 0;
    self.last_timecode = 0;
  }

  // compute BPM, bars at each tick
  fn tick(& mut self, tcode: u64) {
    // calculate bpm form midi timecode
    if self.last_timecode == 0 {
      self.last_timecode = tcode;
    } else {
      let bpm = (tcode - self.last_timecode) as f32;
      let bpm = (bpm/1000.0)*PPQN as f32;
      let bpm = 60_000.0/bpm;
      self.tempo = bpm.round();
      self.last_timecode = tcode;
    }
    // update tick counter
    self.ticks += 1;

    // how many beats from the start
    self.beats = Ticks(self.ticks as i64).beats(PPQN);

    if (self.beats/4.0) % 1.0 == 0.0 {
      println!("BAR at beat {}", self.beats);
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
    },
    252 => {
      tx.broadcast(SyncMessage::Stop());
    },
    248 => {
      tx.broadcast(SyncMessage::Tick(tcode));
    },
    _ => (), // nothing
  }
}

// initialize midi machinery
pub fn initialize_inputs() -> thread::JoinHandle<()> {

  // initialize in its own thread
  let midi_thread = thread::spawn(move || {

    // bus channel to communicate form the midi callback to this thread
    let mut bus = Bus::new(1);
    let mut rx = bus.add_rx();

    // mutable midi time
    let mut midi_time = MidiTime {
      tempo: 120.0,
      ticks: 0,
      beats: 0.0,
      last_timecode: 0,
    };

    // open midi input
    let input = MidiInput::new("Smplr").expect("Couldn't open midi input");
    
    // take first port
    // let port_name = input.port_name(0).expect("Couldn't get midi port");

    // open connection on virtual port
    let _connection = input.create_virtual("Rust Smplr Input", midi_sync_cb, bus).expect("Couldn't open connection");

    // -> 
    println!("Listen to midi on port: {}", "Rust Smplr Input");
    println!("Init Tempo: {}", midi_time.tempo);

    // infinite loop in this thread, blocked by channel receiver
    loop {
        // receive form channel
        let message = rx.recv().unwrap();
        match message {
          // start received
          SyncMessage::Start() => {
            println!("start");
            midi_time.restart();
          },
          // stop received
          SyncMessage::Stop() => {
            println!("stop");
            midi_time.restart();
          },
          // tick received
          SyncMessage::Tick(tcode) => {
            midi_time.tick(tcode);
            // println!("ticks: {}", midi_time.tick);
          },
        }
    }
  });

  // return thread
  return midi_thread
}
