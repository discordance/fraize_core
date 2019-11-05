extern crate sample;
use self::sample::frame::Stereo;
use self::sample::Frame;
use std::f32;

/// A clamped fade_in
pub fn fade_in(t: i64, len: i64) -> f32 {
    let c1 = f32::consts::E.powf(t as f32 / len as f32) - 1.0;
    let c2 = f32::consts::E - 1.0;
    let r = c1 / c2;
    if r < 1.0 {
        return r;
    }
    return 1.0;
}

/// a clamped fade out
/// @TODO this env does not sound good for basses
pub fn fade_out(t: i64, len: i64, end: i64) -> f32 {
    let c1 = f32::consts::E.powf((end as f32 - t as f32) / len as f32) - 1.0;
    let c2 = f32::consts::E - 1.0;
    let r = c1 / c2;
    if r < 1.0 {
        return r;
    }
    return 1.0;
}

/// Helper to exec microfades
#[derive(Debug, Default, Copy, Clone)]
pub struct MicroFade {
    /// the ramp t in samples
    cursor: usize,
    /// total micro fade time
    duration: usize,
}

impl MicroFade {
    /// set the micro fade at the start
    pub fn start(&mut self, duration: usize) {
        // needed
        assert_eq!(duration % 2, 0);

        // duration must be a multiple of 2
        self.cursor = 0;
        self.duration = duration;
    }

    /// advance the state of the fade and check if we are in the middle (zero crossing) position
    pub fn next_and_check(&mut self) -> bool {
        self.cursor += 1;
        if self.cursor == self.duration / 2 {
            return true;
        }
        return false;
    }

    /// perform micro frade on the given frame
    pub fn fade_frame(&self, frame: Stereo<f32>) -> Stereo<f32> {
        let d = self.duration / 2;
        if self.cursor < d {
            return // fade out everything before  self.duration / 2
              frame.scale_amp(super::gen_utils::fade_out(
                  self.cursor as i64,
                  d as i64,
                  d as i64
              ));
        }
        if self.cursor == d {
            return Stereo::<f32>::equilibrium();
        }
        if self.cursor > d {
            return frame.scale_amp(super::gen_utils::fade_in(
                (self.cursor - d) as i64,
                d as i64,
            ));
        }
        //
        frame
    }
}

/// Helper to normalize samples assuming interleaved stereo
pub fn normalize_samples(frames: &mut [f32]) {
    // maxes
    let mut l_a_max = 0.0;
    let mut r_a_max = 0.0;

    // get maxes
    for l_r in frames.chunks(2) {
        // should be stereo all around
        assert_eq!(l_r.len(), 2);

        // get sample abs val
        let l_abs = l_r[0].abs();
        let r_abs = l_r[1].abs();

        if l_abs > l_a_max {
            l_a_max = l_abs;
        }
        if r_abs > r_a_max {
            r_a_max = r_abs
        }
    }

    // norm ratios
    let l_n_ratio = 1.0 / l_a_max;
    let r_n_ratio = 1.0 / r_a_max;

    //
    for l_r in frames.chunks_mut(2) {
        // should be stereo all around
        assert_eq!(l_r.len(), 2);

        // apply ratios
        l_r[0] *= l_n_ratio;
        l_r[1] *= r_n_ratio;
    }
}
