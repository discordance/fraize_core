extern crate bus;
extern crate hound;
extern crate num;
extern crate pvoc;
extern crate sample;
extern crate time_calc;

use self::bus::BusReader;
use self::hound::WavReader;
use self::pvoc::{Bin, PhaseVocoder};
use self::sample::frame::Stereo;
use self::sample::{Frame, Sample};

use audio::track_utils;
use std::slice;

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
  frames: Vec<Stereo<f32>>,
  // elapsed frames as requested by audio
  elapsed_frames: u64,
  // phase vocoder
  pvoc: PhaseVocoder,
  // buffers to avoid too much allocs for the pvoc input
  voc_in_buff: [Vec<f32>; 2],
  voc_out_buff: [Vec<f32>; 2],
}

impl PvocAudioTrack {
  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> PvocAudioTrack {
    // init pvoc
    let pvoc = PhaseVocoder::new(2, 44100.0, 256, 4);

    // init the pvoc buffers
    let voc_in_buff = [vec![0f32; 1024], vec![0f32; 1024]];
    let voc_out_buff = [vec![0f32; 1024], vec![0f32; 1024]];

    PvocAudioTrack {
      command_rx,
      original_tempo: 120.0,
      playback_rate: 1.0,
      playing: false,
      volume: 0.5,
      frames: Vec::new(),
      elapsed_frames: 0,
      pvoc,
      voc_in_buff,
      voc_out_buff,
    }
  }

  // returns a buffer insead of frames one by one
  pub fn next_block(&mut self, size: usize) -> Vec<Stereo<f32>> {
    // fill in the pvoc input buffer
    let mut filled = 0;
    while filled < size {
      let next_frame = self.next();
      match next_frame {
        Some(frame) => {
          self.voc_in_buff[0][filled] = frame[0];
          self.voc_in_buff[1][filled] = frame[1];
        }
        None => (),
      }
      filled += 1;
    }

    let in_slices = self
      .voc_in_buff
      .iter()
      .take(2)
      .map(|x| &x[..size])
      .collect::<Vec<_>>();

    let mut out_slices = self
      .voc_out_buff
      .iter_mut()
      .take(2)
      .map(|x| &mut x[..size])
      .collect::<Vec<_>>();

    self.pvoc.process(
      &in_slices[..],
      &mut out_slices[..],
      |channels: usize, bins: usize, input: &[Vec<Bin>], output: &mut [Vec<Bin>]| {
        for i in 0..channels {
          for j in 0..bins {
            // output[i][j] = input[i][j]; // change this!
            let index = ((j as f64) * 0.5) as usize;
            if index < bins / 2 {
                output[i][index].freq = input[i][j].freq * 0.5;
                output[i][index].amp += input[i][j].amp;
            }
          }
        }
      },
    );

    let out_vec = out_slices[0]
      .iter().take(size)
      .zip(out_slices[1].iter().take(size))
      .map(|(l, r)| [*l, *r])
      .collect::<Vec<_>>();

    // send full buffer
    return out_vec;
  }

  // load audio file
  pub fn load_file(&mut self, path: &str) {
    // load some audio
    let reader = WavReader::open(path).unwrap();

    // samples preparation
    let mut samples: Vec<f32> = reader
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
impl Iterator for PvocAudioTrack {
  type Item = Stereo<f32>;

  // next!
  fn next(&mut self) -> Option<Self::Item> {
    // non blocking command fetch
    self.fetch_commands();

    // doesnt consume if not playing
    if !self.playing {
      return Some(Stereo::<f32>::equilibrium());
    }

    // gte next frame
    let next_frame = self.next_frame();

    // return
    return Some(next_frame);
    /*
     * HERE WE CAN PROCESS BY FRAME
     */
    // FILTER BANK
    // let frame = self.filter_bank.process(frame);
  }
}
