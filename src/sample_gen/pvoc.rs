extern crate aubio;
extern crate sample;
extern crate time;
extern crate time_calc;

use self::time::PreciseTime;

use self::aubio::pvoc::Pvoc;
use self::sample::frame::Stereo;
use self::sample::Frame;
use self::time_calc::{Beats, Ppqn, Ticks};
use super::{SampleGen, SampleGenerator, SmartBuffer, PPQN};

///
const PI: f64 = std::f64::consts::PI;
const TWO_PI: f64 = std::f64::consts::PI * 2.0;

/// Factor that balance with other sample gen types
const PVOC_1_GAIN: f32 = 0.475;

fn unwrap2pi(phase: f64) -> f64 {
    return phase + TWO_PI * (1. + (-(phase + PI) / TWO_PI).floor());
}

fn copy_from_64(target: &mut [f32], source: &[f64]) {
    for (i, s) in target.iter_mut().enumerate() {
        *s = source[i] as f32;
    }
}

fn copy_to_64(target: &mut [f64], source: &[f32]) {
    for (i, s) in target.iter_mut().enumerate() {
        *s = source[i] as f64;
    }
}

/// Just memory holders to help with PVOC timestretching maths.
/// Avoids re-alloc.
struct PVOCLocalBuffers {
    curr_norm: Vec<f32>,
    curr_phase: Vec<f32>,
    new_norm: Vec<f32>,
    new_phase: Vec<f32>,
    new_signal: Vec<f32>,
}

/// Phase Vocoder Unit is a stateful phase vocoder unit.
struct PVOCUnit {
    /// Hop size (overlap size in the Pvoc).
    hop_size: usize,
    /// FFT Window size.
    window_size: usize,
    /// Analysis size.
    analysis_size: usize,
    /// Aubio wrapper Phase Vocoder instance.
    pvoc: Pvoc,
    /// Buffer of timeshifted samples.
    buff_pvoc_out: Vec<f32>,
    /// Previous pvoc Norms frame.
    pnorm: Vec<f64>,
    /// Previous pvoc Phase frame.
    pphas: Vec<f64>,
    /// Phase Accumulator to keep track of phase.
    phas_acc: Vec<f64>,
    /// Hops counter. Hops are frames overlaps.
    elapsed_hops: usize,
    /// Used for interpolation, float relative to elapsed hops.
    interp_read: f64,
    /// Used for interpolation.
    interp_block: usize,
    /// Buffers for calculations.
    local_buffers: PVOCLocalBuffers,
}

/// PVOCUnit implementation
impl PVOCUnit {
    /// resets the PVOC Unit.
    fn reset(&mut self) {
        self.elapsed_hops = 1;
        self.interp_block = 0;
        self.interp_read = 0.0;
    }

    /// Performs a timestretch operation on a block
    fn process_block(&mut self, hop_s: &[f32], playback_rate: f64) {
        // compute the first hop, (mono for now)
        self.pvoc.from_signal(
            &hop_s,
            &mut self.local_buffers.curr_norm[..],
            &mut self.local_buffers.curr_phase[..],
        );

        // return early if its first block
        // the phase voc needs a warmup, we keep it silent for the first hop block
        if self.elapsed_hops == 0 {
            copy_to_64(&mut self.pnorm[..], &self.local_buffers.curr_norm[..]);
            copy_to_64(&mut self.pphas[..], &self.local_buffers.curr_phase[..]);
            // push silence in the queue
            for _s in 0..self.hop_size {
                self.buff_pvoc_out.push(0.0);
            }
            self.elapsed_hops += 1;
            return;
        }

        // init the phase accumulator
        if self.elapsed_hops == 1 {
            self.phas_acc.copy_from_slice(&self.pphas[..]);
        }

        // interpolation loop
        loop {
            // break forgot this
            if self.interp_read >= self.elapsed_hops as f64 {
                break;
            }

            // used for timestretch
            let frac = 1.0 - (self.interp_read % 1.0);

            // calc interp
            for (i, cnorm) in self.local_buffers.curr_norm.iter().enumerate() {
                self.local_buffers.new_norm[i] =
                    (frac * self.pnorm[i] + (1.0 - frac) * (*cnorm as f64)) as f32;
            }

            // phas_acc is updated after
            copy_from_64(&mut self.local_buffers.new_phase[..], &self.phas_acc[..]);

            // compute the new hop
            self.pvoc.to_signal(
                &self.local_buffers.new_norm,
                &self.local_buffers.new_phase,
                &mut self.local_buffers.new_signal,
            );

            // push back in buffer
            self.buff_pvoc_out.extend(&self.local_buffers.new_signal);

            // update the phase
            for (i, pacc) in self.phas_acc.iter_mut().enumerate() {
                // calculate phase advance
                let phas_adv =
                    (i as f64 / (self.analysis_size as f64 - 1.0)) * (PI * self.hop_size as f64);
                let mut dphas = self.local_buffers.curr_phase[i] as f64 - self.pphas[i] - phas_adv;
                // unwrap angle to [-pi; pi]
                dphas = unwrap2pi(dphas);
                // cumulate phase, to be used for next frame
                *pacc += phas_adv + dphas;
            }

            // interpolation counters
            self.interp_block += 1;
            self.interp_read = self.interp_block as f64 * playback_rate;
        }

        // copy anyway
        copy_to_64(&mut self.pnorm[..], &self.local_buffers.curr_norm[..]);
        copy_to_64(&mut self.pphas[..], &self.local_buffers.curr_phase[..]);

        // inc hops
        self.elapsed_hops += 1;
    }
}

