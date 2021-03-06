extern crate serde;
extern crate crossbeam_channel;

use self::crossbeam_channel::bounded;
use crate::config::Config;
use serde::Deserialize;
use std::thread;
use crate::sample_gen::slicer::TransformType;
use crate::midi::MidiTime;

/// ControlMessage Enum is the main message for the control bus
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ControlMessage {
    /// Playback message that is always dispatched globally (for all tracks, effects, ui)
    Playback(PlaybackMessage),

    /// Track Volume
    TrackVolume {
        tcode: u64,
        val: f32,
        track_num: usize,
    },
    /// Track Pan
    TrackPan {
        tcode: u64,
        val: f32,
        track_num: usize,
    },
    /// Track sample select, inside the bank
    TrackSampleSelect {
        tcode: u64,
        val: f32,
        track_num: usize,
    },
    /// Select next sample
    TrackNextSample {
        tcode: u64,
        track_num: usize,
    },
    /// Select previous sample
    TrackPrevSample {
        tcode: u64,
        track_num: usize,
    },
    /// Track Pan
    TrackLoopDiv {
        tcode: u64,
        val: u64,
        track_num: usize,
    },
    /// Slicer messages
    Slicer {
        tcode: u64,
        track_num: usize,
        message: SlicerMessage
    }
}

/// Implement control message helpers
impl ControlMessage {
    /// Useful to map value from midi which is usually 0..1 only (midi CC)
    pub fn remap_from_midi(&mut self) {
        match self {
            ControlMessage::TrackVolume{tcode: _, val, track_num: _} => {
                *val = ControlMessage::map(*val, 0.0, 1.0, 0.0, 1.2);
            }
            ControlMessage::TrackPan{tcode: _, val, track_num: _} => {
                *val = ControlMessage::map(*val, 0.0, 1.0, -1.0, 1.0);
            }
            // default case just sets the val
            _ => {
                unimplemented!();
            }
        }
    }

    /// map function as in Processing
    /// @TODO not sure belongs there
    fn map(val: f32, ostart : f32, ostop : f32, nstart : f32, nstop : f32) -> f32 {
        nstart + (nstop - nstart) * ((val - ostart) / (ostop - ostart))
    }
}

/// Slicer specific messages
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SlicerMessage {
    Transform(TransformType)
}

/// PlaybackMessage have all data used for sync
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlaybackMessage {
    pub sync: SyncMessage,
    // @TODO should be more generic struct for time
    pub time: MidiTime,
}

/// Clock sync message
#[derive(Clone, Debug, Deserialize, Serialize)]
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
    Stable(f32),
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
        return DirectionalParam {
            prev_val: pv,
            next_val: nv,
        };
    }

    /// Set next value
    pub fn new_value(&mut self, v: f32) {
        self.prev_val = self.next_val;
        self.next_val = v;
    }

    pub fn get_param(&self) -> Direction {
        if self.next_val > self.prev_val {
            return Direction::Up(self.next_val);
        }
        if self.next_val < self.prev_val {
            return Direction::Down(self.next_val);
        }
        return Direction::Stable(self.next_val);
    }
}

/// SmoothParam is an helper for parameter smoothing in audio thread
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
        return SmoothParam {
            prev_val: pv,
            next_val: nv,
            t: 0,
        };
    }

    /// set the next value but scaled
//    pub fn new_value_scaled(&mut self, v: f32, new_start: f32, new_end: f32) {
//        // scale
//        let nv = new_start + (new_end - new_start) * ((v - 0.0) / (1.0 - 0.0));
//        self.new_value(nv);
//    }

    /// set next value
    pub fn new_value(&mut self, v: f32) {
        self.prev_val = self.next_val;
        self.next_val = v;
        // reset t
        self.t = 0;
    }

    /// lin interp between previous and next value, keeping ramp state
    pub fn get_param(&mut self, len: usize) -> f32 {
        let rt = self.t as f32 / len as f32;
        let smoothed = (1.0 - rt) * self.prev_val + rt * self.next_val;
        // inc the time if the buffer isnt complete
        if self.t < len {
            self.t += 1;
        }
        return smoothed;
    }
}

/// ControlHub is the central place that mux messages from MIDI / OSC ... into a unique place.
pub struct ControlHub {
    // Keeps a copy of the config
//    config: Config,
    // Sends data to OSC
//    osc_snd: crossbeam_channel::Sender<ControlMessage>
}

impl ControlHub {
    /// init the control hub
    pub fn new(
        _config: Config,
        _osc_send: crossbeam_channel::Sender<ControlMessage>,
        osc_rcv: crossbeam_channel::Receiver<ControlMessage>,
        midi_rcv: crossbeam_channel::Receiver<ControlMessage>,
    ) -> (Self, crossbeam_channel::Receiver<ControlMessage>) {
        // init the hub out bus
        let (out_cx_tx, out_cx_rx) = bounded::<ControlMessage>(1024);

        // use crossbeam to have clonable senders
        let (cx_tx, cx_rx) = bounded::<ControlMessage>(1024);
        let cx_tx2 = cx_tx.clone();

        // thread that listen to midi events
        thread::spawn(move || {
            // midi listen loop
            loop {
                match midi_rcv.recv() {
                    Ok(m) => {
                        cx_tx.send(m).unwrap();
                    }
                    Err(e) => {
                        println!("{}", e);
                    }
                }

            }
        });

        // thread that listen to osc events
        thread::spawn(move || {
            // osc listen loop
            loop {
                match osc_rcv.recv() {
                    Ok(m) => {
                        cx_tx2.send(m).unwrap();
                    }
                    Err(e) => {
                       println!("{}", e);
                    }
                }
            }
        });

        // muxer thread that reads crossbeam reciever and send out to bus
        thread::spawn(move || {
            loop {
                match cx_rx.recv() {
                    Ok(m) => {
                        out_cx_tx.send(m).unwrap();
                    }
                    _ => {}
                }
            }
        });

        // create the instance
        let new_hub = ControlHub {
//            config,
//            osc_snd: osc_send,
        };

        // return the hub and the rx
        (new_hub, out_cx_rx)
    }
}
