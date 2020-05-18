
// usefull for crossfade
use heapless::consts::U512;
type CrossfadeLen = U512;

use rand::Rng;
use sample::frame::Stereo;
use sample::Frame;
use time_calc::{TimeSig, Ticks, Bars};
use std::collections::HashMap;
use std::f64;

use crate::control::{ControlMessage, SlicerMessage};
use super::{SampleGen, SampleGenerator, SmartBuffer, PPQN};


/// Used to define slicer fadeins fadeouts in samples
const SLICE_FADE_IN: usize = 256;
const SLICE_FADE_OUT: usize = 512;



/// A Slice struct, represnte a slice of audio in the buffer
/// Doesn't store any audio data, but start and end index
/// should be copied
#[derive(Debug, Default, Copy, Clone)]
struct Slice {
    /// slice id
    id: usize,
    /// start sample index in the buffer
    start: usize,
    /// end sample index in the buffer
    end: usize,
    /// cursor is the current position in the slice
    cursor: usize,
    // reverse
    reverse: bool,
}

impl Slice {
    /// get the next frame at cursor
    /// if the cursor is consumed, return the zero frame
    fn next_frame(&mut self, playback_rate: f64, frames: &[Stereo<f32>]) -> Stereo<f32> {
        // init with default
        let mut next_frame = Stereo::<f32>::equilibrium();

        // grab the frame
        if !self.is_consumed() {
            // get the frame index cursor
            let frame_index = self.cursor + self.start;

            // safely grab a new frame
            let new_frame = frames.get(frame_index);
            match new_frame {
                None => {
                    // out of bounds, should never happend
                    next_frame = Stereo::<f32>::equilibrium();
                }
                Some(f) => next_frame = *f,
            }
        }

        // increment cursor
        self.cursor += 1;

        // ajust len
        let new_len = match playback_rate {
            playback_rate if playback_rate >= 1.0 => (self.len() as f64 / playback_rate) as i64,
            _ => self.len() as i64,
        };

        // return enveloped, ajusted
        next_frame
            .scale_amp(super::gen_utils::fade_in(
                self.cursor as i64,
                (SLICE_FADE_IN as f64 * playback_rate) as i64,
            ))
            .scale_amp(super::gen_utils::fade_out(
                self.cursor as i64,
                (SLICE_FADE_OUT as f64 * playback_rate) as i64, // @TODO this should be param
                new_len,                                        // adjust from playback rate
            ))
            .scale_amp(1.45)
    }

    /// the cursor is consumed
    fn is_consumed(&self) -> bool {
        self.cursor >= self.len()
    }

    /// get slice len
    fn len(&self) -> usize {
        return self.end - self.start;
    }

    /// Remaining samples
    fn remaining(&self) -> usize {
        let r = self.end as isize - (self.start as isize + self.cursor as isize);
        if r < 0 {
            return 0;
        }
        return r as usize;
    }
}

/// Slice Sequence transformation types
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum TransformType {
    /// Reset slices to original order
    Reset(),
    /// Swaps all the slices randomly
    RandSwap(),
    /// Repeat a given Slice on index according to a defined quantization in bar div
    QuantRepeat {
        // repeat length relative to bar
        // 1/quant
        quant: usize,
        // the slice to repeat forever
        slice_index: usize,
    },
}

/// a SliceMap is useful encapsulation to perform transform on slice with hashmap and sorted keys index
/// maybe not very efficient but at least pre-allocated
/// no support for remove as we trash all everytime
/// @TODO add a shuffle on postions
#[derive(Debug, Clone)]
struct SliceMap {
    /// Hashmap of all slices, unordered by the datastruct
    unord_slices: HashMap<usize, Slice>,
    /// keeps an ordered copy of the keys
    ord_keys: Vec<usize>,
    /// a buffer allowing to apply transforms to ord_keys
    shifted_keys: Vec<usize>,
}

impl SliceMap {
    /// new with allocation !
    fn new() -> Self {
        SliceMap {
            unord_slices: HashMap::with_capacity(128),
            ord_keys: Vec::with_capacity(128),
            shifted_keys: Vec::with_capacity(128),
        }
    }

