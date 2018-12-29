extern crate aubio;
extern crate bus;
extern crate hound;
extern crate num;
extern crate sample;
extern crate time_calc;

use self::aubio::pvoc::Pvoc;
use self::bus::BusReader;
use self::hound::WavReader;
use self::sample::frame::Stereo;
use self::sample::{Frame, Sample};

use audio::track_utils;

const HOP_SIZE: usize = 64;
const WIND_SIZE: usize = 512;
const ANALYSE_SIZE: usize = (WIND_SIZE / 2 + 1);
const PI: f32 = std::f32::consts::PI;
const TWO_PI: f32 = std::f32::consts::PI * 2.0;

fn unwrap2pi(phase: f32) -> f32 {
  return phase + TWO_PI * (1. + (-(phase + PI) / TWO_PI).floor());
}

// an audio track
pub struct PvocAudioTrack {
  // commands rx
  command_rx: BusReader<::midi::CommandMessage>,
  // original tempo of the loaded audio
  original_tempo: f64,
  // playback_rate to match original_tempo
  playback_rate: f64,
  // the track is playring ?
  playing: bool,
  // volume of the track
  volume: f32,
  // original samples
  samples: Vec<f32>,
  // elapsed frames as requested by audio
  elapsed_samples: u64,
  // aubio pvoc
  pvoc: Pvoc,
  // buffer of timeshifted samples
  pvoc_ring: Vec<f32>,
  // previous pvoc norm frame
  pnorm: Vec<f32>,
  // previous pvoc phas frame
  pphas: Vec<f32>,
  // phase accumulator
  phas_acc: Vec<f32>,
  // interp_read, float relative to elapsed hops
  interp_read: f32,
  // interp_block, number of hops in the final speed
  interp_block: usize,
}

impl PvocAudioTrack {
  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> PvocAudioTrack {
    // creates the aubio pvoc
    let aubio_pvoc = Pvoc::new(WIND_SIZE, HOP_SIZE).expect("Pvoc::new");

    PvocAudioTrack {
      command_rx,
      original_tempo: 120.0,
      playback_rate: 1.0,
      playing: false,
      volume: 0.5,
      samples: Vec::new(),
      elapsed_samples: 0,
      pvoc: aubio_pvoc,
      pvoc_ring: Vec::with_capacity(2048),
      pnorm: vec![0.0; ANALYSE_SIZE],
      pphas: vec![0.0; ANALYSE_SIZE],
      phas_acc: vec![0.0; ANALYSE_SIZE],
      interp_read: 0.0,
      interp_block: 0,
    }
  }

