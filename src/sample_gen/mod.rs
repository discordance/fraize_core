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

// re-publish submodule repitch as a public module;
pub mod analytics;
pub mod gen_utils;
pub mod pvoc;
pub mod repitch;
pub mod slicer;

use hound::WavReader;
use sample::frame::Stereo;
use sample::{Frame, Sample};
use time_calc::{Beats, Ppqn, Samples};
use std::collections::HashMap;

use crate::control::ControlMessage;

/// pulse per quarter note
pub const PPQN: Ppqn = 24;

/// how many sample to fade in / out to avoid clicks when resync audio
const NOCLICK_FADE_LENGTH: u64 = 64;

/// PositionsMode defines how the slices are cut in a smart buffer.
/// Can be Onset Detection or fixed BAR divisions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PositionsMode {
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
#[derive(Debug, Clone)]
pub struct SmartBuffer {
    /// Keeps track of the audio file name.
    pub file_name: String,
    /// Samples in Stereo / float32 format. Use the `sample` Crate for convenience methods.
    /// We only support this format for the moment.
    frames: Vec<Stereo<f32>>,
    /// Original tempo of the audio phrase (if it's a phrase).
    original_tempo: f64,
    /// Number of beats analyzed in audio.
    num_beats: usize,
    /// Precomputed onsets positions. Contains detected Onsets positions and fixed divisions.
    positions: HashMap<PositionsMode, Vec<usize>>,
}

/// Implementation
impl SmartBuffer {
    /// returns an empty SmartBuffer, without allocation ?
    pub fn new_empty() -> Self {
        SmartBuffer {
            frames: Vec::with_capacity(1024),
            file_name: String::with_capacity(512),
            original_tempo: 120.0,
            num_beats: 4,
            positions: HashMap::<PositionsMode, Vec<usize>>::with_capacity(512),
        }
    }

    /// Copy SmartBuffer without memory allocations
    pub fn copy_from(&mut self, from: &SmartBuffer) {
        // start by the frames
        self.frames
            .resize(from.frames.len(), Stereo::<f32>::equilibrium());
        self.frames.copy_from_slice(&from.frames[..]);

        // copy the fields
        self.file_name.clear();
        self.file_name.push_str(from.file_name.as_str());
        self.num_beats = from.num_beats;
        self.original_tempo = from.original_tempo;

        // clone if empty
        if self.positions.len() == 0 {
            self.positions = from.positions.clone();
        } else {
            // trick to avoid mem allocs
            for (key, val) in self.positions.iter_mut() {
                let from_vec = &from.positions.get(key).unwrap();
                val.resize(from_vec.len(), 0);
                val.copy_from_slice(&from_vec[..]);
            }
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

        // get file spec
        let spec = reader.spec();

        // our samples interleaved
        let mut samples: Vec<f32> = match spec.bits_per_sample {
            24 | 32 => reader
                .into_samples::<i32>()
                .filter_map(Result::ok)
                .map(i32::to_sample::<f32>)
                .collect(),
            16 => reader
                .into_samples::<i16>()
                .filter_map(Result::ok)
                .map(i16::to_sample::<f32>)
                .collect(),
            _ => {
                return Err("Wave bits_per_sample not supported")
            }    
        };

        // normalize samples
        // for consistency in volumes + better analysis
        gen_utils::normalize_samples(&mut samples[..]);

        // store in frames format
        let frames = sample::slice::to_frame_slice(&samples[..]).unwrap() as &[Stereo<f32>]; // needed to be explicit
        self.frames = frames.to_vec();

        // analyse
        self.analyse(&samples[..], path);

        Ok(true)
    }

    /// perform various sample analysis
    fn analyse(&mut self, samples: &[f32], path: &str) {
        // parse tempo from filename if possible
        match analytics::read_original_tempo(path, samples.len()) {
            Some((orig_tempo, beats)) => {
                self.original_tempo = orig_tempo;
                self.num_beats = beats;
            }
            None => {
                // detect from aubio
                self.original_tempo = analytics::detect_bpm(&samples[..]);
                let beats = Samples(samples.len() as i64 / 2).beats(self.original_tempo, 44_100.0);
                self.num_beats = beats as usize;
            }
        }

        // compute onset positions
        let onset_positions = analytics::detect_onsets(&samples[..]);

        self.set_postions(samples, self.num_beats, onset_positions);
    }

    /// setup positions for the smart buffer
    fn set_postions(&mut self, samples: &[f32], beats: usize, onset_positions: Vec<usize>) {
        // sometime we can't calculate onsets
        if onset_positions.len() > 2 {
            // quantize onset for the quantized mode
            // @TODO could be parametrized
            let quantized =
                analytics::quantize_pos(&onset_positions, self.frames.len() / (16 * beats));

            // store quantized onsets
            self.positions
                .insert(PositionsMode::QonsetMode(), quantized);

            // store detected onsets
            self.positions
                .insert(PositionsMode::OnsetMode(), onset_positions);
        } else {
            // replace detected onsets by 8 div
            self.positions.insert(
                PositionsMode::QonsetMode(),
                analytics::slice_onsets(samples.len() / 2, ((beats / 4) * 8) as usize),
            );

            // replace detected onsets by 8 div
            self.positions.insert(
                PositionsMode::OnsetMode(),
                analytics::slice_onsets(samples.len() / 2, ((beats / 4) * 8) as usize),
            );
        }
        // store slice onsets
        self.positions.insert(
            PositionsMode::Bar4Mode(),
            analytics::slice_onsets(samples.len() / 2, ((beats / 4) * 4) as usize),
        );
        // store slice onsets
        self.positions.insert(
            PositionsMode::Bar8Mode(),
            analytics::slice_onsets(samples.len() / 2, ((beats / 4) * 8) as usize),
        );
        // store slice onsets
        self.positions.insert(
            PositionsMode::Bar16Mode(),
            analytics::slice_onsets(samples.len() / 2, ((beats / 4) * 16) as usize),
        );
    }
}

/// SampleGen, abstract level struct common to all samples generators.
/// Used to store common fields, we use Structural composition to `extend` this.
struct SampleGen {
    /// smartbuf is the main source of samples and metadata.
    /// The gen will directly use underlying frames as a wrapped buffer.
    smartbuf: SmartBuffer,
    /// playback_rate is the ratio of current tempo over original tempo.
    playback_rate: f64,
    /// playback_mult is a factor of the playback_rate that can be twisted for fun and profit.
    playback_mult: u64,
    /// loop_div is a div factor to reduce loop size in the buffer (a looping a part of the total available samples). defaults to 1.
    /// should not be activated directly because of clicks
    loop_div: u64,
    /// next loop div that is ready to activate when a beat hit. each sample generator variant is responsible of handling this.
    next_loop_div: u64,
    /// loop_offset is an offset to start looping after the real sample start.
    /// this value is relative to the loop_div. the real offset is loop_offset*loop_div.
    /// defaults to zero
    loop_offset: u64,
    /// Is the track is `playing` ?
    /// If false, it just write zero samples in the output buffer, saves some CPU cycles.
    playing: bool,
    /// `frame_index` gives the current sample index in the SmartBuffer.
    /// This will be corrected by the clock at any change in the playback rate to snap to the clock.
    frame_index: u64,
    /// Count samples for Fade-Out/Fade-In to avoid clicks when sync
    sync_cursor: u64,
    /// Next frame index to sync to when the Fade-Out/Fade-In is at zero
    sync_next_frame_index: u64,
}

/// Standard implem mainly for sync
impl SampleGen {
    /// Synchronize the frame index.
    /// Inits the Fade Out / Fade In Mechanism
    fn sync_set_frame_index(&mut self, new_index: u64) {
        self.sync_cursor = 0;
        self.sync_next_frame_index = new_index;
    }