    // clear keeps allocated memory
    fn clear(&mut self) {
        self.unord_slices.clear();
        self.ord_keys.clear();
        assert_eq!(self.unord_slices.len(), self.ord_keys.len());
    }

    // insert ALWAYS COPY
    fn insert_copy(&mut self, k: usize, v: Slice) {
        // insert in hashmap
        self.unord_slices.insert(k, v);
        // insert in keys
        self.ord_keys.push(k);
        // resort
        self.ord_keys[..].sort();
        assert_eq!(self.unord_slices.len(), self.ord_keys.len());
    }

    // get BY REF from the hashmap
    fn get_by_ref(&self, idx: &usize) -> Option<&Slice> {
        self.unord_slices.get(idx)
    }

    // get BY COPY from the hashmap
    fn get_by_copy(&self, idx: &usize) -> Option<Slice> {
        match self.unord_slices.get(idx) {
            None => None,
            Some(slice_ref) => Some(*slice_ref), // copy!
        }
    }

    // get ordered_keys
    fn ord_keys(&self) -> &[usize] {
        &self.ord_keys[..]
    }

    // copy from another slice map
    fn copy_from(&mut self, other: &Self) {
        self.clear();
        self.unord_slices.extend(&other.unord_slices);
        self.ord_keys.resize(other.ord_keys.len(), 0);
        self.ord_keys.copy_from_slice(&other.ord_keys[..]);
        assert_eq!(self.unord_slices.len(), self.ord_keys.len());
    }

    // randomly swap the slices while keeping the keys order
    // needs to be passed the previous slicemap as we are manipulating this one
    fn rand_swap(&mut self, prev_map: &Self) {
        // will use mangle_keys, need to resize just in case
        self.shifted_keys.resize(self.ord_keys.len(), 0);
        self.shifted_keys.copy_from_slice(&self.ord_keys[..]);

        // shuffle mangle_keys
        rand::thread_rng().shuffle(&mut self.shifted_keys);

        // swap pairwise
        for (idx, slice_index) in self.shifted_keys.iter_mut().enumerate() {
            // get slice from older
            let mut new = prev_map.get_by_copy(&slice_index).unwrap(); // should not fail

            // get the slice in mutable form
            let old_slice = self.unord_slices.get_mut(&self.ord_keys[idx]).unwrap(); // should not fail;

            // fix length
            new.end = new.start+old_slice.len()-1;

            // replace                                                                             // replace
            *old_slice = new;
        }
    }

    // repeat the slice over a quant in sample
    fn quant_repeat(&mut self, quant: usize, slice_idx: usize, max: usize) {
        // copy the slice to repeat
        let mut to_repeat = *self.unord_slices.get(&slice_idx).unwrap();

        // set new length according to the quant
        to_repeat.end = to_repeat.start+quant-1;

        // clear
        self.clear();

        // used to assign new id
        let mut ct: usize = 0;
        for x in (0..max).step_by(quant) {
            // pass a new id
            to_repeat.id = ct;
            self.insert_copy(x, to_repeat);
            ct += 1;
        }
    }

    fn len(&self) -> usize {
        self.ord_keys.len()
    }
}

/// A Slice Sequencer
/// Usefull to order and re-order the slices in any order
/// BTreeMap Keys are the sample index of the start slices at original playback speed
/// By default the keys are given by the buffer onset positions, depending the mode
#[derive(Debug, Clone)]
struct SliceSeq {
    /// Get synced by the global ticks
    ticks: u64,
    /// Global current tempo from the mixer clock
    global_tempo: u64,
    /// count elapsed frames between each clock tick to have a more precise clock
    /// takes care of the playback rate
    inter_tick_frames: f64,
    /// Holds a local copy of the gen smart buffer, so it can change without clicks
    local_buffer: Option<SmartBuffer>,
    /// Positions mode define which kind of positions to use in the slicer
    positions_mode: super::PositionsMode,
    /// Slices in orginal sample gen buffer order
    slices_orig: SliceMap,
    /// Temp Slices used for applying transforms
    slices_temp: SliceMap,
    /// Currently playing Slices
    slices_playing: SliceMap,
    /// Tuple currently playing Slice that will be consumed
    /// Conveniently stores the index also
    curr_slice_tup: (usize, Slice),
    /// pending next transfrom
    next_transform: Option<TransformType>,
    /// crossfade buffer
    crossfade_buffer: heapless::spsc::Queue<Stereo<f32>, CrossfadeLen>,
}