  // returns a buffer insead of frames one by one
  pub fn next_block(&mut self, size: usize) -> Vec<Stereo<f32>> {

    // non blocking command fetch
    self.fetch_commands();

    // doesnt consume if not playing
    if !self.playing {
      return (0..size).map(|_x| Stereo::<f32>::equilibrium()).collect();
    }

    println!("size {}", size);

    // get separate chanels
    let mut block_l = vec![0.0; size];
    let mut block_r = vec![0.0; size];
    for (i, s) in self.take(size).enumerate() {
      if i % 2 == 0 {
        block_l[i] = s;
      } else {
        block_r[i] = s;
      }
    }

    // ugliness
    let mut n = vec![0.0; ANALYSE_SIZE];
    let mut p = vec![0.0; ANALYSE_SIZE];

    // ugliness * 2
    let mut nn = vec![0.0; ANALYSE_SIZE];
    let mut pp = vec![0.0; ANALYSE_SIZE];

    // how many iterations to fill one buffer ?
    let n_iter = size / HOP_SIZE;

    // first while in the aubio demo
    for i in 0..n_iter {
      // which hop block maths
      let mut hops_elapsed = (HOP_SIZE * i) + (self.elapsed_samples as isize - size as isize) as usize;
      hops_elapsed /= HOP_SIZE;

      // index in block samples
      let block_index = i * HOP_SIZE;

      // anyway compute the first hop, (mono for now)
      self.pvoc.from_signal(
        &block_l[block_index..block_index + HOP_SIZE],
        &mut n,
        &mut p,
      );

      // return early if its first block
      // the phase voc needs a warmup, we keep it silent for the first hop block
      if hops_elapsed == 0 {
        self.pnorm.copy_from_slice(&n[..]);
        self.pphas.copy_from_slice(&p[..]);
        // push silence in the deque
        for _s in 0..HOP_SIZE { 
          self.pvoc_ring.push(0.0);
        }
        continue;
      }

      // init the phase accumulator
      if hops_elapsed == 1 {
        self.phas_acc.copy_from_slice(&self.pphas[..]);
      }

      // interpolation loop
      loop {
        // used for timestretch
        let frac = 1.0 - (self.interp_read % 1.0);

        // calc interp
        for (i, cnorm) in n.iter().enumerate() {
          nn[i] = frac * self.pnorm[i] + (1.0 - frac) * cnorm;
        }

        // phas_acc is updated after
        pp.copy_from_slice(&self.phas_acc[..]);

        // produce signal
        let mut new_sig = vec![0.0; HOP_SIZE];

        // anyway compute the hop, (mono for now)
        self.pvoc.to_signal(&nn, &pp, &mut new_sig);

        // push back in buffer
        self.pvoc_ring.extend(new_sig);

        // update the phase
        for (i, pacc) in self.phas_acc.iter_mut().enumerate() {
          // calculate phase advance
          let phas_adv = (i as f32 / (ANALYSE_SIZE as f32 - 1.0)) * (PI * HOP_SIZE as f32);
          let mut dphas = p[i] - self.pphas[i] - phas_adv;
          // unwrap angle to [-pi; pi]
          dphas = unwrap2pi(dphas);
          // cumulate phase, to be used for next frame
          *pacc += phas_adv + dphas;
        }

        // interpolation counters
        self.interp_block += 1;
        self.interp_read = self.interp_block as f32 * self.playback_rate as f32;

        // break
        if self.interp_read >= hops_elapsed as f32 {
          break;
        }
      }
      // copy anyway
      self.pnorm.copy_from_slice(&n[..]);
      self.pphas.copy_from_slice(&p[..]);
    }

    // create buffer
    let drained = self.pvoc_ring.drain(0..size);
    let mut buff = Vec::with_capacity(size*2);
    
    for ns in drained {
      buff.push(ns);
      buff.push(ns);
    }
    // send full buffer <
    return track_utils::to_stereo(buff);
  }

  // load audio file
  pub fn load_file(&mut self, path: &str) {
    // load some audio
    let reader = WavReader::open(path).unwrap();

    // samples preparation
    let samples: Vec<f32> = reader
      .into_samples::<i16>()
      .filter_map(Result::ok)
      .map(i16::to_sample::<f32>)
      .collect();

    // parse and set original tempo
    let (orig_tempo, _beats) = track_utils::parse_original_tempo(path, samples.len());
    self.original_tempo = orig_tempo;

    // store in struct
    self.samples = samples;

    // reset
    self.reset();
  }

  // just iterate into the frame buffer
  fn next_sample(&mut self) -> f32 {
    // grab next frame in the frames buffer
    let next_sample = self.samples[self.elapsed_samples as usize % self.samples.len()];
    self.elapsed_samples += 1;
    return next_sample;
  }

  // reset interp and counter
  fn reset(&mut self) {
    self.elapsed_samples = 0;
    self.interp_block = 0;
    self.interp_read = 0.0;
  }

  // fetch commands from rx, return true if received tick for latter sync
  fn fetch_commands(&mut self) {
    match self.command_rx.try_recv() {
      Ok(command) => match command {
        ::midi::CommandMessage::Playback(playback_message) => match playback_message.sync {
          ::midi::SyncMessage::Start() => {
            self.reset();
            self.playing = true;
          }
          ::midi::SyncMessage::Stop() => {
            self.playing = false;
            self.reset();
          }
          ::midi::SyncMessage::Tick(_tick) => {
            let rate = playback_message.time.tempo / self.original_tempo;
            // changed tempo
            if self.playback_rate != rate {
              self.playback_rate = rate;
            }
          }
        },
      },
      _ => (),
    };
  }
}

// Implement `Iterator` for `AudioTrack`.
impl Iterator for PvocAudioTrack {
  type Item = f32;

  // next!
  fn next(&mut self) -> Option<Self::Item> {
    // non blocking command fetch
    self.fetch_commands();

    // doesnt consume if not playing
    if !self.playing {
      return Some(0.0);
    }

    // gte next frame
    let next_sample = self.next_sample();

    // return
    return Some(next_sample);
    /*
     * HERE WE CAN PROCESS BY FRAME
     */
    // FILTER BANK
    // let frame = self.filter_bank.process(frame);
  }
}
