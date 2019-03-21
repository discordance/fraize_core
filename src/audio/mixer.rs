//! Audio Mixer defines structs and traits useful for sampler routing.
//! This is intended to be as modular as it can be.
extern crate bus;
extern crate sample;

use self::bus::BusReader;
use self::sample::frame::{Frame, Stereo};

use sample_gen::repitch::RePitchGen;
use sample_gen::{SampleGenerator, SmartBuffer};

/// AudioTrack is a AudioMixer track that embeds one sample generator and a chain of effects.
struct AudioTrack {
  /// The attached sample generator.
  generator: Box<SampleGenerator + 'static + Send>,
  /// Track's own audio buffer to write to. Avoid further memory allocations in the hot path.
  /// As we are using cpal, we dont know yet how to size it at init.
  /// A first audio round is necessary to get the size
  audio_buffer: Vec<Stereo<f32>>,
}

/// AudioTrack implementation.
impl AudioTrack {
  /// new init the track from a sample generator
  fn new(generator: Box<SampleGenerator + 'static + Send>) -> Self {
    AudioTrack {
      generator: generator,
      // we still dont know how much the buffer wants.
      // let's init at 512 and extend later.
      audio_buffer: Vec::with_capacity(512),
    }
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
  /// Tracks owned by the mixer.
  tracks: Vec<AudioTrack>,
  /// Clock ticks
  clock_ticks: u64,
  /// Command bus reader. Lockless bus to read command messages
  command_rx: BusReader<::midi::CommandMessage>,
}

/// AudioMixer implementation.
impl AudioMixer {
  /// for testing only
  pub fn new_test(command_rx: BusReader<::midi::CommandMessage>) -> Self {
    // load two samples
    let mut s1 = SmartBuffer::new_empty();
    let mut s2 = SmartBuffer::new_empty();

    // check errors
    // @TODO and error checking ?
    s1.load_wave("/Users/nunja/Documents/Audiolib/smplr/tick_4.wav");
    s2.load_wave("/Users/nunja/Documents/Audiolib/smplr/tick_4.wav");

    // create two gens
    let mut gen1 = RePitchGen::new();
    gen1.load_buffer(s1);
    let mut gen2 = RePitchGen::new();
    gen2.load_buffer(s2);

    // create two tracks
    let mut tracks = Vec::new();
    let track1 = AudioTrack::new(Box::new(gen1));
    let track2 = AudioTrack::new(Box::new(gen2));
    tracks.push(track1);
    tracks.push(track2);

    AudioMixer {
      tracks: tracks,
      clock_ticks: 0,
      command_rx: command_rx,
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

    // mix!
    for (i, frame_out) in block_out.iter_mut().enumerate() {
      // 64 bit mixer
      let mut acc = Stereo::<f64>::equilibrium();
      for track in self.tracks.iter_mut() {
        let frame = track.get_frame(i);
        acc[0] += frame[0] as f64;
        acc[1] += frame[1] as f64;
      }

      // write
      frame_out[0] = acc[0] as f32;
      frame_out[1] = acc[1] as f32;
    }
  }

  /// Reads commands from the bus
  fn fetch_commands(&mut self) {
    match self.command_rx.try_recv() {
      Ok(command) => match command {
        ::midi::CommandMessage::Playback(playback_message) => match playback_message.sync {
          ::midi::SyncMessage::Start() => {
            // unmute all tracks
            for track in self.tracks.iter_mut() {
              track.play();
            }
          }
          ::midi::SyncMessage::Stop() => {
            // mute all tracks
            for track in self.tracks.iter_mut() {
              track.stop();
            }
          }
          ::midi::SyncMessage::Tick(_tick) => {
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
      _ => (),
    };
  }
}