/// Phase Vocoder based sample generator.
/// Use the Aubio phase vocoder to operate time-stretching in real-time.
pub struct PVOCGen {
    /// parent SampleGen struct, as struct composition.
    sample_gen: SampleGen,
    /// Main PhaseVocoder Unit
    pvoc_1: PVOCUnit,
    /// Input buffer stores some fresh samples from the audio source and send them to pvoc units.
    /// Stores mono.
    input_buff: Vec<f32>,
}

/// Specific sub SampleGen implementation
impl PVOCGen {
    /// Inits and return a new SlicerGen sample generator
    pub fn new() -> Self {
        // pvoc 1 vars
        let pvoc_1_window_size = 512;
        let pvoc_1_hopsize = 32;
        let pvoc_1_analyse_size = pvoc_1_window_size / 2 + 1;
        PVOCGen {
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
            pvoc_1: PVOCUnit {
                hop_size: pvoc_1_hopsize,
                window_size: pvoc_1_window_size,
                analysis_size: pvoc_1_analyse_size,
                pvoc: Pvoc::new(pvoc_1_window_size, pvoc_1_hopsize).expect("Pvoc::new"),
                buff_pvoc_out: Vec::with_capacity(1024),
                pnorm: vec![0.0; pvoc_1_analyse_size],
                pphas: vec![0.0; pvoc_1_analyse_size],
                phas_acc: vec![0.0; pvoc_1_analyse_size],
                elapsed_hops: 0,
                interp_read: 0.0,
                interp_block: 0,
                local_buffers: PVOCLocalBuffers {
                    curr_norm: vec![0.0; pvoc_1_analyse_size],
                    curr_phase: vec![0.0; pvoc_1_analyse_size],
                    new_norm: vec![0.0; pvoc_1_analyse_size],
                    new_phase: vec![0.0; pvoc_1_analyse_size],
                    new_signal: vec![0.0; pvoc_1_hopsize],
                },
            },
            input_buff: Vec::with_capacity(1024),
        }
    }
}

/// SampleGenerator implementation for SlicerGen
impl SampleGenerator for PVOCGen {
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

        // hop loop
        // @TODO only working for one pvoc unit as now
        loop {
            // early break
            if self.pvoc_1.buff_pvoc_out.len() >= block_out.len() {
                break;
            }

            // fill input buffer with hop samples
            let hop_size = self.pvoc_1.hop_size;
            for _ in 0..hop_size {
                match self.next() {
                    Some(f) => self.input_buff.push(f[0]),
                    None => self.input_buff.push(0.0),
                };
            }

            // process in pvoc 1
            self.pvoc_1
                .process_block(&self.input_buff[..], self.sample_gen.playback_rate);

            // clear input
            self.input_buff.clear();
        }

        // drain pvoc 1 and write it to block_out
        let mut drained = self.pvoc_1.buff_pvoc_out.drain(0..block_out.len());
        for frame_out in block_out.iter_mut() {
            match drained.next() {
                // yes here it needs some cuts
                Some(s) => *frame_out = [s * PVOC_1_GAIN, s * PVOC_1_GAIN],
                None => *frame_out = Stereo::<f32>::equilibrium(),
            };
        }
    }

    /// Loads a SmartBuffer, moving it
    fn load_buffer(&mut self, smartbuf: &SmartBuffer) {
        // clone is faster ....
        self.sample_gen.smartbuf = smartbuf.clone();
    }

    /// Sync the pvoc according to global sync values
    fn sync(&mut self, global_tempo: u64, tick: u64) {
        // calculate elapsed clock frames according to the original tempo
        let original_tempo = self.sample_gen.smartbuf.original_tempo;
        let clock_frames = Ticks(tick as i64).samples(original_tempo, PPQN, 44_100.0) as u64;

        // we want to resync for each beat
        let beat_samples = Beats(1).samples(self.sample_gen.smartbuf.original_tempo, 44_100.0) as u64;
        let is_beat = clock_frames % beat_samples == 0;

        // calculates the new playback rate
        let new_rate = global_tempo as f64 / original_tempo;
        
        // has the tempo changed ? update accordingly
        if self.sample_gen.playback_rate != new_rate || is_beat {
            // simple update
            self.sample_gen.playback_rate = new_rate;
            // set the frameindex relative to the mixer ticks
            self.sample_gen.sync_frame_index(clock_frames);
            // needs to reset the PVOC
            self.pvoc_1.reset();
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
    fn reset(&mut self) {
        // here is useless to reset the frame index as it closely follows the mixer ticks
        self.sample_gen.sync_reset();
        self.pvoc_1.reset();
    }

    /// Sets the loop div
    fn set_loop_div(&mut self, loop_div : u64) {
        self.sample_gen.next_loop_div = loop_div;
    }
}

/// Implement `Iterator` for `RePitchGen`.
impl Iterator for PVOCGen {
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

        // get next frame, uses sync function to avoid clicks
        let next_frame = self.sample_gen.sync_get_next_frame();
        // return to iter
        return Some(next_frame);
    }
}
