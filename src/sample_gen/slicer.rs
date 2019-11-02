extern crate rand;
extern crate sample;
extern crate time_calc;

use self::rand::Rng;
use self::sample::frame::Stereo;
use self::sample::Frame;
use self::time_calc::Ticks;
use super::{SampleGen, SampleGenerator, SmartBuffer, PPQN};
use std::collections::BTreeMap;

/// Used to define slicer fadeins fadeouts in samples
const SLICE_FADE_IN: usize = 32;
const SLICE_FADE_OUT: usize = 1024 * 2;
const SAFE_FADE_OUT_IN: usize = 64;

/// A Slice struct
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

            // increment cursor
            self.cursor += 1;

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

        // return but avoiding clicks
        next_frame
            .scale_amp(super::gen_utils::fade_in(
                self.cursor as i64,
                SLICE_FADE_IN as i64,
            ))
            .scale_amp(super::gen_utils::fade_out(
                self.cursor as i64,
                SLICE_FADE_OUT as i64, // @TODO this should be param
                self.len() as i64,
            ))
            .scale_amp(1.45)
    }

    /// the cursor is consumed
    fn is_consumed(&self) -> bool {
        self.cursor == self.len()
    }

    /// how many left
    fn remaining(&self) -> usize {
        return self.len() - self.cursor;
    }

    /// get slice len
    fn len(&self) -> usize {
        return self.end - self.start;
    }
}

/// Slice Sequence transformation types
#[derive(Debug, Clone)]
enum TransformType {
    Shuffle(),
}

/// Help to smoothly apply transformation with fadeout.
#[derive(Debug, Default, Clone)]
struct Transform {
    /// stores the next transform to be applied
    next_transform: Option<TransformType>,
}

/// A Slice Sequencer
/// Usefull to order and re-order the slices in any order
/// BTreeMap Keys are the sample index of the start slices at original playback speed
/// By default the keys are given by the buffer onset positions, depending the mode
/// @TODO missing a fade in out when appling transforms
#[derive(Debug, Clone)]
struct SliceSeq {
    /// Positions mode define which kind of positions to use in the slicer
    positions_mode: super::PositionsMode,
    /// Slices ordered according to the keys, in orginal order
    slices_orig: BTreeMap<usize, Slice>,
    /// transformed slices
    /// this hold the result of any transformation
    t_slices: BTreeMap<usize, Slice>,
    /// curently playing slice that will be consumed
    current_slice: Slice,
    /// helper to apply transforms
    transform: Transform,
}

impl SliceSeq {
    /// inits the sequencer from the smart buffer
    fn init_from_buffer(&mut self, buffer: &SmartBuffer) {
        let positions = &buffer.positions[&self.positions_mode];

        // cleared but memory is kept
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

        // ...
        self.t_slices = self.slices_orig.clone();

        // init the first slice
        self.current_slice = *self.slices_orig.get(&0).unwrap();
    }

    /// get next frame according to the given frame index at seq level
    /// it uses playback_speed to adjust the slice envelope
    fn next_frame(
        &mut self,
        playback_rate: f64,
        frame_index: u64,
        frames: &[Stereo<f32>],
    ) -> Stereo<f32> {
        // apply any pending transform

        // grab the next frame
        let mut next_frame = self.current_slice.next_frame(playback_rate, frames);

        // perform the next slice computation
        // give a nice ordered list of start slices
        let mut kz = self.t_slices.keys();

        // elegant and ugly at the same time
        // find the first slice index in sample that is just above the frame_index
        let curr_slice_idx = kz.rev().find(|s| **s <= frame_index as usize);

        // check the curr_slice_idx, if none, it is the last
        let (curr_slice_idx, curr_slice) = match curr_slice_idx {
            None => (
                *self.t_slices.keys().last().unwrap(),
                self.t_slices.values().last().unwrap(),
            ),
            Some(idx) => (*idx, self.t_slices.get(idx).unwrap()),
        };

        // shadow kv
        let mut kz = self.slices_orig.keys();

        // get the next slice idx
        let next_slice_idx = kz.find(|s| **s > curr_slice_idx);

        // need to look ahead of time to fix glitches in shuffling non equal len slices
        let next_slice_idx = match next_slice_idx {
            None => 0,
            Some(nidx) => *nidx,
        };

        // current slice is not the one that should be, time to switch !
        if self.current_slice.id != curr_slice.id {
            // apply trasnforms at new slice is better
            match &self.transform.next_transform {
                None => {
                    // no transform
                    // actual switch
                    self.current_slice = *curr_slice;
                }
                Some(transform) => {
                    // check the pending transform
                    match transform {
                        TransformType::Shuffle() => {
                            // apply shuffle
                            self.shuffle();
                        }
                    }
                    // go back to 1
                    self.current_slice = *self.t_slices.get(&0).unwrap();
                    self.transform.next_transform = None;
                }
            }

            // needs end of slice error fix
            let mut adjusted_len = self.current_slice.len();

            // fix for shuffle and mismatching length
            if curr_slice_idx < next_slice_idx {
                let real_len = (next_slice_idx - curr_slice_idx);
                if adjusted_len > real_len {
                    adjusted_len = real_len
                }
            } else {
                adjusted_len = frames.len() - curr_slice_idx;
            }

            // fix for faster playback rate
            if playback_rate > 1.0 {
                adjusted_len = (adjusted_len as f64 / playback_rate) as usize;
            }

            // apply new length
            self.current_slice.end = self.current_slice.start + adjusted_len;
        };

        // return next frame
        next_frame
    }

