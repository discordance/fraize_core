extern crate sample;
extern crate time_calc;
extern crate rand;

use self::sample::frame::Stereo;
use self::sample::Frame;
use self::time_calc::Ticks;
use super::{SampleGen, SampleGenerator, SmartBuffer, PPQN};
use std::collections::BTreeMap;
use self::rand::Rng;

/// A Slice struct
/// should be copied
#[derive(Debug, Default, Copy, Clone)]
struct Slice {
    /// slice idx
    idx: usize,
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

        // adjust the fade out according to playback_rate
        let mut adjusted_len = self.len() as i64;
        if playback_rate > 1.0 {
            adjusted_len = (adjusted_len as f64 / playback_rate) as i64;
        }

        // return but avoiding clicks
        next_frame
          .scale_amp(super::gen_utils::fade_in(self.cursor as i64, 64))
          .scale_amp(super::gen_utils::fade_out(
              self.cursor as i64,
              1024 * 2, // @TODO this should be param
              adjusted_len,
          ))
          .scale_amp(1.45)
    }

    /// the cursor is consumed
    fn is_consumed(&self) -> bool {
        self.cursor == self.len()
    }

    /// get slice len
    fn len(&self) -> usize {
        return self.end - self.start;
    }
}

/// A Slice Sequencer
/// Usefull to order and re-order the slices in any order
/// BTreeMap Keys are the sample index of the start slices at original playback speed
/// By default the keys are given by the buffer onset positions, depending the mode
#[derive(Debug, Clone)]
struct SliceSeq {
    /// Positions mode define which kind of positions to use in the slicer
    positions_mode: super::PositionsMode,
    /// Slices ordered according to the keys, in orginal order
    slices: BTreeMap<usize, Slice>,
    /// transformed slices
    t_slices: BTreeMap<usize, Slice>,
    /// curently playing slice that will be consumed
    current_slice: Slice,
}

impl SliceSeq {
    /// inits the sequencer from the smart buffer
    fn init_from_buffer(&mut self, buffer: &SmartBuffer) {
        let positions = &buffer.positions[&self.positions_mode];

        // cleared but memory is kept
        self.slices.clear();

        // iterate and set
        for (idx, pos) in positions.windows(2).enumerate() {
            self.slices.insert(
                pos[0],
                Slice {
                    idx,
                    start: pos[0],
                    end: pos[1], // can't fail
                    cursor: 0,
                },
            );
        }

        // ...
        self.t_slices = self.slices.clone();

        // init the first slice
        self.current_slice = *self.slices.get(&0).unwrap();

//        println!("{:?}", self.current_slice);
    }

    /// get next frame according to the given frame index at seq level
    /// it uses playback_speed to adjust the slice envelope
    fn next_frame(&mut self, playback_rate: f64, frame_index: u64, frames: &[Stereo<f32>]) -> Stereo<f32> {
        // grab the next frame
        let next_frame = self.current_slice.next_frame(playback_rate, frames);

        // perform the next slice computation
        // give a nice ordered list of start slices
        let mut kz = self.slices.keys();

        // elegant and ugly at the same time
        // find the first slice index in sample that is just above the frame_index
        let curr_slice_idx = kz.rev().find(|s| **s <= frame_index as usize);

        // check the curr_slice_idx, if none, it is the last
        let curr_slice = match curr_slice_idx {
            None => self.t_slices.values().last().unwrap(),
            Some(idx) => self.t_slices.get(idx).unwrap(),
        };

        // current slice is consumable so we need to check if its not already the same one
        // @TODO maybe a bit harsh, check if slice have been consumed before
        if self.current_slice.idx != curr_slice.idx {
            if !self.current_slice.is_consumed() {
                println!("unfinished");
            }
            self.current_slice = *curr_slice;
        };

        // return next frame
        next_frame
    }

    /// Shuffles the slices !
    fn shuffle(&mut self) {
        // shuffle the keys
        // @TODO first make it work ....

        println!("LOL");

        // will be shuffled
        let mut kz: Vec<usize> = self.slices.keys().map(|k| *k).collect();

        // keep the original keys
        let orig = kz.clone();

        // shuffle
        rand::thread_rng().shuffle(&mut kz );

        // swap pairwise
        for (idx, slice_index) in kz.iter().enumerate() {
            let mut new = *self.slices.get(&slice_index).unwrap();
            let old = self.slices.get(&orig[idx]).unwrap();
            self.t_slices.insert(orig[idx], new);
        }

        // slice length correction
        for (idx, slice_index) in self.slices.keys().enumerate() {
            let old = self.slices.get(slice_index).unwrap();
            let mut this = *self.t_slices.get(slice_index).unwrap();
            this.end = this.start + old.len();
            self.t_slices.insert(*slice_index, this);
        }

        let mut new_total = 0;
        let mut old_total = 0;
        // check
        // @TODO MISSING ELLIOT
        for (key, slice) in self.t_slices.iter() {
            new_total += slice.len();
        }
        for (key, slice) in self.slices.iter() {
            old_total += slice.len();
        }

        println!("new total: {}, old_total: {}", new_total, old_total);
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
                slices: BTreeMap::new(),
                t_slices: BTreeMap::new(),
                current_slice: Default::default(),
                positions_mode: super::PositionsMode::OnsetMode(),
            },
        }
    }

    /// Main logic of Slicer computing the nextframe using the slice seq
    fn slicer_next_frame(&mut self) -> Stereo<f32> {
        // compute the frame index as given by the clock
        let frame_index = self.sample_gen.frame_index;

        // just use the slice sequencer
        self.slice_seq
            .next_frame(self.sample_gen.playback_rate, frame_index, &self.sample_gen.smartbuf.frames)
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
        self.slice_seq.shuffle();
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
//            self.slice_seq.shuffle();
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