impl SliceSeq {
    /// Sync the slice sequencer by the ticks and global tempo
    fn sync(&mut self, global_tempo: u64, ticks: u64) {
        // crossfade if tempo externally changed
        if self.global_tempo != global_tempo {
            // prepare crossfade buffer
            self.fill_crossfade_buffer();
        }

        self.ticks = ticks;
        self.global_tempo = global_tempo;
        // reset elapsed frames
        self.inter_tick_frames = 0f64;
    }

    /// Computes the clock in frames scaled / wrapped according to the local smart buffer
    fn get_local_clock(&self) -> u64 {
        if let Some(lb) = &self.local_buffer {
            let original_tempo = lb.original_tempo;
            let abs = Ticks(self.ticks as i64).samples(original_tempo, PPQN, 44_100.0) as u64
                % lb.frames.len() as u64;
            return abs + self.inter_tick_frames as u64;
        }
        0
    }

    /// Compute the current playback rate
    fn playback_rate(&self) -> f64 {
        if let Some(lb) = &self.local_buffer {
            return self.global_tempo as f64 / lb.original_tempo;
        }
        120.0
    }

    /// Increment the elapsed frame counter
    fn new_frame(&mut self) {
        self.inter_tick_frames += self.playback_rate();
    }

    /// set transformation
    fn push_transform(&mut self, t: Option<TransformType>) {
        // set
        self.next_transform = t;
    }

    /// fills the crossfade buffer
    fn fill_crossfade_buffer(&mut self) {
        match &self.local_buffer {
            None => (),
            Some(local_buff) => {
                // fill with current slice
                for _i in 0..(self.crossfade_buffer.capacity()-self.crossfade_buffer.len()) {
                    self.crossfade_buffer
                        .enqueue(
                            self.curr_slice_tup
                                .1
                                .next_frame(self.playback_rate(), &local_buff.frames[..]),
                        )
                        .expect("no overflow");
                }
            }
        }
    }

    /// Copy a smart buffer frames into the local buffer
    /// trying to not generate clicks
    fn load_buffer(&mut self, buffer: &SmartBuffer) {
        // prepare crossfade buffer
        self.fill_crossfade_buffer();

        // check if we have a
        match &mut self.local_buffer {
            None => {
                // clone only one time !
                self.local_buffer = Some(buffer.clone());
            }
            Some(local) => {
                local.copy_from(buffer);
            }
        }

        // get positions
        let positions = &buffer
            .positions
            .get(&self.positions_mode)
            .expect("position mode exists");

        self.slices_orig.clear();

        // iterate and set
        for (idx, pos) in positions.windows(2).enumerate() {
            self.slices_orig.insert_copy(
                *pos.first().expect("have a first pos"),
                Slice {
                    id: idx,
                    start: *pos.first().expect("have a first pos"),
                    end: *pos.last().expect("have a last pos"), // can't fail
                    cursor: 0,
                    reverse: false,
                },
            );
        }

        // init the currently playing slice map 
        self.slices_playing.copy_from(&self.slices_orig);

        // adjust current slice
        self.adjust_current_slice();
    }

    /// Ajust current slice to local clock
    fn adjust_current_slice(&mut self) {
        // compute current slice index in the playing slices according to the clock
        let curr_slice_idx = self.current_slice_idx();

        // get the current slice copy
        let mut curr_slice = self.slices_playing.get_by_copy(&curr_slice_idx).unwrap();

        // adjust the cursor from the clock
        let cursor_gap = self.get_local_clock() - curr_slice_idx as u64;
        curr_slice.cursor += cursor_gap as usize; // ultra important step

        // set current slice
        self.curr_slice_tup = (curr_slice_idx, curr_slice);
    }

