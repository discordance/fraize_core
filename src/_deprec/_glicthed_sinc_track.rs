extern crate bus;
extern crate elapsed;
extern crate hound;
extern crate num;
extern crate sample;
extern crate time_calc;

use self::bus::BusReader;
use self::hound::WavReader;
use self::sample::frame::Stereo;
use self::sample::ring_buffer;
use self::sample::{Frame, Sample};

use audio::filters::{BiquadFilter, FilterOp, FilterType};
use audio::track_utils;
use std::f64;

const R_BUFF_LEN: usize = 64;

// struct to help interpolation
struct Interp {
  iterp_val: f64,
  frames: ring_buffer::Fixed<[Stereo<f32>; R_BUFF_LEN]>,
  idx: usize,
}
impl Interp {
  // depth
  fn depth(&self) -> usize {
    self.frames.len() / 2
  }

  // Advance
  fn next_source_frame(&mut self, frame: Stereo<f32>) {
    let _old_frame = self.frames.push(frame);
    if self.idx < self.depth() {
      self.idx += 1;
    }
  }

  // Converts linearly from the previous value, using the next value to interpolate.
  fn interpolate(&mut self, x: f64) -> Stereo<f32> {
    let phil = x;
    let phir = 1.0 - x;
    let nl = self.idx;
    let nr = self.idx + 1;
    let depth = self.depth();

    let rightmost = nl + depth;
    let leftmost = nr as isize - depth as isize;
    let max_depth = if rightmost >= self.frames.len() {
      self.frames.len() - depth
    } else if leftmost < 0 {
      (depth as isize + leftmost) as usize
    } else {
      depth
    };

    (0..max_depth).fold(Stereo::<f32>::equilibrium(), |mut v, n| {
      v = {
        let a = f64::consts::PI * (phil + n as f64);
        let first = f64::sin(a) / a;
        let second = 0.5 + 0.5 * f64::cos(a / (phil + max_depth as f64));

        //
        v.zip_map(self.frames[nr - n], |vs, r_lag| {
          vs.add_amp(
            (first * second * r_lag.to_sample::<f64>())
              .to_sample::<<Stereo<f32> as Frame>::Sample>()
              .to_signed_sample(),
          )
        })
      };

      let a = f64::consts::PI * (phir + n as f64);
      let first = f64::sin(a) / a;
      let second = 0.5 + 0.5 * f64::cos(a / (phir + max_depth as f64));
      v.zip_map(self.frames[nl + n], |vs, r_lag| {
        vs.add_amp(
          (first * second * r_lag.to_sample::<f64>())
            .to_sample::<<Stereo<f32> as Frame>::Sample>()
            .to_signed_sample(),
        )
      })
    })
  }
}

// an audio track
pub struct SincAudioTrack {
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
  frames: Vec<Stereo<f32>>,
  // interpolation
  interpolation: Interp,
  // elapsed frames as requested by audio
  elapsed_frames: u64,
  // filter bank
  filter_bank: BiquadFilter,
}
impl SincAudioTrack {
  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> SincAudioTrack {
    // filter
    let filter = BiquadFilter::create_filter(
      FilterType::LowPass(),
      FilterOp::UseQ(),
      44_100.0,       // rate
      44_100.0/2.0, // cutoff
      1.0,            // db gain
      1.0,            // q
      1.0,            // bw
      1.0,            //slope
    );

    // ring buffer for Sinc Interp
    let ring_buffer = ring_buffer::Fixed::from([Stereo::<f32>::equilibrium(); R_BUFF_LEN]);

    SincAudioTrack {
      command_rx,
      original_tempo: 120.0,
      playback_rate: 1.0,
      playing: false,
      volume: 0.5,
      frames: Vec::new(),
      interpolation: Interp {
        iterp_val: -1e-10,
        idx: 0,
        frames: ring_buffer,
      },
      elapsed_frames: 0,
      filter_bank: filter,
    }
  }

  // returns a buffer insead of frames one by one
  pub fn next_block(&mut self, size: usize) -> Vec<Stereo<f32>> {
    // take the slice
    // @TODO REMOVE ALLOCATION HERE
    let audio_buffer = self.take(size).collect();
    /*
     * HERE WE CAN PROCESS BY CHUNK
     */
    // send full buffer
    return audio_buffer;
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

    // convert to stereo frames
    self.frames = track_utils::to_stereo(samples);

    // reset
    self.reset();
  }

  // just iterate into the frame buffer
  fn next_frame(&mut self) -> Stereo<f32> {
    // grab next frame in the frames buffer
    let next_frame = self.frames[self.elapsed_frames as usize % self.frames.len()];
    self.elapsed_frames += 1;
    return next_frame;
  }

  // reset interp and counter
  fn reset(&mut self) {
    self.elapsed_frames = 0;
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
impl Iterator for SincAudioTrack {
  type Item = Stereo<f32>;

  // next!
  fn next(&mut self) -> Option<Self::Item> {
    // non blocking command fetch
    self.fetch_commands();

    // doesnt consume if not playing
    if !self.playing {
      return Some(Stereo::<f32>::equilibrium());
    }

    // advance frames
    while self.interpolation.iterp_val >= 1.0 {
      let next_frame = self.next_frame();
      self.interpolation.next_source_frame(next_frame);
      self.interpolation.iterp_val -= 1.0;
    }

    // apply interpolation
    let interp_val = self.interpolation.iterp_val;
    let next_i_frame = self.interpolation.interpolate(interp_val);
    self.interpolation.iterp_val += self.playback_rate;
    // println!("{:?}", next_i_frame);
    // return
    return Some(next_i_frame);
    /*
     * HERE WE CAN PROCESS BY FRAME
     */
    // FILTER BANK
    // let frame = self.filter_bank.process(frame);
  }
}
