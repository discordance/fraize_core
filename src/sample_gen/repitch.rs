extern crate sample;
extern crate time_calc;

use self::sample::frame::Stereo;
use self::sample::{Frame, Sample};
use self::time_calc::{Ticks, Beats};

use super::{SampleGen, SampleGenerator, SmartBuffer, PPQN};

/// LinInterp is a struct that helps interpolation operations.
struct LinInterp {
    interp_val: f64,
    left: Stereo<f32>,
    right: Stereo<f32>,
}

/// LinInterp implementation inspired by the sample crate lerp.
impl LinInterp {
    /// Advance in interpolation
    fn next_source_frame(&mut self, frame: Stereo<f32>) {
        self.left = self.right;
        self.right = frame;
    }

    /// Converts linearly from the previous value, using the next value to interpolate.
    fn interpolate(&mut self, x: f64) -> Stereo<f32> {
        self.left.zip_map(self.right, |l, r| {
            let l_f = l.to_sample::<f64>();
            let r_f = r.to_sample::<f64>();
            let diff = r_f - l_f;
            let out = ((diff * x) + l_f).to_sample::<f32>();
            out
        })
    }
}

/// RePitch sample generator.
/// Use a simple linear interp. Fast / Obvious.
pub struct RePitchGen {
    /// parent SampleGen struct, as struct composition.
    sample_gen: SampleGen,
    /// interpolation LinInterp helper.
    interpolation: LinInterp,
}

/// Specific sub SampleGen implementation
impl RePitchGen {
    /// Inits and return a new RePitchGen sample generator
    pub fn new() -> Self {
        RePitchGen {
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
            interpolation: LinInterp {
                interp_val: 0.0,
                left: Stereo::<f32>::equilibrium(),
                right: Stereo::<f32>::equilibrium(),
            },
        }
    }
}

/// SampleGenerator implementation for RePitchGen
impl SampleGenerator for RePitchGen {
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
            *frame_out = self.next().unwrap().scale_amp(1.44); // factor that balance with other sample gen types
        }
    }

    /// Loads a SmartBuffer, moving it
    fn load_buffer(&mut self, smartbuf: &SmartBuffer) {
        // simply move
        self.sample_gen.smartbuf = smartbuf.clone();
    }

    /// Sync the sample buffer according to global sync values
    fn sync(&mut self, global_tempo: u64, tick: u64) {
        // calculate elapsed clock frames according to the original tempo
        let original_tempo = self.sample_gen.smartbuf.original_tempo;
        let clock_frames = Ticks(tick as i64).samples(original_tempo, PPQN, 44_100.0) as u64;

        // we want to resync for each beat
        let beat_samples = Beats(1).samples(self.sample_gen.smartbuf.original_tempo, 44_100.0) as u64;
        let is_beat = clock_frames % beat_samples == 0;

        // calculates the new playback rate
        let new_rate = global_tempo as f64 / original_tempo;

        // println!("gtempo: {} tick: {} newrate: {}", global_tempo, tick, new_rate);
        // has the tempo changed ? update accordingly
        if self.sample_gen.playback_rate != new_rate || is_beat {
            // simple update
            self.sample_gen.playback_rate = new_rate;
            // sync to the clock estimated frame index
            self.sample_gen.sync_frame_index(clock_frames);
        }
    }

    /// sets play
    /// @TODO Notify Error if no frame sto read
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
    fn reset(&mut self) {
        //self.sample_gen.frame_index = 0;
        self.sample_gen.sync_reset();
    }

    /// Sets the loop div
    fn set_loop_div(&mut self, loop_div : u64) {
        // record next loop_div
        self.sample_gen.next_loop_div = loop_div;
    }
}

/// Implement `Iterator` for `RePitchGen`.
impl Iterator for RePitchGen {
    /// returns stereo frames
    type Item = Stereo<f32>;

    /// Next computes the next frame and returns a Stereo<f32>
    fn next(&mut self) -> Option<Self::Item> {

        // loop div activation
        if self.sample_gen.is_beat_frame() {
            if self.sample_gen.next_loop_div != self.sample_gen.loop_div {
                self.sample_gen.loop_div = self.sample_gen.next_loop_div;
            }
        }

        // advance frames and calc interp val
        while self.interpolation.interp_val >= 1.0 {
            // get next frame, uses sync function to avoid clicks
            let next_frame = self.sample_gen.sync_get_next_frame();

            // interpolate
            let f0 = next_frame;
            self.interpolation.next_source_frame(f0);
            self.interpolation.interp_val -= 1.0;
        }

        // // apply interpolation
        let interp_val = self.interpolation.interp_val;
        let next_i_frame = self.interpolation.interpolate(interp_val);
        self.interpolation.interp_val += self.sample_gen.playback_rate;

        return Some(next_i_frame);
    }
}
