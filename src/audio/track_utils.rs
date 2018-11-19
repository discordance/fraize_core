extern crate num;
extern crate time_calc;

use std::path::Path;

use self::num::ToPrimitive;
use self::time_calc::Samples;


// helper that parses the number of beats of an audio sample in the filepath
// @TODO Way to much unwarp here
pub fn parse_filepath_beats(path: &str) -> i16 {
  // compute path
  let path_obj = Path::new(path);
  let file_stem = path_obj.file_stem().unwrap();
  let file_stem = file_stem.to_str().unwrap();
  let split = file_stem.split("_");
  let split: Vec<&str> = split.collect();
  let beats = split[1].parse::<i16>().unwrap();
  return beats;
}

pub fn parse_original_tempo(path: &str, num_samples: usize) -> f64 {
  // compute number of beats
  let beats = parse_filepath_beats(path);
  let ms = Samples((num_samples as i64) / 2).to_ms(44_100.0);
  let secs = ms.to_f64().unwrap() / 1000.0;
  return 60.0 / (secs / beats as f64);
}