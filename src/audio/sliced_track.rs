extern crate bus;
extern crate hound;
extern crate sample;

use self::bus::BusReader;
use self::hound::WavReader;
use self::sample::frame::Stereo;
use self::sample::{signal, Frame, Sample, Signal};

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
  // onset positions
  positions: Vec<u32>,
  // original samples
  samples: Vec<f32>,
  // iterator
  signal_it : Box<Iterator<Item = Stereo<f32>> + Send>
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
      positions: vec![],
      samples: vec![],
      signal_it: Box::new(vec![].into_iter()),
    }
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
    let orig_tempo = track_utils::parse_original_tempo(path, self.samples.len());
    self.original_tempo = orig_tempo;

    // send for analytics :p
    self.positions = analytics::detect_onsets(self.samples.clone());

    // reloop
    self.reloop();
  }

  // reloop rewind the conv
  // abusing boxes
  fn reloop(&mut self) {
    let mut signal_slice = self.samples.clone().into_boxed_slice();
    let sig_framed: Box<[Stereo<f32>]> = sample::slice::to_boxed_frame_slice(signal_slice).unwrap();

    let mut sig_it: Box<Iterator<Item = &Stereo<f32>>> =
      Box::new(sig_framed.iter().take(0).chain(&[][..]));

    // // init
    let mut last_pos = 0;

    // stucturate
    for pos in self.positions.iter() {
      // jump
      if *pos == 0 {
        continue;
      }

      // compute the slided sample position according to current playback_rate
      let last_slided_pos = ((last_pos as f64) * 1.0 / self.playback_rate) as usize;
      let next_slided_pos = ((*pos as f64) * 1.0 / self.playback_rate) as usize;
      // println!("pos {} {}", pos, slided_pos);

      if *pos as usize >= next_slided_pos {
        sig_it = Box::new(
          sig_it.chain(
            sig_framed
              .iter()
              .skip(last_slided_pos)
              .take(next_slided_pos),
          ),
        );
      }
      last_pos = *pos;
    }

    let total: Vec<Stereo<f32>> = sig_it.cloned().collect();
    self.signal_it = Box::new(total.into_iter());
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
              self.reloop();
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

    // println!("whas {}", self.signal_it.next().unwrap()[0]);
    match self.signal_it.next() {
      Some(frame) => return Some(frame),
      None => {
        self.reloop();
        return Some(Stereo::<f32>::equilibrium())
      }
    }

    // doesnt consume if not playing
    // if !self.playing {
    // return Some(Stereo::<f32>::equilibrium());
    // }

    // // check if iterator is exhausted
    // if self.sample_converter.is_exhausted() {
    //   self.reloop();
    //   return Some(Stereo::<f32>::equilibrium());
    // }

    // // else next
    // let frame = self.sample_converter.next();

    //  /*
    //  * HERE WE CAN PROCESS BY FRAME
    //  */
    // // FILTER BANK
    // let frame = self.filter_bank.process(frame);

    // // yield with the volume post fx
    // return Some(frame.scale_amp(self.volume));
  }
}
