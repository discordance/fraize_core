extern crate bus;
extern crate elapsed;
extern crate hound;
extern crate num;
extern crate sample;
extern crate time_calc;

use self::bus::BusReader;
use self::hound::WavReader;
use self::sample::frame::Stereo;
use self::sample::interpolate::{Converter, Linear};
use self::sample::{signal, Frame, Sample, Signal};

use audio::filters::{FilterOp, FilterType, BiquadFilter};
use audio::track_utils;

// @TODO this is ugly but what to do without generics ?
type FramedSignal = signal::FromInterleavedSamplesIterator<std::vec::IntoIter<f32>, Stereo<f32>>;

// an audio track
pub struct BasicAudioTrack {
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
  // iterator / converter
  sample_converter: Converter<FramedSignal, Linear<Stereo<f32>>>,
  // filter bank
  filter_bank: BiquadFilter
}
impl BasicAudioTrack {
  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> BasicAudioTrack {
    
    // init dummy
    let mut signal = signal::from_interleaved_samples_iter::<Vec<f32>, Stereo<f32>>(Vec::new());
    let interp = Linear::from_source(&mut signal);
    let conv = signal.scale_hz(interp, 1.0);

    // filter
    let filter = BiquadFilter::create_filter(
      FilterType::LowPass(),
      FilterOp::UseQ(),
      44_100.0, // rate
      1000.0, // cutoff
      1.0, // db gain
      2.0, // q
      1.0, // bw
      1.0 //slope
    );

    BasicAudioTrack {
      command_rx,
      original_tempo: 120.0,
      playback_rate: 1.0,
      playing: false,
      volume: 0.5,
      samples: Vec::new(),
      sample_converter: conv,
      filter_bank: filter
    }
  }

  // returns a buffer insead of frames one by one
  pub fn next_block(&mut self, size: usize) -> Vec<Stereo<f32>> {
    // take the slice
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
    self.samples = reader
      .into_samples::<i16>()
      .filter_map(Result::ok)
      .map(i16::to_sample::<f32>)
      .collect();

    // parse and set original tempo
    let (orig_tempo, _beats) = track_utils::parse_original_tempo(path, self.samples.len());
    self.original_tempo = orig_tempo;

    // reloop to avoid clicks
    self.reloop();
  }

  // change playback speed
  fn respeed(&mut self) {
    self
      .sample_converter
      .set_sample_hz_scale(1.0 / self.playback_rate);
  }

  // reloop rewind the conv
  fn reloop(&mut self) {
    // cook it
    // efficent way to copy !??
    let mut signal =
      signal::from_interleaved_samples_iter::<Vec<f32>, Stereo<f32>>(self.samples.clone());

    // for interpolation
    let interp = Linear::from_source(&mut signal);

    let scaled = signal.scale_hz(interp, self.playback_rate);
    self.sample_converter = scaled;
  }

  // fetch commands from rx
  fn fetch_commands(&mut self) {
    match self.command_rx.try_recv() {
      Ok(command) => match command {
        ::midi::CommandMessage::Playback(playback_message) => match playback_message.sync {
          ::midi::SyncMessage::Start() => {
            self.reloop();
            self.playing = true;
          }
          ::midi::SyncMessage::Stop() => {
            self.playing = false;
            self.reloop();
          }
          ::midi::SyncMessage::Tick(_tick) => {
            let rate = playback_message.time.tempo / self.original_tempo;
            // changed tempo
            if self.playback_rate != rate {
              self.playback_rate = rate;
              self.respeed();
            }
          }
        },
      },
      _ => (),
    };
  }
}

// Implement `Iterator` for `AudioTrack`.
impl Iterator for BasicAudioTrack {
  type Item = Stereo<f32>;

  // next!
  fn next(&mut self) -> Option<Self::Item> {
    // non blocking command fetch
    self.fetch_commands();

    // doesnt consume if not playing
    if !self.playing {
      return Some(Stereo::<f32>::equilibrium());
    }

    // check if iterator is exhausted
    if self.sample_converter.is_exhausted() {
      self.reloop();
      return Some(Stereo::<f32>::equilibrium());
    }

    // else next
    let frame = self.sample_converter.next();

     /*
     * HERE WE CAN PROCESS BY FRAME
     */
    // FILTER BANK
    let frame = self.filter_bank.process(frame);

    // yield with the volume post fx
    return Some(frame.scale_amp(self.volume));
  }
}
