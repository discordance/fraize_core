extern crate bus;
extern crate hound;
extern crate sample;

use self::bus::BusReader;
use self::hound::WavReader;
use self::sample::frame::Stereo;
use self::sample::{Frame, Sample, Signal};

use audio::analytics;
use audio::track_utils;

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
  // elapsed frames
  elapsed_frames: u32,
  // onset positions
  positions: Vec<u32>,
  // original samples, framed
  frames: Vec<Stereo<f32>>,
}
impl SlicedAudioTrack {
  // constructor
  pub fn new(command_rx: BusReader<::midi::CommandMessage>) -> SlicedAudioTrack {
    SlicedAudioTrack {
      command_rx,
      original_tempo: 130.0,
      playback_rate: 1.0,
      playing: false,
      volume: 0.5,
      elapsed_frames: 0,
      positions: Vec::new(),
      frames: Vec::new(),
    }
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

    println!("len samples {}", samples.len()); 

    // send for analytics :p
    self.positions = analytics::detect_onsets(samples.clone());

    // convert to stereo frames
    self.frames = track_utils::to_stereo(samples); 

    // parse and set original tempo
    let orig_tempo = track_utils::parse_original_tempo(path, self.frames.len());
    self.original_tempo = orig_tempo;

    // setup
    self.slice();
  }

  // reloop rewind the conv
  // abusing boxes
  pub fn slice(&mut self) {

    // let mut mutable_iter = self.signal_iter;
    // *mutable_iter = self.frames.iter().cloned();

    // let mut last_pos = 0;

    // for pos in self.positions.iter() {
    //   // jump
    //   if *pos == 0 {
    //     continue;
    //   }
    //   let new_pos = *pos as usize;
    //   if self.elapsed_frames < *pos {
    //     self.signal_it = Arc::new(signal.skip(last_pos).take(new_pos-last_pos));
    //     return;
    //   }
    //   last_pos = new_pos;
    // }
    //   // compute the slided sample position according to current playback_rate
    //   let last_slided_pos = ((last_pos as f64) * 1.0 / self.playback_rate) as usize;
    //   let next_slided_pos = ((*pos as f64) * 1.0 / self.playback_rate) as usize;
  }

  // fetch commands from rx
  fn fetch_commands(&mut self) {
    match self.command_rx.try_recv() {
      Ok(command) => match command {
        ::midi::CommandMessage::Playback(playback_message) => match playback_message.sync {
          ::midi::SyncMessage::Start() => {
            self.elapsed_frames = 0;
            self.playing = true;
            // self.slice();
          }
          ::midi::SyncMessage::Stop() => {
            self.elapsed_frames = 0;
            self.playing = false;
            // self.slice();
          }
          ::midi::SyncMessage::Tick(_tick) => {
            let rate = playback_message.time.tempo / self.original_tempo;
            // changed tempo
            if self.playback_rate != rate {
              self.playback_rate = rate;
              // self.slice();
            }
          }
        },
      },
      _ => (),
    };
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
}

// Implement `Iterator` for `AudioTrack`.
impl Iterator for SlicedAudioTrack {
  type Item = Stereo<f32>;

  // next!
  fn next(&mut self) -> Option<Self::Item> {
    
    // non blocking command fetch
    self.fetch_commands();

    // doesnt consume if not playing
    if !self.playing {
      return Some(Stereo::<f32>::equilibrium());
    }

    let next_frame = self.frames[self.elapsed_frames as usize];
    self.elapsed_frames += 1;
    return Some(next_frame);
    // match next_frame {
    //   Some(f) => return Some(f),
    //   None => return Some(Stereo::<f32>::equilibrium()),
    // }
  }
}
