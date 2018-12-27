extern crate bus;
extern crate hound;
extern crate sample;
extern crate time_calc;

use self::bus::BusReader;
use self::hound::WavReader;
use self::sample::frame::Stereo;
use self::sample::{Frame, Sample};
use self::time_calc::{Ppqn, Ticks};

use audio::analytics;
use audio::track_utils;

// clock resolution

const PPQN: Ppqn = 24;
const QUANT: u32 = 16;

// a slicer track
pub struct SlicedAudioTrack {
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
  // elapsed frames as requested by audio
  elapsed_frames: u64,
  // current clock ticks
  ticks: u64,
  // onset positions
  positions: Vec<u32>,
  // prev slice memory
  pslice: usize,
  // cursor in the buffer
  cursor: i64,
  // original samples, framed
  frames: Vec<Stereo<f32>>,
}
impl SlicedAudioTrack {
  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> SlicedAudioTrack {
    SlicedAudioTrack {
      command_rx,
      original_tempo: 120.0,
      playback_rate: 1.0,
      playing: false,
      volume: 0.5,
      elapsed_frames: 0,
      ticks: 0,
      positions: Vec::new(),
      pslice: 0,
      cursor: 0,
      frames: Vec::new(),
    }
  }

  // reset counters
  fn reset(&mut self) {
    self.elapsed_frames = 0;
    self.ticks = 0;
    self.pslice = self.positions.len() - 1;
    self.cursor = 0;
  }

  // load audio file
  pub fn load_file(&mut self, path: &str) {
    // load some audio
    let reader = WavReader::open(path).unwrap();

    // samples conv from 16bit to f32
    let mut samples: Vec<f32> = reader
      .into_samples::<i16>()
      .filter_map(Result::ok)
      .map(i16::to_sample::<f32>)
      .collect();

    // parse and set original tempo
    let (orig_tempo, beats) = track_utils::parse_original_tempo(path, samples.len());
    self.original_tempo = orig_tempo;

    // send for analytics :p
    let mut positions = analytics::detect_onsets(samples.clone());

    // convert to stereo frames
    self.frames = track_utils::to_stereo(samples);

    // last postition to push
    positions.push(self.frames.len() as u32);

    // quantize the slices
    let quantized = track_utils::quantize_pos(
      &positions,
      self.frames.len() as u32 / (QUANT * beats as u32),
    );
    self.positions = quantized;

    // reset counters
    self.reset();
  }

  // @TODO What the heck this is too much casting
  fn compute_next_frame(&mut self, tick_frame: bool) -> Stereo<f32> {
    // total number of frames in the buffer
    let num_frames = self.frames.len() as i64;

    // number of slices
    let num_slices = self.positions.len() as i64;

    // how many frames elapsed from the clock point of view
    let clock_frames = Ticks(self.ticks as i64).samples(self.original_tempo, PPQN, 44_100.0) as i64;

    // cycles
    let cycle = (clock_frames as f32 / num_frames as f32) as i64;

    // next slice
    let next_slice = self
      .positions
      .iter()
      .position(|&x| x as i64 + (cycle * num_frames) > clock_frames);
    let next_slice = match next_slice {
      Some(idx) => idx,
      None => 0,
    };

    // curr slice
    let curr_slice = (next_slice as i64 - 1) % num_slices;

    if self.pslice != curr_slice as usize {
      // reset cursor
      self.cursor = 0;
      self.pslice = curr_slice as usize;
    }

    // get this slice len in samples
    let slice_len = self.positions[next_slice as usize] - self.positions[curr_slice as usize];

    // init nextframe to silence
    let mut next_frame = Stereo::<f32>::equilibrium();

    // we have still samples to read in this slice ?
    if (slice_len as i64 - self.cursor) > 0 {
      // get the right index in buffer
      let mut findex = self.cursor as u32 + self.positions[curr_slice as usize];

      // dont overflow the buffer with wrapping
      findex = findex % num_frames as u32;

      // get next frame, apply fade in/out env
      next_frame = self.frames[findex as usize]
        .scale_amp(track_utils::fade_in(self.cursor, 64))
        .scale_amp(track_utils::fade_out(
          self.cursor,
          1024 * 4,
          slice_len as i64,
        )) // @TODO must be relative with the speed
        .scale_amp(2.0);

      // println!("{}", track_utils::fade_out(self.cursor, 128, slice_len as u64));
      self.cursor += 1;
    }

    // increment of counters
    if tick_frame {
      self.ticks += 1;
    }
    // usefoul ?
    self.elapsed_frames += 1;

    // return
    return next_frame;
  }

  // fetch commands from rx, return true if received tick for latter sync
  fn fetch_commands(&mut self) -> bool {
    // init tick flag
    let mut tick_received = false;

    match self.command_rx.try_recv() {
      Ok(command) => match command {
        // fetch playback
        ::midi::CommandMessage::Playback(playback_message) => match playback_message.sync {
          ::midi::SyncMessage::Start() => {
            self.reset();
            self.playing = true;
          }
          ::midi::SyncMessage::Stop() => {
            self.reset();
            self.playing = false;
          }
          ::midi::SyncMessage::Tick(_tick) => {
            let rate = playback_message.time.tempo / self.original_tempo;
            // changed tempo
            if self.playback_rate != rate {
              self.playback_rate = rate;
            }
            // we received a tick, but we need to inc it later
            tick_received = true;
          }
        },
      },
      _ => (),
    };

    // return tick received
    return tick_received;
  }

  // returns a buffer insead of frames one by one
  pub fn next_block(&mut self, size: usize) -> Vec<Stereo<f32>> {
    // take the size
    // @TODO REMOVE THE ALLOCATION HERE
    let audio_buffer = self.take(size).collect();
    /*
     * HERE WE CAN PROCESS BY CHUNK
     */
    // send full buffer
    return audio_buffer;
  }
}

// Implement `Iterator` for `SlicedAudioTrack`.
impl Iterator for SlicedAudioTrack {
  type Item = Stereo<f32>;

  // next!
  fn next(&mut self) -> Option<Self::Item> {
    // non blocking midi command fetch
    let tick_frame = self.fetch_commands();

    // does not consume extra cpu if not playing
    if !self.playing {
      return Some(Stereo::<f32>::equilibrium());
    }

    // compute next frame
    let next_frame = self.compute_next_frame(tick_frame);

    // return to iter
    return Some(next_frame);
  }
}