    /// Get the next frame, being sure no click is generated by frame index sync
    fn sync_get_next_frame(&mut self) -> Stereo<f32> {
        // grab some fresh frame
        let max_frame = self.loop_get_max_frame();
        let mut next_frame = self.smartbuf.frames[self.frame_index as usize % max_frame];

        // fade in / out
        next_frame = match self.sync_cursor {
            0..=NOCLICK_FADE_LENGTH => next_frame.scale_amp(gen_utils::fade_out(
                self.sync_cursor as i64,
                NOCLICK_FADE_LENGTH as i64,
                NOCLICK_FADE_LENGTH as i64,
            )),
            _ => next_frame.scale_amp(gen_utils::fade_in(
                self.sync_cursor as i64 - NOCLICK_FADE_LENGTH as i64,
                NOCLICK_FADE_LENGTH as i64,
            )),
        };

        // check if we must change the frame index now
        if self.sync_cursor == NOCLICK_FADE_LENGTH {
            self.frame_index = self.sync_next_frame_index + NOCLICK_FADE_LENGTH;
        } else {
            self.frame_index += 1;
        }

        // inc the sync_cursor
        self.sync_cursor += 1;

        // yield next
        next_frame
    }

    /// Reset the Rync
    fn sync_reset(&mut self) {
        self.frame_index = 0;
        self.sync_cursor = 0;
        self.sync_next_frame_index = 0;
    }

    /// Get the loop upper bound in samples, according to the loop_div (sub-loop length)
    fn loop_get_max_frame(&self) -> usize {
        // how many beats we want
        let mut num_beats_divided = self.smartbuf.num_beats / self.loop_div as usize;

        // safe
        if num_beats_divided < 1 {
            num_beats_divided = 1;
        }

        // convert to samples, in original tempo ofc
        Beats(num_beats_divided as i64).samples(self.smartbuf.original_tempo, 44_100.0) as usize
    }

    /// Is this frame a beat frame
    fn is_beat_frame(&self) -> bool {
        let beat_samples = Beats(1).samples(self.smartbuf.original_tempo, 44_100.0) as u64;
        if self.frame_index % beat_samples == 0 {
            return true;
        }
        false
    }
}

/// SampleGenerator Trait.
/// Useful to hide the engines complexity.
pub trait SampleGenerator {
    /// Processes the next block of samples, write it in referenced frame slice.
    fn next_block(&mut self, block_out: &mut [Stereo<f32>]);
    /// Loads a SmartBuffer by copying
    fn load_buffer(&mut self, smartbuf: &SmartBuffer);
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
    /// Sets the loop div
    fn set_loop_div(&mut self, loop_div: u64);
    /// Used to pass control message that triggers actions specific to SampleGenerator implementations
    fn push_control_message(&mut self, message: ControlMessage);
}
