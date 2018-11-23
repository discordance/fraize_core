extern crate sample;

use self::sample::frame::Stereo;
use self::sample::Frame;
use std::f32;

/*
 * See Robert Bristow-Johnson's "cookbook filters" article.
 * This code implements the "cookbook" filters discussed
 * there.
 */

/* bank filter */
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
pub struct BiquadFilter {
  // coefs
  b0: f32,
  b1: f32,
  b2: f32,
  a0: f32,
  a1: f32,
  a2: f32,
  // c's
  c1: f32,
  c2: f32,
  c3: f32,
  c4: f32,
  c5: f32,
  // x and y
  x: [[f32; 3]; 2],
  y: [[f32; 3]; 2],
}

impl BiquadFilter {

  // computes and return a biquad filter
  pub fn create_filter(
    filter_type: FilterType,
    filter_opt: FilterOp,
    fs: f32,
    f0: f32,
    db_gain: f32,
    q: f32,
    bw: f32,
    slope: f32,
  ) -> BiquadFilter {
    //
    let (mut A, mut w0, mut alpha, mut beta) = (0.0f32, 0.0, 0.0, 0.0);
    let (mut a0, mut a1, mut a2, mut b0, mut b1, mut b2) = (0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0);

    //
    A = match filter_type {
      FilterType::PeakingEQ() | FilterType::LowShelf() | FilterType::HiShelf() => {
        f32::powf(10.0, db_gain / 40.0)
      }
      _ => 0.0,
    };

    //
    w0 = 2.0 * f32::consts::PI * f0 / fs;

    //
    match filter_opt {
      FilterOp::UseSlope() => match filter_type {
        FilterType::LowShelf() | FilterType::HiShelf() => {
          alpha = f32::sin(w0) / 2.0 * f32::sqrt((A + 1.0 / A) * (1.0 / slope - 1.0) + 2.0)
        }
        _ => alpha = f32::sin(w0) / (2.0 * q),
      },
      FilterOp::UseBW() => {
        alpha = f32::sin(w0) * f32::sinh(f32::ln(2.0) / 2.0 * bw * w0 / f32::sin(w0))
      }
      FilterOp::UseQ() => alpha = f32::sin(w0) / (2.0*q)
    }

    // switch and calc
    match filter_type {
      FilterType::LowPass() => {
        b0 = (1.0 - f32::cos(w0)) / 2.0;
        b1 = 1.0 - f32::cos(w0);
        b2 = (1.0 - f32::cos(w0)) / 2.0;
        a0 = 1.0 + alpha;
        a1 = -2.0 * f32::cos(w0);
        a2 = 1.0 - alpha;
      }
      FilterType::HiPass() => {
        b0 = (1.0 + f32::cos(w0)) / 2.0;
        b1 = -(1.0 + f32::cos(w0));
        b2 = (1.0 + f32::cos(w0)) / 2.0;
        a0 = 1.0 + alpha;
        a1 = -2.0 * f32::cos(w0);
        a2 = 1.0 - alpha;
      }
      FilterType::BpSkirtGain() => {
        b0 = q * alpha; /* or sin(w0)/2.0 */
        b1 = 0.0;
        b2 = -q * alpha; /* or -sin(w0)/2.0 */
        a0 = 1.0 + alpha;
        a1 = -2.0 * f32::cos(w0);
        a2 = 1.0 - alpha;
      }
      FilterType::BpConstantPeak() => {
        b0 = alpha;
        b1 = 0.0;
        b2 = -alpha;
        a0 = 1.0 + alpha;
        a1 = -2.0 * f32::cos(w0);
        a2 = 1.0 - alpha;
      }
      FilterType::Notch() => {
        b0 = 1.0;
        b1 = -2.0 * f32::cos(w0);
        b2 = 1.0;
        a0 = 1.0 + alpha;
        a1 = -2.0 * f32::cos(w0);
        a2 = 1.0 - alpha;
      }
      FilterType::Ap() => {
        b0 = 1.0 - alpha;
        b1 = -2.0 * f32::cos(w0);
        b2 = 1.0 + alpha;
        a0 = 1.0 + alpha;
        a1 = -2.0 * f32::cos(w0);
        a2 = 1.0 - alpha;
      }
      FilterType::PeakingEQ() => {
        b0 = 1.0 + alpha * A;
        b1 = -2.0 * f32::cos(w0);
        b2 = 1.0 - alpha * A;
        a0 = 1.0 + alpha;
        a1 = -2.0 * f32::cos(w0);
        a2 = 1.0 - alpha;
      }
      FilterType::LowShelf() => {
        beta = 2.0*f32::sqrt(A)*alpha;
        b0 = A * ((A + 1.0) - (A - 1.0) * f32::cos(w0) + beta);
        b1 = 2.0 * A * ((A - 1.0) - (A + 1.0) * f32::cos(w0));
        b2 = A * ((A + 1.0) - (A - 1.0) * f32::cos(w0) - beta);
        a0 = (A + 1.0) + (A - 1.0) * f32::cos(w0) + beta;
        a1 = -2.0 * ((A - 1.0) + (A + 1.0) * f32::cos(w0));
        a2 = (A + 1.0) + (A - 1.0) * f32::cos(w0) - beta;
      }
      FilterType::HiShelf() => {
        beta = 2.0*f32::sqrt(A)*alpha;
        b0 = A * ((A + 1.0) + (A - 1.0) * f32::cos(w0) + beta);
        b1 = -2.0 * A * ((A - 1.0) + (A + 1.0) * f32::cos(w0));
        b2 = A * ((A + 1.0) + (A - 1.0) * f32::cos(w0) - beta);
        a0 = (A + 1.0) - (A - 1.0) * f32::cos(w0) + beta;
        a1 = 2.0 * ((A - 1.0) - (A + 1.0) * f32::cos(w0));
        a2 = (A + 1.0) - (A - 1.0) * f32::cos(w0) - beta;
      }
    }

    // return
    BiquadFilter {
      a0: a0,
      a1: a1,
      a2: a2,
      b0: b0,
      b1: b1,
      b2: b2,
      c1: b0 / a0,
      c2: b1 / a0,
      c3: b2 / a0,
      c4: a1 / a0,
      c5: a2 / a0,
      x: [[0f32; 3]; 2],
      y: [[0f32; 3]; 2],
    }
  }

  // process
  #[inline(always)]
  pub fn process(&mut self, frame: Stereo<f32>) -> Stereo<f32> {
   
    // ->
    let n: usize = 2;
   
    // new frame
    let mut new_frame =  Stereo::<f32>::equilibrium();

    // needed
    for o in 0..2 {
      self.x[o][n - 2] = self.x[o][n - 1];
      self.x[o][n - 1] = self.x[o][n];
      self.x[o][n] = frame[o];
      self.y[o][n - 2] = self.y[o][n - 1];
      self.y[o][n - 1] = self.y[o][n];
      self.y[o][n] =
          self.c1 
          * self.x[o][n] 
          + self.c2 * self.x[o][n - 1] 
          + self.c3 * self.x[o][n - 2]
          - self.c4 * self.y[o][n - 1]
          - self.c5 * self.y[o][n - 2];

      new_frame[o] = self.y[o][n];
    }
    
    // return
    new_frame
  }
}
