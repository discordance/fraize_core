extern crate bus;
extern crate midir;
extern crate time_calc;
extern crate wmidi;

use std::thread;

use self::bus::{Bus, BusReader};
use self::midir::os::unix::VirtualInput;
use self::midir::MidiInput;
use self::time_calc::{Ppqn, Ticks};
use self::wmidi::MidiMessage;

use control::{ControlMessage, PlaybackMessage, SyncMessage};

const PPQN: Ppqn = 24;

/// MidiTime keeps time with midi and calculate useful values
#[derive(Clone)]
pub struct MidiTime {
  pub tempo: f64,
  pub ticks: u64, // tick counter
  pub beats: f64,
  last_timecode: u64,
}

/// MidiTime implementation
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
      // println!("midi: BAR at beat {}", self.beats);
    }
    // calcu
  }
}

// midi callback in midi thread
// passing the sender to send data back to the main midi thread
fn midi_cb(tcode: u64, mid_data: &[u8], cb_data: &mut (Bus<ControlMessage>, MidiTime)) {
  // destructure the tuple
  let (tx, midi_time) = cb_data;

  // parse raw midi inito a usable message
  let message = MidiMessage::from_bytes(mid_data);
  match message {
    Ok(mess) => {
      match mess {
        MidiMessage::NoteOff(_, _, _) => {},
        MidiMessage::NoteOn(_, _, _) => {},
        MidiMessage::PolyphonicKeyPressure(_, _, _) => {},
        MidiMessage::ControlChange(_, _, _) => {},
        MidiMessage::ProgramChange(_, _) => {},
        MidiMessage::ChannelPressure(_, _) => {},
        MidiMessage::PitchBendChange(_, _) => {},
        MidiMessage::SysEx(_) => {},
        MidiMessage::MidiTimeCode(_) => {},
        MidiMessage::SongPositionPointer(_) => {},
        MidiMessage::SongSelect(_) => {},
        MidiMessage::Reserved(_) => {},
        MidiMessage::TuneRequest => {},
        // clock ticks
        MidiMessage::TimingClock => {
          midi_time.tick(tcode);
          let message = SyncMessage::Tick(tcode);
          tx.broadcast(::control::ControlMessage::Playback(PlaybackMessage{
            sync:message,
            time: midi_time.clone()
          }));
        },
        // clock start
        MidiMessage::Start => {
          midi_time.restart();
          let message = SyncMessage::Start();
          tx.broadcast(::control::ControlMessage::Playback(PlaybackMessage{
            sync:message,
            time: midi_time.clone()
          }));
        },
        MidiMessage::Continue => {},
        // clock stop
        MidiMessage::Stop => {
          midi_time.restart();
          let message = SyncMessage::Stop();
          tx.broadcast(::control::ControlMessage::Playback(PlaybackMessage{
            sync:message,
            time: midi_time.clone()
          }));
        },
        MidiMessage::ActiveSensing => {},
        MidiMessage::Reset => {},
      }
    },
    Err(_) => {}, // do nothing
  }
}

// initialize midi machinery
pub fn initialize_inputs() -> (thread::JoinHandle<()>, BusReader<ControlMessage>) {
  // init the control bus
  let mut control_bus = ::control::initialize_control();
  // bus channel to communicate from the midi callback to audio tracks
  let outer_rx = control_bus.add_rx();

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

    // ->
    println!("midi: Listen to midi on port: {}", "Rust Smplr Input");
    println!("midi: Initial Tempo: {}", midi_time.tempo);

    // open midi input
    let mut input = MidiInput::new("Smplr").expect("midi: Couldn't open midi input");

    // we need to move a lot of stuff in our midi
    let mut data_tup = (inner_bus, midi_time);
    // take first port
    // let port_name = input.port_name(0).expect("Couldn't get midi port");

    // open connection on virtual port (for Ableton or any host)
    let _connection = input
      // moving data in the callback: inner_bus and midi_time
      .create_virtual("Rust Smplr Input", midi_cb, data_tup)
      .expect("midi: Couldn't open connection");

    // infinite loop in this thread, blocked by channel receiver
    loop {
      // receive form channel
      let message = inner_rx.recv().unwrap();
      control_bus.broadcast(message);
    }
  });

  // return thread
  return (midi_thread, outer_rx);
}