    /// Check if current slice is obsolete and return clock current slice index
    fn compute_curr_slice(&self) -> (bool, usize) {
        // compute current slice index in the playing slices according to the clock
        let curr_slice_idx = self.current_slice_idx();

        (self.curr_slice_tup.0 != curr_slice_idx, curr_slice_idx)
    }

    /// Check for pending transforms and apply timely
    fn apply_transform(&mut self) {
        // checks if there is a transform stacked
        if let Some(nt) = self.next_transform {
            // fill the crossfade buffer with the current slice
            self.fill_crossfade_buffer();

            // apply according transform
            match nt {
                TransformType::Reset() => { 
                    self.do_reset();
                },
                TransformType::RandSwap() => { 
                    self.do_rand_swap();
                },
                TransformType::QuantRepeat { quant, slice_index } => {
                    // another way to avoid pattern matching
                    let local_buff = self.local_buffer.as_ref().expect("buffer here");
                    
                    // how many samples per bar
                    let smpls_per_bar = Bars(1).samples(
                        local_buff.original_tempo,
                        TimeSig { top: 4, bottom: 4 },
                        44_100.0
                    );

                    // repeat in samples
                    let quant_samples = smpls_per_bar / quant as i64;

                    // apply repeat
                    self.do_quant_repeat(quant_samples as usize, slice_index);
                },
            }

            // adjust current slice (operation above changed it)
            self.adjust_current_slice();

            // unstack
            self.next_transform = None;
        }    
    }

    /// Updates the current slice if have to
    fn update_curr_slice(&mut self, _gen_buffer: &SmartBuffer) {
        // nothing to worry about
        if self.local_buffer.is_none() {
            return;
        }

        // compute current slice index in the playing slices according to the clock
        let (is_obsolete, curr_slice_idx) = self.compute_curr_slice();

        // check if clock given current slice is the same as the playing current slice
        // if not, we should set the self.curren_slice
        if is_obsolete {
            // NEW SLICE HERE
            let next_curr_slice = self.slices_playing.get_by_copy(&curr_slice_idx).unwrap();
            self.curr_slice_tup = (curr_slice_idx, next_curr_slice);
        }
    }

    fn current_slice_idx(&self) -> usize {
        // gives an ordered list of the currently playing slices indexes
        let indexes = self.slices_playing.ord_keys();
        // find the first slice index in sample that is just above the clock_frames
        // it gives us which slice should play according to the clock
        let curr_slice_idx = indexes
            .iter() // get all idx iter
            .rev() // start form the end (reverse)
            // might not find if we are in the last slice
            .find(|s| **s <= self.get_local_clock() as usize)
            // return the last slice index if we are not there
            .unwrap_or(self.slices_playing.ord_keys().last().unwrap());

        *curr_slice_idx
    }

    /// get next frame according to the given clock frames index and the stae of slices.
    /// it uses playback_speed to adjust the slice envelope
    /// get the ref of the sample generator frames, and use a local copy
    fn next_frame(&mut self, gen_buffer: &SmartBuffer) -> Stereo<f32> {
        // updates the inter ticks elapsed frames
        // for fine grained clock
        self.new_frame();

        // updates transforms
        self.apply_transform();

        // updates the current slice
        self.update_curr_slice(gen_buffer);

        // check if we have a local buffer
        match &self.local_buffer {
            // nope so we send back silence
            None => return Stereo::<f32>::equilibrium(),
            // grab the next frame
            Some(local_buff) => {
                // grab next frame
                let next_frame = self
                    .curr_slice_tup
                    .1
                    .next_frame(self.playback_rate(), &local_buff.frames[..]);

                // crossfade
                if self.crossfade_buffer.len() > 0 {
                    let t = self.crossfade_buffer.capacity() - self.crossfade_buffer.len();
                    let fade_in_ratio = super::gen_utils::fade_in(
                        t as i64,
                        self.crossfade_buffer.capacity() as i64,
                    );
                    let fade_out_ratio = super::gen_utils::fade_out(
                        t as i64,
                        self.crossfade_buffer.capacity() as i64,
                        self.crossfade_buffer.capacity() as i64,
                    );

                    let old_f = self.crossfade_buffer.dequeue().unwrap();

                    // actual crossfade
                    let mixed_frame = next_frame
                        .scale_amp(fade_in_ratio)
                        .add_amp(old_f.scale_amp(fade_out_ratio));
                    return mixed_frame;
                }
                return next_frame;
            }
        }
    }

