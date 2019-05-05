//! Audio Mixer defines structs and traits useful for sampler routing.
//! This is intended to be as modular as it can be.
extern crate bus;
extern crate sample;

use self::bus::BusReader;
use self::sample::frame::{Frame, Stereo};

use sample_gen::repitch::RePitchGen;
use sample_gen::slicer::SlicerGen;
use sample_gen::pvoc::PVOCGen;
use sample_gen::{SampleGenerator, SmartBuffer};
use sampling::SampleLib;
use aubio::pvoc::Pvoc;

/// extending the StereoTrait for additional mixing power
pub trait StereoExt<F32> {
  fn pan(self, val: f32) -> Self;
}

impl StereoExt<f32> for Stereo<f32> {
  //
  fn pan(mut self, val: f32) -> Self {
    let angle = (std::f32::consts::FRAC_PI_2 - 0.0) * ((val- (-1.0)) / (1.0 - (-1.0)));
    self[0] = self[0]*angle.sin();
    self[1] = self[1]*angle.cos();
    self
  }

}

/// AudioTrack is a AudioMixer track that embeds one sample generator and a chain of effects.
struct AudioTrack {
  /// The attached sample generator.
  generator: Box<SampleGenerator + 'static + Send>,
  /// Track's own audio buffer to write to. Avoid further memory allocations in the hot path.
  /// As we are using cpal, we dont know yet how to size it at init.
  /// A first audio round is necessary to get the size
  audio_buffer: Vec<Stereo<f32>>,
  /// Gain is the gain value of the track, pre effects, smoothed
  gain: ::control::SmoothParam,
  /// Pan is the panning value of the track, pre effects, smoothed
  pan: ::control::SmoothParam,
  /// Bank index (track-locked)
  bank: usize,
  /// Direction parameter for sample selection (Up/Down).
  sample_select: ::control::DirectionalParam,
  /// Sample name to keep track for presets as the lib grows
  sample_name: String
}

/// AudioTrack implementation.
impl AudioTrack {
  /// new init the track from a sample generator
  fn new(generator: Box<SampleGenerator + 'static + Send>, bank: usize) -> Self {
    AudioTrack {
      generator,
      // we still dont know how much the buffer wants.
      // let's init at 512 and extend later.
      audio_buffer: Vec::with_capacity(512),
      gain: ::control::SmoothParam::new(0.0, 1.0),
      pan: ::control::SmoothParam::new(0.0, 0.0),
      sample_select: ::control::DirectionalParam::new(0.0, 0.0),
      bank,
      sample_name: String::from(""),
    }
  }

  /// Loads (moves) an arbitrary SmartBuffer in the gen.
  fn load_buffer(&mut self, buffer: &SmartBuffer) {
    // memorize
    self.sample_name = buffer.file_name.clone(); // @TODO Clone
    self.generator.load_buffer(buffer);
  }

  /// loads currently tracked smart buffer
  fn load_current_buffer(&mut self, sample_lib: &SampleLib) {
    // @TODO There is a clone here in audio path
    self.generator.load_buffer(sample_lib.get_sample_by_name(self.bank, self.sample_name.as_str()))
  }

  /// loads the first sample in the the bank
  fn load_first_buffer(&mut self, sample_lib: &SampleLib) {
    // @TODO There is a clone here in audio path
    let first = sample_lib.get_first_sample(self.bank);
    self.load_buffer(first)
  }

  /// loads the next sample in the the bank
  fn load_next_buffer(&mut self, sample_lib: &SampleLib) {
    // @TODO There is a clone here in audio path
    let next = sample_lib.get_sibling_sample(self.bank, self.sample_name.as_str(), 1);
    self.load_buffer(next)
  }

  /// loads the next sample in the the bank
  fn load_prev_buffer(&mut self, sample_lib: &SampleLib) {
    // @TODO There is a clone here in audio path
    let next = sample_lib.get_sibling_sample(self.bank, self.sample_name.as_str(), -1);
    self.load_buffer(next)
  }

  /// play the underlying sample gen
  fn play(&mut self) {
    self.generator.play();
  }


  /// pause the underlying sample gen
  fn stop(&mut self) {
    self.generator.stop();
  }

  /// synchronize the underlying samplegen
  fn sync(&mut self, global_tempo: u64, tick: u64) {
    self.generator.sync(global_tempo, tick);
  }

  /// process and fill next block of audio.
  fn fill_next_block(&mut self, size: usize) {
    // first check if the buffer is init
    if self.audio_buffer.len() == 0 {
      println!("init buffer to size: {}", size);
      self.audio_buffer = vec![Stereo::<f32>::equilibrium(); size];
    }
    // fill buffer
    self.generator.next_block(&mut self.audio_buffer);
  }

  /// Get frame at specific place
  fn get_frame(&self, index: usize) -> Stereo<f32> {
    match self.audio_buffer.get(index) {
      Some(f) => return *f,
      None => return Stereo::<f32>::equilibrium()
    }
  }
}

