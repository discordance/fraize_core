extern crate num;
extern crate time_calc;
extern crate sample;

use std::path::Path;

use self::num::ToPrimitive;
use self::time_calc::Samples;
use self::sample::frame::Stereo;


// helper that parses the number of beats of an audio sample in the filepath
// @TODO Way to much unwarp here
pub fn parse_filepath_beats(path: &str) -> i64 {
  // compute path
  let path_obj = Path::new(path);
  let file_stem = path_obj.file_stem().unwrap();
  let file_stem = file_stem.to_str().unwrap();
  let split = file_stem.split("_");
  let split: Vec<&str> = split.collect();
  let beats = split[1].parse::<i64>().unwrap();
  return beats;
}

// calculate the beat
pub fn parse_original_tempo(path: &str, num_samples: usize) -> (f64, i64) {
  // compute number of beats
  let beats = parse_filepath_beats(path);
  let ms = Samples((num_samples as i64) / 2).to_ms(44_100.0);
  let secs = ms.to_f64().unwrap() / 1000.0;
  return (60.0 / (secs / beats as f64), beats);
}

// transforms sample vector to frame vector
pub fn to_stereo(samples: Vec<f32>) -> Vec<Stereo<f32>> {
  
  // consumable the iterator
  let mut it = samples.into_iter();
  let mut stereo : Vec<Stereo<f32>> = Vec::new();
  
  // iterate
  loop {
    let f = (it.next(), it.next());
    // :D
    match f {
      (Some(l), Some(r)) => stereo.push([l, r]),
      (Some(l), None) => stereo.push([l, 0.0f32]),
      (None, Some(r)) => stereo.push([0.0f32, r]),
      (None, None) => break,
    };
  }
  // return
  stereo
}