extern crate rand;
extern crate sample;
extern crate time_calc;
//extern crate trallocator;

use self::rand::Rng;
use self::sample::frame::Stereo;
use self::sample::Frame;
use self::time_calc::{Samples, Ticks, TimeSig};
use super::{SampleGen, SampleGenerator, SmartBuffer, PPQN};
use control::{ControlMessage, SlicerMessage};
use std::collections::HashMap;
use std::f64;

//
//use std::alloc::System;
//#[global_allocator]
//static GLOBAL: trallocator::Trallocator<System> = trallocator::Trallocator::new(System);

/// Used to define slicer fadeins fadeouts in samples
const SLICE_FADE_IN: usize = 64;
const SLICE_FADE_OUT: usize = 1024 * 2;
const SLICER_MICRO_FADE: usize = 128;
const SLICER_T_FADE_OUT: usize = 64;

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
//    reverse: bool,
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
            1.0..=f64::INFINITY => {
                (self.len() as f64 / playback_rate) as i64
            }
            _ => {
                self.len() as i64
            }
        };

        // return enveloped, ajusted
        next_frame
            .scale_amp(super::gen_utils::fade_in(
                self.cursor as i64,
                SLICE_FADE_IN as i64,
            ))
            .scale_amp(super::gen_utils::fade_out(
                self.cursor as i64,
                SLICE_FADE_OUT as i64, // @TODO this should be param
                new_len, // adjust from playback rate
            ))
            .scale_amp(1.45)
    }

    /// the cursor is consumed
    fn is_consumed(&self) -> bool {
        self.cursor >= self.len()
    }

    /// how many left
    fn remaining(&self) -> usize {
        if !self.is_consumed() {
            return self.len() - self.cursor;
        }
        0
    }

    /// get slice len
    fn len(&self) -> usize {
        return self.end - self.start;
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
        // our length relative to bar div
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
    mangle_keys: Vec<usize>,
}

impl SliceMap {
    /// new with allocation !
    fn new() -> Self {
        SliceMap {
            unord_slices: HashMap::with_capacity(128),
            ord_keys: Vec::with_capacity(128),
            mangle_keys: Vec::with_capacity(128),
        }
    }

    // clear keeps allocated memory
    fn clear(&mut self) {
        self.unord_slices.clear();
        self.ord_keys.clear();
        assert_eq!(self.unord_slices.len(), self.ord_keys.len());
    }

    // insert
    fn insert(&mut self, k: usize, v: Slice) {
        // insert in hashmap
        self.unord_slices.insert(k, v);
        // insert in keys
        self.ord_keys.push(k);
        // resort
        self.ord_keys[..].sort();
        assert_eq!(self.unord_slices.len(), self.ord_keys.len());
    }

    // get from the hashmap
    fn get(&self, idx: &usize) -> Option<&Slice> {
        self.unord_slices.get(idx)
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
        self.mangle_keys.resize(self.ord_keys.len(), 0);
        self.mangle_keys.copy_from_slice(&self.ord_keys[..]);

        // shuffle mangle_keys
        rand::thread_rng().shuffle(&mut self.mangle_keys);

        // swap pairwise
        for (idx, slice_index) in self.mangle_keys.iter_mut().enumerate() {
            // get slice from older
            let new = *prev_map.get(&slice_index).unwrap(); // should not fail

            // get the slice in mutable
            let old_slice = self.unord_slices.get_mut(&self.ord_keys[idx]).unwrap(); // should not fail;

            // replace                                                                             // replace
            *old_slice = new;
        }
    }

