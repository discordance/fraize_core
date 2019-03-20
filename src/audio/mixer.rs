//! Audio Mixer defines structs and traits useful for sampler routing.
//! This is intended to be as modular as it can be.
extern crate sample;

use self::sample::frame::Stereo;
use sample_gen::{SampleGenerator, SmartBuffer};
use sample_gen::repitch::{RePitchGen};

/// AudioTrack is a AudioMixer track that embeds one sample generator and a chain of effects.
struct AudioTrack {
  /// The attached sample generator.
  generator: Box<SampleGenerator + 'static + Send>,
  /// Track's own audio buffer to write to. Avoid further memory allocations in the hot path.
  /// As we are using cpal, we dont know yet how to size it at init. 
  /// A first audio round is necessary to get the size
  audio_buffer: Vec<Stereo<f32>>
}

/// AudioTrack implementation.
impl AudioTrack {
  /// new init the track from a sample generator
  fn new(generator: Box<SampleGenerator + 'static + Send>) -> Self {
    AudioTrack{
      generator: generator,
      // we still dont know how much the buffer wants.
      // let's init at 512 and extend later.
      audio_buffer: Vec::with_capacity(512)
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
}

/// AudioMixer implementation.
impl AudioMixer {
  /// for testing only
  pub fn new_test() -> Self {
    // load two samples
    let mut s1 = SmartBuffer::new_empty();
    let mut s2 = SmartBuffer::new_empty();

    // check erros
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

    AudioMixer{
      tracks: tracks,
      clock_ticks: 0,
    }
  }

  /// Get the number of tracks
  pub fn get_tracks_number(&self) -> usize {
    return self.tracks.len();
  }
}