    // transforms

    /// reset the slices !
    fn do_reset(&mut self) {
        self.slices_playing.copy_from(&self.slices_orig);
    }

    /// Rand swaps the slices ! Can introduce clicks if done in the middle
    /// safe_shuffle should be used instead
    fn do_rand_swap(&mut self) {
        // randomly swap the slices
        self.slices_playing.rand_swap(&self.slices_orig);
    }

    /// Repeat a slice accoding to a quantization in samples
    fn do_quant_repeat(&mut self, quant_samples: usize, slice_idx: usize) {
        if let Some(f) = &self.local_buffer {
            self.slices_playing
                .quant_repeat(quant_samples, slice_idx, f.frames.len());
        }
    }
}

/// Slicer sample generator.
/// Use a method inspired by Beat Slicers like Propellerheads Reason.
pub struct SlicerGen {
    /// parent SampleGen struct, as struct composition.
    sample_gen: SampleGen,
    /// Slice Sequencer
    slice_seq: SliceSeq,
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
                next_loop_div: 1,
                loop_offset: 0,
                playing: false,
                smartbuf: SmartBuffer::new_empty(), // source of truth
                sync_cursor: 0,
                sync_next_frame_index: 0,
            },
            slice_seq: SliceSeq {
                ticks: 0,
                global_tempo: 120,
                inter_tick_frames: 0f64,
                local_buffer: None, // one sec
                slices_orig: SliceMap::new(),
                slices_temp: SliceMap::new(),
                slices_playing: SliceMap::new(),
                curr_slice_tup: Default::default(),
                positions_mode: super::PositionsMode::OnsetMode(),
                next_transform: None,
                crossfade_buffer: heapless::spsc::Queue::new(),
            },
        }
    }

    /// Main logic of Slicer computing the nextframe using the slice seq
    fn slicer_next_frame(&mut self) -> Stereo<f32> {
        // just use the slice sequencer
        self.slice_seq.next_frame(&self.sample_gen.smartbuf)
    }
}

/// SampleGenerator implementation for SlicerGen
impl SampleGenerator for SlicerGen {
    /// Yields processed block out of the samplegen.
    /// This method trigger all the processing.
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

    /// Loads a SmartBuffer from a reference
    fn load_buffer(&mut self, smartbuf: &SmartBuffer) {
        self.slice_seq.load_buffer(smartbuf);
        self.sample_gen.smartbuf.copy_from(smartbuf);
    }

    /// Sync the slicer according to a clock
    fn sync(&mut self, global_tempo: u64, tick: u64) {
        // calculate elapsed clock frames according to the original tempo
        if let Some(_lb) = &self.slice_seq.local_buffer {
            self.slice_seq.sync(global_tempo, tick);
        }
    }

    /// sets play
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
    fn reset(&mut self) {}

    /// Sets the loop div
    fn set_loop_div(&mut self, loop_div: u64) {
        // record next loop_div
        self.sample_gen.next_loop_div = loop_div;
    }

    /// SampleGen impl specific control message
    fn push_control_message(&mut self, message: ControlMessage) {
        // only interested in Slicer messages
        match message {
            ControlMessage::Slicer {
                tcode: _,
                track_num: _,
                message,
            } => match message {
                SlicerMessage::Transform(t) => {
                    // match if we have a repeat, capture needs to be immediate
                    match t {
                        // catch repeat to catch the current slice idx
                        TransformType::QuantRepeat {
                            quant,
                            ..
                        } => {
                            self.slice_seq
                                .push_transform(Some(TransformType::QuantRepeat {
                                    quant,
                                    slice_index: self.slice_seq.curr_slice_tup.0,
                                }));
                        }
                        // all pass trought
                        _ => {
                            self.slice_seq.push_transform(Some(t));
                        }
                    }
                }
            },
            _ => (), // ignore the rest
        }
    }
}

/// Implement `Iterator` for `SliceGen`.
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
