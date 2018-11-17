/* bank filter */
use std::f32;

/*
 * See Robert Bristow-Johnson's "cookbook filters" article.
 * This code implements the "cookbook" filters discussed
 * there.
 */

// plenty wow :)
pub enum FilterType {
  LowPass(),
  HiPass(),
  BpSkirtGain(),
  BpConstantPeak(),
  Notch(),
  Ap(),
  PeakingEQ(),
  LowShelf(),
  HiShelf(),
}

//
pub enum FilterOp {
  UseQ(),
  UseBW(),
  UseSlope(),
}

// biquad coefs
pub struct BiquadCoeffs {
  b0: f32,
  b1: f32,
  b2: f32,
  a0: f32,
  a1: f32,
  a2: f32,
}

impl BiquadCoeffs {
  // computes the biquad filter
  fn compute_coeffs(
    &mut self,
    filter_type: FilterType,
    filter_opt: FilterOp,
    fs: f32,
    f0: f32,
    db_gain: f32,
    q: f32,
    bw: f32,
    slope: f32,
  ) {
    //
    let (mut A, mut w0, alpha, beta) = (0.0, 0.0, 0.0, 0.0);
    let (a0, a1, a2, b0, b1, b2) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

    A = match filter_type {
      FilterType::PeakingEQ() | FilterType::LowShelf() | FilterType::HiShelf() => {
        f32::powf(10.0, db_gain / 40.0)
      },
      _ => 0.0,
    };

    w0 = 2.0 * f32::consts::PI * f0 / fs;
  }
}
