extern crate aubio;
extern crate sample;

use self::aubio::onset::Onset;
use self::aubio::tempo::Tempo;

// consts
const HOP_SIZE: usize = 512;
const WIND_SIZE: usize = 2048;
const SR: usize = 44_100;

// bpm detector via aubio
pub fn detect_bpm(samples: Vec<f32>) -> f32 {
  // mono version
  let mono: Vec<f32> = samples.into_iter().step_by(2).collect();
  let mut chunk_iter = mono.chunks(HOP_SIZE); // by chunk
  let mut tempo = Tempo::new(WIND_SIZE, HOP_SIZE, SR).expect("Tempo::new");
  let mut detected_tempo = 120.0;

  loop {
    let next = chunk_iter.next();
    match next {
      Some(chunk) => {
        // break the fft
        if chunk.len() != HOP_SIZE {
          break;
        }
        tempo.execute(&chunk);
        match tempo.bpm() {
          Some(tempo) => detected_tempo = tempo,
          None => (),
        }
      }
      None => break,
    }
  }

  println!("analysis: detected tempo: {}", detected_tempo);

  // return
  detected_tempo
}

pub fn detect_zero_crossing(samples: &Vec<f32>) -> Vec<u32> {
  let mut zero_indices: Vec<u32> = Vec::new();
  let mut sign = -1;
  for (i, f) in samples.iter().step_by(2).enumerate() {
    if *f < 0.0 && sign > 0 {
      sign = -1;
      zero_indices.push(i as u32);
    }
    if *f > 0.0 && sign < 0 {
      sign = 1;
      zero_indices.push(i as u32);
    }
  }
  return zero_indices;
}

// onset detector via aubio <3
pub fn detect_onsets(samples: Vec<f32>) -> Vec<u32> {
  // get zero crossings
  let crossings = detect_zero_crossing(&samples);

  let len = samples.len() / 2;
  let mono: Vec<f32> = samples.into_iter().step_by(2).collect();
  let mut chunk_iter = mono.chunks(HOP_SIZE);

  // onset
  let mut onset = Onset::new(WIND_SIZE, HOP_SIZE, SR).expect("Onset::new");

  // params
  onset.set_threshold(0.9);
  onset.set_silence(-40.0);
  onset.set_minioi(0.005);

  // save position in seconds (we can get that in samples later)
  let mut positions: Vec<u32> = Vec::new();

  // zero by default
  positions.push(0);

  // track
  let mut latest_detection = 0;

  loop {
    let next = chunk_iter.next();
    match next {
      Some(chunk) => {
        // break the fft
        if chunk.len() != HOP_SIZE {
          break;
        }
        onset.execute(&chunk);
        let mut detected = onset.last_onset();
        if latest_detection < detected {
          // match zero crossing right
          let next_crossing = crossings
            .iter()
            .find(|&x| *x > detected);
          let prev_crossing = crossings
            .iter()
            .rev()
            .find(|&x| *x < detected);            
          // match pair, rust got us covered
          match (prev_crossing, next_crossing) {
            (Some(left), Some(right)) => {
              // get distances to snap to the closest
              // let d_left = detected-left;
              // let d_right = right-detected;

              // if d_left <= d_right {
              //   positions.push(*left)
              // } else {
                positions.push(*left)
              // }
            },
            (None, Some(right)) => positions.push(*right),
            (Some(left), None) => positions.push(*left),
            (None, None) => positions.push(detected),
          }
          ;
          latest_detection = detected;
        }
      }
      None => break,
    }
  }
  // push the len as last position
  positions.push(len as u32);
  // return
  positions
}