/// AudioMixer manage and mixes many AudioTrack.
/// Also take care of the control events routing.
pub struct AudioMixer {
  /// SampleLib, owned by the mixer
  sample_lib: SampleLib,
  /// Tracks owned by the mixer.
  tracks: Vec<AudioTrack>,
  /// Clock ticks are counted here to keep sync with tracks
  clock_ticks: u64,
  /// Command bus reader. Lockless bus to read command messages
  command_rx: BusReader<::control::ControlMessage>,
}

/// AudioMixer implementation.
impl AudioMixer {
  /// for testing only
  pub fn new_test(command_rx: BusReader<::control::ControlMessage>) -> Self {

    // init the sample lib, crash of err
    let sample_lib = ::sampling::init_lib().expect("Unable to load some samples, maybe an issue with the AUDIO_ROOT ?");

    // create two gens
    let mut gen1 = PVOCGen ::new();
    let mut gen2 = RePitchGen::new();

    // create two tracks
    let mut tracks = Vec::new();
    let mut track1 = AudioTrack::new(Box::new(gen1), 0);
    let mut track2 = AudioTrack::new(Box::new(gen2), 1);

    // load defaults
    track1.load_first_buffer(&sample_lib);
    track2.load_first_buffer(&sample_lib);

    // some some defaults
    tracks.push(track1);
//    tracks.push(track2);

    AudioMixer {
      tracks,
      command_rx,
      clock_ticks: 0,
      sample_lib,
    }
  }

  /// Get the number of tracks
  pub fn get_tracks_number(&self) -> usize {
    return self.tracks.len();
  }

  /// Reads blocks for all the tracks and mix them
  pub fn next_block(&mut self, block_out: &mut [Stereo<f32>]) {
    // first fetch commands
    self.fetch_commands();

    // get size
    let buff_size = block_out.len();

    // fill each tracks blocks
    for track in self.tracks.iter_mut() {
      track.fill_next_block(buff_size);
    }

    // MIX!
    for (i, frame_out) in block_out.iter_mut().enumerate() {
      // 64 bit mixer
      let mut acc = Stereo::<f64>::equilibrium();
      for track in self.tracks.iter_mut() {

        let mut frame = track.get_frame(i);

        // gain stage
        frame = frame.scale_amp(track.gain.get_param(buff_size));

        // pan stage
        frame = frame.pan(track.pan.get_param(buff_size));
//        println!("{:?}", frame);

        // mix stage
        acc[0] += frame[0] as f64;
        acc[1] += frame[1] as f64;
      }

      // write
      frame_out[0] = acc[0] as f32;
      frame_out[1] = acc[1] as f32;
    }
  }

  /// Reads commands from the bus.
  /// Must iterate to consume all messages at one buffer cycle.
  fn fetch_commands(&mut self) {
    loop {
      match self.command_rx.try_recv() {
        // we have a message
        Ok(command) => match command {
          // Change tracked Sample inside the bank
          ::control::ControlMessage::TrackSampleSelect { tcode: _, val, track_num} => {
            // check if tracknum is around
            let tr = self.tracks.get_mut(track_num);
            match tr {
              Some(t) => {
                // set the new selecta
                t.sample_select.new_value(val);

                // match the resulting dir enum
                match t.sample_select.get_param() {
                  ::control::Direction::Up(_) => {
                    t.load_next_buffer(&self.sample_lib);
                  },
                  ::control::Direction::Down(_) => {
                    t.load_prev_buffer(&self.sample_lib);
                  },
                  ::control::Direction::Stable(_) => {},
                }

              }
              _ => ()
            }
//            println!("change sample on track {}", track_num);
          }
          // Gain
          ::control::ControlMessage::TrackGain{tcode: _,  val, track_num} => {
            // check if tracknum is around
            let tr = self.tracks.get_mut(track_num);
            match tr {
              Some(t) => {
                // set the gain (max 1.2)
                t.gain.new_value(val * 1.2);
              }
              _ => ()
            }

          }
          // Gain
          ::control::ControlMessage::TrackPan{tcode: _,  val, track_num} => {
            // check if tracknum is around
            let tr = self.tracks.get_mut(track_num);
            match tr {
              Some(t) => {
                // set the gain (max 1.2)
                t.pan.new_value_scaled(val, -1.0, 1.0);
              }
              _ => ()
            }

          }
          // Playback management
          ::control::ControlMessage::Playback(playback_message) => match playback_message.sync {
            ::control::SyncMessage::Start() => {
              // unmute all tracks
              for track in self.tracks.iter_mut() {
                track.play();
              }
              self.clock_ticks = 0;
            }
            ::control::SyncMessage::Stop() => {
              // mute all tracks
              for track in self.tracks.iter_mut() {
                track.stop();
              }
              self.clock_ticks = 0;
            }
            ::control::SyncMessage::Tick(_tick) => {
              // update tracks sync
              let global_tempo = playback_message.time.tempo;
              for track in self.tracks.iter_mut() {
                track.sync(global_tempo as u64, self.clock_ticks);
              }
              // inc ticks received by the mixer
              self.clock_ticks += 1;
            }
          },
        },
        // its empty
        _ => return,
      };
    } // loop
  }
}
