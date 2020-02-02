extern crate midir;
extern crate serde;
extern crate time;
extern crate time_calc;
extern crate wmidi;
extern crate crossbeam_channel;

use serde::Deserialize;
use std::thread;

use self::crossbeam_channel::bounded;
use self::midir::os::unix::VirtualInput;
use self::midir::MidiInput;
use self::time_calc::{Ppqn, Ticks};
use self::wmidi::MidiMessage;

use config::Config;
use control::{ControlMessage, PlaybackMessage, SyncMessage};

const PPQN: Ppqn = 24;

/// MidiTime keeps time with midi and calculate useful values
#[derive(Clone, Debug, Deserialize, Serialize)]
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
fn midi_cb(
    midi_tcode: u64,
    mid_data: &[u8],
    cb_data: &mut (crossbeam_channel::Sender<ControlMessage>, MidiTime, Config),
) {
    // destructure the tuple
    let (cx_tx, midi_time, conf) = cb_data;

    // parse raw midi inito a usable message
    let message = MidiMessage::from_bytes(mid_data);
    match message {
        Ok(mess) => {
            match mess {
                MidiMessage::NoteOff(_, _, _) => {}
                MidiMessage::NoteOn(_, _, _) => {}
                MidiMessage::PolyphonicKeyPressure(_, _, _) => {}
                MidiMessage::ControlChange(chan, cc_num, val) => {
                    // floatify the val
                    let val_f = val as f32 / 128.0;

                    // parse midi channel and cc num to string for hashmap
                    let midi_chan_str = chan.number().to_string();
                    let cc_num_str = cc_num.to_string();

                    // check if it exists
                    // @TODO use some pattern matching + Option ??
                    if conf.midi_map.cc.contains_key(&midi_chan_str) {
                        // check if it contains the control
                        if conf.midi_map.cc[&midi_chan_str].contains_key(&cc_num_str) {
                            // clone the CommandMessage
                            let message = conf.midi_map.cc[&midi_chan_str][&cc_num_str].clone();
                            // fill in good values and broadcast
                            match message {
                                ControlMessage::Playback(_) => {}
                                ControlMessage::TrackVolume {
                                    tcode: _,
                                    val: _,
                                    track_num,
                                } => {
                                    // broadcast
                                    let mut m = ControlMessage::TrackVolume {
                                        tcode: midi_tcode,
                                        val: val_f,
                                        track_num,
                                    };
                                    // needs a remapping
                                    m.remap_from_midi();
                                    cx_tx.try_send(m).unwrap();
                                }
                                ControlMessage::TrackPan {
                                    tcode: _,
                                    val: _,
                                    track_num,
                                } => {
                                    // broadcast
                                    let mut m = ControlMessage::TrackPan {
                                        tcode: midi_tcode,
                                        val: val_f,
                                        track_num,
                                    };
                                     // needs a remapping
                                    m.remap_from_midi();
                                    cx_tx.try_send(m).unwrap();
                                }
                                ControlMessage::TrackSampleSelect {
                                    tcode: _,
                                    val: _,
                                    track_num,
                                } => {
                                    let m = ControlMessage::TrackSampleSelect {
                                        tcode: midi_tcode,
                                        val: val_f,
                                        track_num,
                                    };
                                    // no need to remap
                                    cx_tx.try_send(m).unwrap();
                                }
                                ControlMessage::TrackLoopDiv {
                                    tcode: _,
                                    val: _,
                                    track_num: _,
                                } => {
                                    unimplemented!();
                                }
                                ControlMessage::Slicer {
                                    tcode: _,
                                    track_num: _,
                                    message: _,
                                } => {
                                    unimplemented!();
                                }
                                ControlMessage::TrackNextSample { tcode: _, track_num: _ } => {
                                    unimplemented!();
                                }
                                ControlMessage::TrackPrevSample { tcode: _, track_num: _ } => {
                                    unimplemented!();
                                }
                            }
                        }
                    }
                }
                MidiMessage::ProgramChange(_, _) => {}
                MidiMessage::ChannelPressure(_, _) => {}
                MidiMessage::PitchBendChange(_, _) => {}
                MidiMessage::SysEx(_) => {}
                MidiMessage::MidiTimeCode(_) => {}
                MidiMessage::SongPositionPointer(_) => {}
                MidiMessage::SongSelect(_) => {}
                MidiMessage::Reserved(_) => {}
                MidiMessage::TuneRequest => {}
                // clock ticks
                MidiMessage::TimingClock => {
                    midi_time.tick(midi_tcode);
                    let message = SyncMessage::Tick(midi_tcode);
                    cx_tx.try_send(::control::ControlMessage::Playback(PlaybackMessage {
                        sync: message,
                        time: midi_time.clone(),
                    })).unwrap();
                }
                // clock start
                MidiMessage::Start => {
                    println!("st");
                    midi_time.restart();
                    let message = SyncMessage::Start();
                    cx_tx.try_send(::control::ControlMessage::Playback(PlaybackMessage {
                        sync: message,
                        time: midi_time.clone(),
                    })).unwrap();
                }
                MidiMessage::Continue => {}
                // clock stop
                MidiMessage::Stop => {
                    midi_time.restart();
                    let message = SyncMessage::Stop();
                    cx_tx.try_send(::control::ControlMessage::Playback(PlaybackMessage {
                        sync: message,
                        time: midi_time.clone(),
                    })).unwrap();
                }
                MidiMessage::ActiveSensing => {}
                MidiMessage::Reset => {}
            }
        }
        Err(_) => {} // do nothing
    }
    //  println!("{} seconds loop midi LOOP CB .", start.to(end));
}

// initialize midi machinery
pub fn initialize_midi(conf: Config) -> (thread::JoinHandle<()>, crossbeam_channel::Receiver<ControlMessage>) {
    // init the control bus
    let (cx_tx, cx_rx) = bounded::<ControlMessage>(1024);

    // initialize in its own thread
    let midi_thread = thread::spawn(move || {
        // bus channel to communicate from the midi callback to this thread safely
        let (i_cx_tx, i_cx_rx) = bounded::<ControlMessage>(1024);

        // mutable midi time
        let midi_time = MidiTime {
            tempo: 120.0,
            ticks: 0,
            beats: 0.0,
            last_timecode: 0,
        };

        // ->
        println!("midi: Listen to midi on port: {}", "Rust Smplr Input");
        println!("midi: Initial Tempo: {}", midi_time.tempo);

        // open midi input
        let input = MidiInput::new("Smplr").expect("midi: Couldn't open midi input");

        // we need to move a lot of stuff in our midi
        let data_tup = (i_cx_tx, midi_time, conf);
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
            let message = i_cx_rx.recv().unwrap();

            let res = cx_tx.try_send(message);
            match res {
                Ok(_) => {}
                Err(e) => {
                    println!("missed in control bus {:?}", e);
                }
            }
        }
    });

    // return thread
    return (midi_thread, cx_rx);
}