    // repeat the slice over a quant in sample
    fn quant_repeat(&mut self, quant: usize, slice_idx: usize, max: usize) {
        // copy the slice to repeat
        let mut to_repeat = *self.unord_slices.get(&slice_idx).unwrap();

        // clear
        self.clear();

        // used to assign new id
        let mut ct: usize = 0;
        for x in (0..max).step_by(quant) {
            // pass a new id
            to_repeat.id = ct;
            self.insert(x, to_repeat);
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
    /// Get synced by the global clock
    abs_clock_frames: u64,
    /// Holds a local copy of the gen smart buffer, so it can change without clicks
    local_sbuffer: Option<SmartBuffer>,
    /// Positions mode define which kind of positions to use in the slicer
    positions_mode: super::PositionsMode,
    /// Slices in orginal sample gen buffer order
    slices_orig: SliceMap,
    /// Temp Slices used for applying transforms
    slices_temp: SliceMap,
    /// Currently playing Slices
    slices_playing: SliceMap,
    /// Currently playing Slice that will be consumed
    /// Conveniently stores the index also
    curr_slice: (usize, Slice),
    /// pending next buffer change
    next_buffer_change: Option<usize>,
    /// pending next transfrom
    next_transform: Option<TransformType>,
    // useful to perform a micro fade out/in when swaping buffers
//    buffer_swap_fade: super::gen_utils::MicroFadeOutIn,
    /// useful to perform a micro fade out when transform will be applied
    transform_fade_out: Option<super::gen_utils::MicroFadeOut>
}

impl SliceSeq {
    /// set transformation, take cares of the fadeout
    fn push_transform(&mut self, t: Option<TransformType>) {
        // set
        self.next_transform = t;

        // start fade out
        self.transform_fade_out = Some(super::gen_utils::MicroFadeOut::default());
        self.transform_fade_out.unwrap().start(SLICER_T_FADE_OUT); // can't fail
    }

    /// Swap the local frame buffer with the new one according new positions in the new buffer.
    /// Takes immediate action if the local buffer is empty
    fn sync_load_buffer(&mut self, next_buffer: &SmartBuffer) {
        match self.local_sbuffer {
            // we don't have a local buffer yet, so we init (will alloc memory)
            None => {
                self.do_load_buffer(next_buffer);
            }
            // postpone
            Some(_) => {
                // what will be the next slice index in the new buffer ?
                let next_buff_positions = &next_buffer.positions[&self.positions_mode];

                // clock wrapped in the next buffer scale
                let wrapped_clock = self.abs_clock_frames as usize % next_buffer.frames.len();

                // current slice idx in the next buffer
                let mut curr_slice_idx = next_buff_positions.iter().rev().find(|x| {
                    **x <= wrapped_clock
                }).unwrap(); // should never fail

                // we want to change the buffer when the this current slice (on the next buffer) will be on the next slice
                self.next_buffer_change = Some(*curr_slice_idx);
            }
        }
    }

    /// Copy a smart buffer frames into the local buffer
    /// can generate clicks!
    fn do_load_buffer(&mut self, buffer: &SmartBuffer) {
        // check if we have a
        match &mut self.local_sbuffer {
            None => {
                // clone only one time !
                self.local_sbuffer = Some(buffer.clone());
            }
            Some(local) => {
                local.copy_from(buffer);
            }
        }

        // get positions
        let positions = &buffer.positions[&self.positions_mode];

        self.slices_orig.clear();

        // iterate and set
        for (idx, pos) in positions.windows(2).enumerate() {
            self.slices_orig.insert(
                pos[0],
                Slice {
                    id: idx,
                    start: pos[0],
                    end: pos[1], // can't fail
                    cursor: 0,
                },
            );
        }

        //
        self.slices_playing.copy_from(&self.slices_orig);

        // init the first slice
        self.curr_slice = (0, *self.slices_orig.get(&0).unwrap());
    }

    /// Updates the state of slices
    /// Applying transforms
    /// Switching to the next slice
    /// Changing buffer
    fn update(&mut self, gen_buffer: &SmartBuffer) {
        // we need the local buffer initialized
        match &self.local_sbuffer {
            Some(lb) => {
                // store local buffer length here to avoid using lb ref after
                // Rust wart ...
                let mut local_b_len = lb.frames.len();

                // we have a next buffer change pending
                if let Some(change_req_idx) = self.next_buffer_change {
                    // check if the current slice 'virtually' playing in the next buffer is new
                    // relatively to the cuurent slice at the buffer change request time
                    // what will be the next slice index in the new buffer ?
                    let next_buff_positions = &gen_buffer.positions[&self.positions_mode];

                    // clock wrapped in the next buffer scale
                    let wrapped_clock = self.abs_clock_frames as usize % gen_buffer.frames.len();

                    // current slice idx in the next buffer
                    let mut curr_slice_idx = next_buff_positions.iter().rev().find(|x| {
                        **x <= wrapped_clock
                    }).unwrap(); // should never fail

                    // if is not the same, brutally change buffer
                    if *curr_slice_idx != change_req_idx {
                        self.do_load_buffer(gen_buffer);
                        // remove the buffer change pending
                        self.next_buffer_change = None;
                        // we need to update local_b_len, Rust wart
                        local_b_len = gen_buffer.frames.len();
                    }
                }

                // @TODO take care of transform shit

                // gives an ordered list of the currently playing slices indexes
                let indexes = self.slices_playing.ord_keys();

                // find the first slice index in sample that is just above the clock_frames
                // it gives us which slice should play according to the clock
                let curr_slice_idx = indexes
                    .iter() // get all idx iter
                    .rev() // start form the end (reverse)
                    // might not find if we are in the last slice
                    .find(|s| **s <= (self.abs_clock_frames as usize) % local_b_len)
                    // return the last slice index if we are not there
                    .unwrap_or(self.slices_playing.ord_keys().last().unwrap());

                // fetch current slice
                // cannot fail
                let curr_slice = *self.slices_playing.get(&curr_slice_idx).unwrap();

                // check if clock given current slice is the same as the playing current slice
                // if not, we should set the self.curren_slice
                if self.curr_slice.0 != *curr_slice_idx {
                    self.curr_slice = (*curr_slice_idx, curr_slice);
                }
            }
            None => {} // nothing to update
        }
    }


    /// get next frame according to the given clock frames index and the stae of slices.
    /// it uses playback_speed to adjust the slice envelope
    /// get the ref of the sample generator frames, and use a local copy
    /// @TODO needs to apply a short fadeout when queue a transform, still have clicks sometimes
    /// @TODO this function is way too long
    fn next_frame(
        &mut self,
        playback_rate: f64,
        gen_buffer: &SmartBuffer,
    ) -> Stereo<f32> {
        // !! update first !!
        self.update(gen_buffer);

        // check if we have a local buffer
        match &self.local_sbuffer {
            // nope so we send back silence
            None => return Stereo::<f32>::equilibrium(),
            // grab the next frame
            Some(local_buff) => {
                return self
                    .curr_slice.1
                    .next_frame(playback_rate, &local_buff.frames[..]);
//                // always check and advance the buffer_swap_fade
//                if self.buffer_swap_fade.next_and_check() {
//                    // we need to swap the buffer
//                    // copy the gen buffer into the local buffer
//                    self.do_load_buffer(gen_buffer);
//
//                    // and we return here empty here
//                    return Stereo::<f32>::equilibrium();
//                }
//
//                // grab the next frame
//                let mut next_frame = self
//                    .current_slice
//                    .next_frame(playback_rate, &local_frames[..]);
//
//                // apply microfrade if any for the buffer swap
//                next_frame = self.buffer_swap_fade.fade_frame(next_frame);
//
//                // check if there is a pending transform fade out and apply if its the case
//                if let Some(fo) = self.transform_fade_out.as_mut() {
//                    fo.next_and_check();
//                    next_frame = fo.fade_frame(next_frame);
//                }
//
//                // perform the next slice computation
//                // give a nice ordered list of start slices
//                let kz = self.t_slices.ord_keys();
//
//                // elegant and ugly at the same time
//                // find the first slice index in sample that is just above the frame_index
//                let curr_slice_idx = kz
//                    .iter()
//                    .rev()
//                    .find(|s| **s <= (clock_frames as usize) % local_frames.len());
//
//                // check the curr_slice_idx, if none, it is the last
//                let curr_slice_idx = match curr_slice_idx {
//                    None => (*self.t_slices.ord_keys().last().unwrap()),
//                    Some(idx) => *idx,
//                };
//
//                // update the current slice index in the SliceSeq
//                // allow crazy snappy beat repeats
//                self.curr_slice_idx = curr_slice_idx;
//
//                // fetch current slice
//                let curr_slice = *self.t_slices.get(&curr_slice_idx).unwrap();
//
//                // get the next slice idx
//                let next_slice_idx = kz.iter().find(|s| **s > curr_slice_idx);
//
//                // need to look ahead of time to fix glitches in shuffling non equal len slices
//                let next_slice_idx = match next_slice_idx {
//                    None => 0,
//                    Some(nidx) => *nidx,
//                };
//
//                // current slice is not the one that should be, time to switch !
//                // be careful when applying transforms to slice, new IDS have to be provided
//                // or it will get stuck. 10.0.17763.0
//                // @TODO NEED TO WAY THE DIV AFTER FOR APPLYING TRANSFORM
//                // @NOT THE NEXT SLICE
//                if self.current_slice.id != curr_slice.id {
//
//                    // NEW SLICE !
//                    // apply transforms at new slice is better
//                    let next_t = self.next_transform;
//                    match next_t {
//                        None => {
//                            // no transform
//                            // actual switch
//                            self.current_slice = curr_slice;
//                        }
//                        Some(transform) => {
//                            println!("current slice rest {} last frame {:?}",
//                                     self.current_slice.remaining(),
//                                     next_frame);
//
//                            // kill the fade out
//                            self.transform_fade_out = None;
//
//                            // check the pending transform
//                            match transform {
//                                TransformType::Reset() => {
//                                    self.do_reset();
//                                }
//                                TransformType::RandSwap() => {
//                                    // apply shuffle
//                                    self.do_rand_swap();
//                                }
//                                TransformType::QuantRepeat {
//                                    quant,
//                                    slice_index,
//                                } => {
//                                    // need a local buffer
//                                    if let Some(f) = &self.local_frames {
//                                        // how many bars we have
//                                        let num_bars = Samples(f.len() as i64).bars(
//                                            self.orig_tempo,
//                                            TimeSig { top: 4, bottom: 4 },
//                                            44_100.0,
//                                        );
//
//                                        // convert the div in samples
//                                        let mut quant_samples = f.len() / num_bars as usize;
//                                        quant_samples /= quant;
//
//                                        // apply repeat
//                                        self.do_quant_repeat(quant_samples, slice_index);
//                                    }
//                                }
//                            }
//                            // go back to 1
//                            // maybe no good
//                            self.current_slice = *self.t_slices.get(&0).unwrap();
//                            self.next_transform = None;
//                        }
//                    }
//
//                    // needs end of slice error fix
//                    let mut adjusted_len = self.current_slice.len();
//
//                    // fix for shuffle and mismatching length
//                    if curr_slice_idx < next_slice_idx {
//                        let real_len = next_slice_idx - curr_slice_idx;
//                        if adjusted_len > real_len {
//                            adjusted_len = real_len
//                        }
//                    } else {
//                        // if the last slice len is greater than the buffer end, we cut
//                        if curr_slice.start + curr_slice.len()
//                            > self.local_frames.as_ref().unwrap().len()
//                        {
//                            adjusted_len =
//                                self.local_frames.as_ref().unwrap().len() - curr_slice_idx;
//                            // local_frames could not be empty at this stage
//                        }
//                    }
//
//                    // fix for faster playback rate
//                    if playback_rate > 1.0 {
//                        adjusted_len = (adjusted_len as f64 / playback_rate) as usize;
//                    }
//
//                    // apply new length
//                    self.current_slice.end = self.current_slice.start + adjusted_len;
//                };
//
//                return next_frame;
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
        // shuffle the keys
        self.slices_playing.rand_swap(&self.slices_orig);
    }

    /// Repeat a slice accoding to a quantization in samples
    fn do_quant_repeat(&mut self, quant_samples: usize, slice_idx: usize) {
        if let Some(f) = &self.local_sbuffer {
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
                abs_clock_frames: 0,
                local_sbuffer: None, // one sec
                slices_orig: SliceMap::new(),
                slices_temp: SliceMap::new(),
                slices_playing: SliceMap::new(),
                curr_slice: Default::default(),
                positions_mode: super::PositionsMode::OnsetMode(),
                next_transform: None,
                next_buffer_change: None,
//                buffer_swap_fade: Default::default(),
                transform_fade_out: None,
            },
        }
    }

    /// Main logic of Slicer computing the nextframe using the slice seq
    fn slicer_next_frame(&mut self) -> Stereo<f32> {
        // just use the slice sequencer
        self.slice_seq.next_frame(
            self.sample_gen.playback_rate,
            &self.sample_gen.smartbuf,
        )
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
        self.sample_gen.smartbuf.copy_from(smartbuf);
        self.slice_seq.sync_load_buffer(smartbuf);
    }

    /// Sync the slicer according to a clock
    fn sync(&mut self, global_tempo: u64, tick: u64) {
        // calculate elapsed clock frames according to the original tempo
        let original_tempo = self.sample_gen.smartbuf.original_tempo;
        let clock_frames = Ticks(tick as i64).samples(original_tempo, PPQN, 44_100.0) as u64;

        // ALWAYS sync the slice_seq relative to the mixer ticks
        self.slice_seq.abs_clock_frames = clock_frames;


        // calculates the new playback rate
        let new_rate = global_tempo as f64 / original_tempo;

        // has the tempo changed ? update accordingly
        // @TODO equality check of float ...
        if self.sample_gen.playback_rate != new_rate {
            // simple update
            self.sample_gen.playback_rate = new_rate;
        }

        //        if self.sample_gen.is_beat_frame() {
        //        }
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
                        TransformType::QuantRepeat { quant, slice_index: _ } => {
                            self.slice_seq.push_transform(Some(TransformType::QuantRepeat{
                                quant,
                                slice_index: self.slice_seq.curr_slice.0 // gives the index
                            }));
                        },
                        // all pass trought
                        _ => {
                            self.slice_seq.push_transform(Some(t));
                        },
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
