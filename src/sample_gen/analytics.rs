extern crate aubio;
extern crate num;
extern crate sample;
extern crate time_calc;

use std::path::Path;

use self::aubio::onset::Onset;
use self::aubio::tempo::Tempo;
use self::num::ToPrimitive;
use self::time_calc::{Bpm, Samples};

// consts
const HOP_SIZE: usize = 512;
const WIND_SIZE: usize = 2048;
const SR: usize = 44_100;

// Parse the original tempo based on the beat value written in the filename
fn parse_filepath_beats(path: &str) -> Result<usize, &str> {
    // compute path
    let path_obj = Path::new(path);
    let file_stem = match path_obj.file_stem() {
        Some(fstem) => fstem,
        None => return Err("NoFileName"),
    };
    let file_stem = match file_stem.to_str() {
        Some(s) => s,
        None => return Err("NoFileName"),
    };
    let split: Vec<&str> = file_stem.split("_").collect();
    match split.last() {
        Some(last) => {
            match last.parse::<usize>() {
                Ok(b) => return Ok(b),
                Err(_err) => return Err("ParseIntError"),
            };
        }
        None => return Err("NoBeatNum"),
    }
}

/// Get the original tempo based on the beat value written in the filename, or analized with Aubio if not present.
/// Returns original tempo as computed from file name and the number of beats
/// @TODO take care of the the aubio part
pub fn read_original_tempo(path: &str, num_samples: usize) -> Option<(f64, usize)> {
    // compute number of beats
    let num_beats = match parse_filepath_beats(path) {
        Ok(n) => n,
        Err(err) => return None,
    };
    let ms = Samples((num_samples as i64) / 2).to_ms(44_100.0);

    let secs = ms.to_f64().unwrap() / 1000.0;
    return Some((60.0 / (secs / num_beats as f64), num_beats));
}

/// Onset detector via Aubio.
pub fn detect_onsets(samples: &[f32]) -> Vec<usize> {
    let len = samples.len() / 2;
    let mono: Vec<f32> = samples
        .iter()
        .step_by(2)
        .zip(samples.iter().step_by(2).skip(1))
        .map(|(l, r)| (l + r) / 2.0)
        .collect();
    let mut chunk_iter = mono.chunks(HOP_SIZE);

    // onset
    let mut onset = Onset::new(WIND_SIZE, HOP_SIZE, SR).expect("Onset::new");

    // params
    onset.set_threshold(0.3);
    onset.set_silence(-30.0);
    onset.set_minioi(0.02);

    // detected positions
    let mut positions: Vec<usize> = Vec::new();

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
                let detected = onset.last_onset();

                // check for some invalid, bug from aubio
                if detected > len as u32 {
                    continue;
                }

                if latest_detection < detected {

                    positions.push(detected as usize);
                    latest_detection = detected;
                }
            }
            None => break,
        }
    }

    // push the len as last position
    positions.push(len);

    // return
    positions
}

/// BPM detector via aubio.
pub fn detect_bpm(samples: &[f32]) -> f64 {
    // mono version
    let mono: Vec<f32> = samples
        .iter()
        .step_by(2)
        .zip(samples.iter().step_by(2).skip(1))
        .map(|(l, r)| (l + r) / 2.0)
        .collect();
        
    // let mono: Vec<f32> = samples.iter().step_by(2).map(|x| *x).collect();
    let mut chunk_iter = mono.chunks(HOP_SIZE / 4); // by chunk
    let mut tempo = Tempo::new(WIND_SIZE / 4, HOP_SIZE / 4, SR).expect("Tempo::new");
    // let mut detected_tempo = 120.0;

    loop {
        let next = chunk_iter.next();
        match next {
            Some(chunk) => {
                // break the fft
                if chunk.len() != HOP_SIZE / 4 {
                    break;
                }
                tempo.execute(&chunk);
            }
            None => break,
        }
    }

    // read analysed
    let mut analysed_t = tempo.bpm().expect("Should have analysed a tempo").floor();

    // heuristic that is a bit dirty
    if analysed_t < 80.0 {
        analysed_t *= 2.0;
    }
    if analysed_t > 190.0 {
        analysed_t /= 2.0;
    }

    // return
    analysed_t as f64
}

/// Basic division onsets position.
pub fn slice_onsets(len: usize, divisor: usize) -> Vec<usize> {
    let step = len / divisor;
    let mut positions = Vec::new();
    for x in 0..divisor {
        positions.push(x * step);
    }
    positions.push(len);
    return positions;
}

/// Quantize a position vector to factor `multiple`
pub fn quantize_pos(d: &[usize], multiple: usize) -> Vec<usize> {
    let mut new_pos = Vec::new();
    for pos in d.iter() {
        let q = (*pos as f32 / multiple as f32).round() * multiple as f32;
        new_pos.push(q as usize);
    }
    new_pos
}
