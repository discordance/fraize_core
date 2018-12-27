extern crate basic_dsp;
extern crate bus;
extern crate elapsed;
extern crate hound;
extern crate num;
extern crate sample;
extern crate time_calc;

use self::bus::BusReader;
use self::hound::WavReader;
use self::sample::frame::Stereo;
use self::sample::{Frame, Sample};

use self::basic_dsp::conv_types::SincFunction;
use self::basic_dsp::{FromVector, InterpolationOps, SingleBuffer, ToComplexVector, Vector};

use audio::filters::{BiquadFilter, FilterOp, FilterType};
use audio::track_utils;

const INTERP_SIZE: usize = 16;

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
  // original samples in frame
  frames: Vec<Stereo<f32>>,
  // interp buffer
  interp_buffer: SingleBuffer<f32>,
  // interp function
  sinc_function: SincFunction<f32>,
  // to ipol
  prev_samples: [f32; INTERP_SIZE],
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
      44_100.0, // rate
      44_100.0/2.0, // cutoff
      1.0,      // db gain
      1.0,      // q
      1.0,      // bw
      1.0,      //slope
    );

    SincAudioTrack {
      command_rx,
      original_tempo: 120.0,
      playback_rate: 1.0,
      playing: false,
      volume: 0.5,
      frames: Vec::new(),
      interp_buffer: SingleBuffer::new(),
      sinc_function: SincFunction::new(),
      prev_samples: [0.0; INTERP_SIZE],
      elapsed_frames: 0,
      filter_bank: filter,
    }
  }

  // returns a buffer instead of frames one by one
  pub fn next_block(&mut self, size: usize) -> Vec<Stereo<f32>> {
    // non blocking command fetch
    self.fetch_commands();

    // doesnt consume if not playing
    if !self.playing {
      return (0..size).map(|_x| Stereo::<f32>::equilibrium()).collect();
    }

    // how much we need to prune
    let to_take = ((size as f64) * self.playback_rate) as usize;

    // to interleaved
    let interleaved: Vec<f32> = self.take(to_take).flat_map(|x| x.to_vec()).collect();

    // to complex
    let mut complex = interleaved.to_complex_time_vec();

    // replace with !
    // https://github.com/lrbalt/libsoxr-rs
    complex.interpolatef(
      &mut self.interp_buffer,
      &self.sinc_function,
      (1.0 / self.playback_rate) as f32,
      0.0,
      INTERP_SIZE,
    );

    // re-frame the signal
    let chunked = complex.to_slice().chunks(2);
    let mut out = Vec::new();
    for chunk in chunked {
      let c = [chunk[0], chunk[1]];
      out.push(c);
    }

    // send full buffer
    return out;
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

    println!("{}", self.frames.len());

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
    let next_frame = self.next_frame();

    // return
    return Some(next_frame);
  }
}
