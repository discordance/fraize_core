extern crate sample;
extern crate aubio;


use self::aubio::tempo::Tempo;
use self::aubio::onset::Onset;

// consts
const HOP_SIZE : usize = 256;
const WIND_SIZE : usize = 1024;
const SR : usize = 44_100;

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
          None => ()
        }
      },
      None => break
    }
  }

  println!("analysis: detected tempo: {}", detected_tempo);

  // return
  detected_tempo
}

// onset detector via aubio <3
pub fn detect_onsets(samples: Vec<f32>) -> Vec<u32> {

  let mono: Vec<f32> = samples.into_iter().step_by(2).collect();
  let mut chunk_iter = mono.chunks(HOP_SIZE);

  // onset
  let mut onset = Onset::new(WIND_SIZE, HOP_SIZE, SR).expect("Onset::new");

  // params
  onset.set_threshold(1.0);
  onset.set_silence(-90.0);
  onset.set_minioi(0.1);

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
        // round
        // detected = (detected * 1000.0).round() / 1000.0;
        if latest_detection < detected {
          positions.push(detected);
          latest_detection = detected;
          // println!("last onset {} ", detected);
        }
      },
      None => break
    }
  }

  // return
  positions
}