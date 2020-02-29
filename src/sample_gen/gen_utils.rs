#[cfg(test)]
extern crate plotlib;

extern crate sample;
extern crate easer;

use self::easer::functions::*;
use std::f32;

/// A clamped cubic fade_in
pub fn fade_in(t: i64, len: i64) -> f32 {
    if t == 0 {
        return 0.0;
    }
    if t >= len {
        return 1.0;
    }
    Cubic::ease_in_out(t as f32, 0.0, 1.0, len as f32)
}

/// A cubic fade out
pub fn fade_out(t: i64, len: i64, end: i64) -> f32 {
    if t < end-len {
        return 1.0
    }
    if t >= end {
        return 0.0
    }
    Cubic::ease_in_out((end-t) as f32, 0.0, 1.0, len as f32)
}

#[cfg(test)]
mod tests {

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::plotlib::style::Line;
    use super::*;

    #[test]
    fn test_fade_in() {
        let mut data: Vec<(f64, f64)> = Vec::new();

        for t in 0..400 {
            let y = fade_in(t, 100);
            data.push((t as f64, y as f64));
        }

        let l1 = plotlib::line::Line::new(&data[..])
            .style(plotlib::line::Style::new().colour("red"));
        let v = plotlib::view::ContinuousView::new().add(&l1);
        plotlib::page::Page::single(&v)
            .save("plots/fade_in.svg")
            .expect("saving svg");
    }

    #[test]
    fn test_fade_out() {
        let mut data: Vec<(f64, f64)> = Vec::new();

        for t in 0..800 {
            let y = fade_out(t, 100, 400);
            data.push((t as f64, y as f64));
            // print!("{},", y);
        }

        let l1 = plotlib::line::Line::new(&data[..])
            .style(plotlib::line::Style::new().colour("red"));
        let v = plotlib::view::ContinuousView::new().add(&l1);
        plotlib::page::Page::single(&v)
            .save("plots/fade_out.svg")
            .expect("saving svg");
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