    /// safe shuffle
    /// will wait new slice to kick in
    fn safe_shuffle(&mut self) {
        // set next transform
        self.transform.next_transform = Some(TransformType::Shuffle());
    }

    /// Shuffles the slices ! CLICK INDUCING
    /// safe_shuffle should be used instead
    /// @TODO suspect mem allocation
    fn shuffle(&mut self) {
        // shuffle the keys
        // will be shuffled
        let mut kz: Vec<usize> = self.slices_orig.keys().map(|k| *k).collect();

        // keep the original keys
        let orig = kz.clone();

        // shuffle
        rand::thread_rng().shuffle(&mut kz);

        // swap pairwise
        for (idx, slice_index) in kz.iter().enumerate() {
            let mut new = *self.slices_orig.get(&slice_index).unwrap();
            let old = self.slices_orig.get(&orig[idx]).unwrap();
            self.t_slices.insert(orig[idx], new);
        }
    }
}

/// Slicer sample generator.
/// Use a method inspired by Beat Slicers.
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
                smartbuf: SmartBuffer::new_empty(),
                sync_cursor: 0,
                sync_next_frame_index: 0,
            },
            slice_seq: SliceSeq {
                slices_orig: BTreeMap::new(),
                t_slices: BTreeMap::new(),
                current_slice: Default::default(),
                positions_mode: super::PositionsMode::Bar16Mode(),
                transform: Default::default(),
            },
        }
    }

    /// Main logic of Slicer computing the nextframe using the slice seq
    fn slicer_next_frame(&mut self) -> Stereo<f32> {
        // compute the frame index as given by the clock
        let frame_index = self.sample_gen.frame_index;

        // just use the slice sequencer
        self.slice_seq.next_frame(
            self.sample_gen.playback_rate,
            frame_index,
            &self.sample_gen.smartbuf.frames,
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

    /// Loads a SmartBuffer, moving it
    fn load_buffer(&mut self, smartbuf: &SmartBuffer) {
        // simply clone in the buffer
        self.sample_gen.smartbuf = smartbuf.clone();
        self.slice_seq.init_from_buffer(smartbuf);
    }

    /// Sync the slicer according to a clock
    fn sync(&mut self, global_tempo: u64, tick: u64) {
        // calculate elapsed clock frames according to the original tempo
        let original_tempo = self.sample_gen.smartbuf.original_tempo;
        let clock_frames = Ticks(tick as i64).samples(original_tempo, PPQN, 44_100.0) as u64;

        // ALWAYS set the frameindex relative to the mixer ticks
        self.sample_gen.frame_index = clock_frames % self.sample_gen.loop_get_max_frame() as u64;

        //        println!("{} {}", self.sample_gen.loop_get_max_frame(), self.sample_gen.smartbuf.frames.len());
        // calculates the new playback rate
        let new_rate = global_tempo as f64 / original_tempo;

        // has the tempo changed ? update accordingly
        // @TODO equality check of float ...
        if self.sample_gen.playback_rate != new_rate {
            // simple update
            self.sample_gen.playback_rate = new_rate;
        }

        if self.sample_gen.is_beat_frame() {
            self.slice_seq.safe_shuffle();
        }
    }

    /// sets play
    /// @TODO Notify Error if no frame sto read.println.
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
}

/// Implement `Iterator` for `RePitchGen`.
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
