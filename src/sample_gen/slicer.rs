extern crate sample;
extern crate time_calc;

use self::sample::frame::Stereo;
use self::sample::Frame;
use self::time_calc::Ticks;
use super::{SampleGen, SampleGenerator, SmartBuffer, PPQN};
use std::collections::BTreeMap;

/// A Slice struct
/// should be copied
#[derive(Debug, Default, Copy, Clone)]
struct Slice {
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
    fn next_frame(&mut self, frames: &[Stereo<f32>]) -> Stereo<f32> {
        if !self.is_consumed() {
            // get the frame index cursor
            let frame_index = self.cursor + self.start;

            // increment cursor
            self.cursor += 1;

            // safely grab a new frame
            let new_frame = frames.get(frame_index);
            match new_frame {
                None => {
                    println!("out of slice bound");
                    return Stereo::<f32>::equilibrium();
                }
                Some(f) => return *f,
            }
        }
        Stereo::<f32>::equilibrium()
    }

    /// the cursor is consumed
    fn is_consumed(&self) -> bool {
        self.cursor > self.len()
    }

    /// get slice len
    fn len(&self) -> usize {
        return self.end - self.start;
    }
}

/// A Slice Sequencer
/// Usefull to order and re-order the slices in any order
/// Hashmap Keys are the sample index of the start slices at original playback speed
/// By default the order given by the sample position
#[derive(Debug, Clone)]
struct SliceSeq {
    /// Positions mode define which kind of positions to use in the slicer
    positions_mode: super::PositionsMode,
    /// Slice ordered according to the keys
    slices: BTreeMap<usize, Slice>,
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
                    start: pos[0],
                    end: pos[1] - 1, // can't fail
                    cursor: 0,
                },
            );
        }

        // init the first slice
        self.current_slice = *self.slices.get(&0).unwrap();

        println!("{:?}", self.current_slice);
    }

    /// get next frame according to the given frame index at seq level
    fn next_frame(&mut self, frame_index: u64, frames: &[Stereo<f32>]) -> Stereo<f32> {
        // grab the next frame
        let next_frame = self.current_slice.next_frame(frames);

        // perform the next slice computation
        // give a nice ordered list of start slices
        let mut kz = self.slices.keys();

        // elegant and ugly at the same time
        // find the first slice index in sample that is just above the frame_index
        let curr_slice_idx = kz.rev().find(|s| **s <= frame_index as usize);

        // check the curr_slice_idx, if none, it is the last
        let curr_slice = match curr_slice_idx {
            None => self.slices.values().last().unwrap(),
            Some(idx) => self.slices.get(idx).unwrap(),
        };

        // current slice is consumable so we need to check if its not already the same one
        // @TODO doesnt allows for repeats
        if self.current_slice.start != curr_slice.start {
//            println!("{}", frame_index);
            self.current_slice = *curr_slice;
        };

        // return next frame
        next_frame
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
                slices: BTreeMap::new(), // is it useful to pre-allocate?
                current_slice: Default::default(),
                positions_mode: super::PositionsMode::Bar16Mode(),
            },
        }
    }

    /// Main logic of Slicer computing the nextframe using the slice seq
    fn slicer_next_frame(&mut self) -> Stereo<f32> {
        // compute the frame index as given by the clock
        let frame_index = self.sample_gen.frame_index;

        self.slice_seq.next_frame(frame_index, &self.sample_gen.smartbuf.frames)
    }

    //    /// Main Logic of Slicer computing the nextframe
    //    fn slicer_next_frame_old(&mut self) -> Stereo<f32> {
    //        // slice pis bounds
    //        let bounds = self.get_position_bounds();
    //
    //        // get original positions
    //        let positions = &self.sample_gen.smartbuf.positions[&self.positions_mode][..];
    //        // apply bounds
    //        let positions = &positions[bounds.0..bounds.1];
    //
    //        // all frames
    //        let frames = &self.sample_gen.smartbuf.frames;
    //
    //        // total number of frames in the buffer
    //        // let num_frames = frames.len() as i64;
    //        let num_frames = self.sample_gen.loop_get_max_frame() as i64;
    //
    //        // number of slices
    //        let num_slices = positions[bounds.0..bounds.1].len() as i64;
    //        // how many frames elapsed from the clock point of view
    //        // because the frame_index is ALWAYS in sync with the ticks
    //        let clock_frames = self.sample_gen.frame_index as i64;
    //        // current cycle, i.e n rotations of full smart buffer
    //        let cycle = (clock_frames as f32 / num_frames as f32) as i64;
    //
    //        // compute next slice
    //        let next_slice = match positions
    //            .iter()
    //            .position(|&x| x as i64 + (cycle * num_frames) > clock_frames)
    //        {
    //            Some(idx) => idx,
    //            None => 0,
    //        };
    //
    //        // compute curr slice
    //        let curr_slice = (next_slice as i64 - 1) % num_slices;
    //
    //        // we just suddently jumped to the next slice :)
    //        if self.pslice != curr_slice as usize {
    //            // set loop div to the next slice
    //            // it works with clicks
    //            if self.sample_gen.loop_div != self.sample_gen.next_loop_div {
    //                self.sample_gen.loop_div = self.sample_gen.next_loop_div;
    //            }
    //            // reset cursor
    //            self.cursor = 0;
    //            // update the previous slice
    //            self.pslice = curr_slice as usize;
    //        }
    //
    //        // compute this current slice len in samples
    //        let slice_len = positions[next_slice as usize] - positions[curr_slice as usize];
    //
    //        // init nextframe with silence
    //        let mut next_frame = Stereo::<f32>::equilibrium();
    //
    //        // checj if we have still samples to read in this slice ?
    //        if (slice_len as i64 - self.cursor) > 0 {
    //            // get the right index in buffer
    //            let mut findex = self.cursor as u64 + positions[curr_slice as usize];
    //
    //            // dont overflow the buffer with wrapping
    //            findex = findex % num_frames as u64;
    //
    //            // get next frame, apply fade in/out slopes
    //            next_frame = frames[findex as usize]
    //                .scale_amp(super::gen_utils::fade_in(self.cursor, 64))
    //                .scale_amp(super::gen_utils::fade_out(
    //                    self.cursor,
    //                    1024 * 2,
    //                    slice_len as i64,
    //                ))
    //                .scale_amp(1.45); // factor that balance with other sample gen types
    //
    //            self.cursor += 1;
    //        }
    //
    //        return next_frame;
    //    }
}

/// SampleGenerator implementation for SlicerGen
impl SampleGenerator for SlicerGen {
    /// Yields processed block out of the samplegen.
    /// This lazy method trigger all the processing.
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
        if self.sample_gen.playback_rate != new_rate {
            // simple update
            self.sample_gen.playback_rate = new_rate;
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
