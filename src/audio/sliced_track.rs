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
  // current pos in slices
  cursor: u32,
  // onset positions
  positions: Vec<u32>,
  // last onset
  last_onset: u32,
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
      cursor: 0,
      positions: Vec::new(),
      last_onset: 0,
      frames: Vec::new(),
    }
  }

  fn reset (&mut self) {
    self.elapsed_frames = 0;
    self.cursor = 0;
    self.last_onset = 0;
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
    let orig_tempo = track_utils::parse_original_tempo(path, samples.len());
    self.original_tempo = orig_tempo;

    // send for analytics :p
    self.positions = analytics::detect_onsets(samples.clone());

    // convert to stereo frames
    self.frames = track_utils::to_stereo(samples);

    self.reset();
  }

  #[inline(always)]
  fn compute_next_frame(&mut self) -> Stereo<f32> {

    let num_poss = self.positions.len() as u32;  
    let num_frames = self.frames.len() as u32;
    let scaled_num_frames = ((num_frames as f64) * (1.0 / self.playback_rate)) as u32;
    let scaled_elapsed = ((self.elapsed_frames as f64) * (1.0 / self.playback_rate)) as u32 % scaled_num_frames;

    self.elapsed_frames += 1;
    // let num_frames = self.frames.len() as i32;
    // let num_position = self.positions.len() as i32;
    // let scaled_num_frames = ((num_frames as f64) * (1.0 / self.playback_rate)) as i32;
    // let offset_per_slice = (scaled_num_frames-num_frames) / num_position;
    
    // let next_pos = self.positions.iter().find(|x| {
    //   self.elapsed_frames < **x as i32
    // });
    // let next_pos = match next_pos {
    //   Some(pos) => *pos as i32,
    //   None => num_frames,
    // };
    // println!("{}", next_pos-self.elapsed_frames);
    // self.elapsed_frames = (1 + self.elapsed_frames)%scaled_num_frames;

    // // 
    // // let curr_slided_pos = ((curr_pos as f64)*(1.0/self.playback_rate)) as i32;

    // // println!("-> {} {}", curr_pos, curr_slided_pos);

    // let next_pos = match next_pos {
    //   Some(pos) => *pos as i32,
    //   None => num_frames,
    // };
    // let next_x_pos = next_pos+offset_per_slice;
    // if self.elapsed_frames >= next_pos-offset_per_slice {
    //   self.elapsed_frames = (1 + self.elapsed_frames)%num_frames;
    //   return Stereo::<f32>::equilibrium()
    // } else {
    //   let next_frame = self.frames[self.cursor as usize];
    //   self.cursor = (1 + self.cursor)%num_frames;
    //   self.elapsed_frames = (1 + self.elapsed_frames)%num_frames;
    //   return next_frame
    // }
    // // if self.elapsed_frames < next_pos {

    // // } else {
    // //   self.elapsed_frames = (1 + self.elapsed_frames)%num_frames;
    // //   return Stereo::<f32>::equilibrium()
    // // }
    return Stereo::<f32>::equilibrium()
  }

  // fetch commands from rx
  fn fetch_commands(&mut self) {
    match self.command_rx.try_recv() {
      Ok(command) => match command {
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
    // non blocking midi command fetch
    self.fetch_commands();

    // does not consume extra cpu if not playing
    if !self.playing {
      return Some(Stereo::<f32>::equilibrium());
    }

    // compute next frame
    let next_frame = self.compute_next_frame();

    // return to iter
    return Some(next_frame);
  }
}
