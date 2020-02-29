//! Audio Mixer defines structs and traits useful for sampler routing.
//! This is intended to be as modular as it can be.
extern crate sample;
extern crate crossbeam_channel;

use self::sample::frame::{Frame, Stereo};

use config::{Config, TrackType};
use sample_gen::pvoc::PVOCGen;
use sample_gen::repitch::RePitchGen;
use sample_gen::slicer::SlicerGen;
use sample_gen::{SampleGenerator, SmartBuffer};
use sampling::SampleLib;
use control::ControlMessage;

/// extending the Stereo Trait for additional mixing power
pub trait StereoExt<F32> {
    fn pan(self, val: f32) -> Self;
}

impl StereoExt<f32> for Stereo<f32> {
    //
    fn pan(mut self, val: f32) -> Self {
        let angle = (std::f32::consts::FRAC_PI_2 - 0.0) * ((val - (-1.0)) / (1.0 - (-1.0)));
        self[0] = self[0] * angle.cos();
        self[1] = self[1] * angle.sin();
        self
    }
}

/// AudioTrack is a AudioMixer track that embeds one sample generator and a chain of effects.
struct AudioTrack {
    /// The attached sample generator.
    generator: Box<dyn SampleGenerator + 'static + Send>,
    /// Track's own audio buffer to write to. Avoid further memory allocations in the hot path.
    /// As we are using cpal, we dont know yet how to size it at init.
    /// A first audio round is necessary to get the size
    audio_buffer: Vec<Stereo<f32>>,
    /// Volume is the volume value of the track, pre effects, smoothed
    volume: ::control::SmoothParam,
    /// Pan is the panning value of the track, pre effects, smoothed
    pan: ::control::SmoothParam,
    /// Bank index (track-locked)
    bank: usize,
    /// Direction parameter for sample selection (Up/Down).
    sample_select: ::control::DirectionalParam,
    /// Sample name to keep track for presets as the lib grows
    sample_name: String,
}

/// AudioTrack implementation.
impl AudioTrack {
    /// new init the track from a sample generator
    fn new(generator: Box<dyn SampleGenerator + 'static + Send>, bank: usize) -> Self {
        AudioTrack {
            generator,
            // we still dont know how much the buffer wants.
            // let's init at 512 and extend later.
            audio_buffer: Vec::with_capacity(512),
            volume: ::control::SmoothParam::new(0.0, 1.0),
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
        self.generator
            .load_buffer(sample_lib.get_sample_by_name(self.bank, self.sample_name.as_str()))
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

    /// set loop div
    fn set_loop_div(&mut self, loop_div: u64) {
        self.generator.set_loop_div(loop_div);
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
            None => return Stereo::<f32>::equilibrium(),
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
    command_rx: crossbeam_channel::Receiver<ControlMessage>,
}

/// AudioMixer implementation.
impl AudioMixer {
    /// init a new mixer, a lot of heavy lifting here
    pub fn new(conf: Config, command_rx: crossbeam_channel::Receiver<ControlMessage>) -> Self {
        // init the sample lib, crash of err
        let sample_lib = ::sampling::init_lib(conf.clone())
            .expect("Unable to load some samples, maybe an issue with the AUDIO_ROOT in conf ?");

        // create tracks according to the config
        let mut tracks = Vec::new();
        for t in conf.tracks.iter() {
            match t {
                TrackType::RePitchGen { bank } => {
                    let gen = RePitchGen::new();
                    let mut track = AudioTrack::new(Box::new(gen), *bank);
                    track.load_first_buffer(&sample_lib);
                    tracks.push(track);
                }
                TrackType::SlicerGen { bank } => {
                    let gen = SlicerGen::new();
                    let mut track = AudioTrack::new(Box::new(gen), *bank);
                    track.load_first_buffer(&sample_lib);
                    tracks.push(track);
                }
                TrackType::PVOCGen { bank } => {
                    let gen = PVOCGen::new();
                    let mut track = AudioTrack::new(Box::new(gen), *bank);
                    track.load_first_buffer(&sample_lib);
                    tracks.push(track);
                }
            }
        }

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

                // volume stage
                frame = frame.scale_amp(track.volume.get_param(buff_size));

                // pan stage
                frame = frame.pan(track.pan.get_param(buff_size));
   
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
    /// Must iterate to consume all messages for one buffer cycle util its empty.
    /// This is not fetching commands at sample level.
    fn fetch_commands(&mut self) {
        // loop until all simultaneous commands are fetched
        loop {
            match self.command_rx.try_recv() {
                // we have a message
                Ok(command) => match command {
                    // Change tracked Sample inside the bank
                    ::control::ControlMessage::TrackSampleSelect {
                        tcode: _,
                        val,
                        track_num,
                    } => {
                        // check if tracknum is around
                        let tr = self.tracks.get_mut(track_num);
                        if let Some(t) = tr {
                            // set the new selecta
                            t.sample_select.new_value(val);

                            // match the resulting dir enum
                            match t.sample_select.get_param() {
                                ::control::Direction::Up(_) => {
                                    t.load_next_buffer(&self.sample_lib);
                                }
                                ::control::Direction::Down(_) => {
                                    t.load_prev_buffer(&self.sample_lib);
                                }
                                ::control::Direction::Stable(_) => {}
                            }
                        }
                    },
                    // Next Sample
                    ::control::ControlMessage::TrackNextSample {
                        tcode: _,
                        track_num,
                    } => {
                        // check if tracknum is around
                        let tr = self.tracks.get_mut(track_num);
                        if let Some(t) = tr {
                            // set the next sample
                            t.load_next_buffer(&self.sample_lib);
                        }
                    },
                    // Previous Sample
                    ::control::ControlMessage::TrackPrevSample {
                        tcode: _,
                        track_num,
                    } => {
                        // check if tracknum is around
                        let tr = self.tracks.get_mut(track_num);
                        if let Some(t) = tr {
                            // set the prev sample
                            t.load_prev_buffer(&self.sample_lib);
                        }
                    },
                    // Volume
                    ::control::ControlMessage::TrackVolume {
                        tcode: _,
                        val,
                        track_num,
                    } => {
                        // check if tracknum is around
                        let tr = self.tracks.get_mut(track_num);
                        if let Some(t) = tr {
                            // set the volume
                            t.volume.new_value(val);
                        }
                    }
                    // Pan
                    ::control::ControlMessage::TrackPan {
                        tcode: _,
                        val,
                        track_num,
                    } => {
                        // check if tracknum is around
                        let tr = self.tracks.get_mut(track_num);
                        if let Some(t) = tr {
                            // set the pan
                            t.pan.new_value(val);
                        }
                    }
                    // LoopDiv
                    ::control::ControlMessage::TrackLoopDiv {
                        tcode: _,
                        val,
                        track_num,
                    } => {
                        // check if tracknum is around
                        let tr = self.tracks.get_mut(track_num);
                        if let Some(t) = tr {
                            // set the loop div
                            t.set_loop_div(val);
                        }
                    }
                    // Playback management
                    ::control::ControlMessage::Playback(playback_message) => {
                        match playback_message.sync {
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
                        }
                    }
                    // got a slicer message, we just find the right track and pass down to the generator implementation
                    ControlMessage::Slicer { tcode: _, track_num, message: _ } => {
                        // check if tracknum is around
                        let tr = self.tracks.get_mut(track_num);
                        if let Some(t) = tr {
                            t.generator.push_control_message(command);
                        }
                    }
                },
                // its empty
                _ => return,
            };
        } // loop
    }
}
