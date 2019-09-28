#[macro_use]
extern crate serde;
extern crate aubio;
extern crate hound;
extern crate num;

mod audio;
mod config;
mod control;
mod midi;
mod osc;
mod sample_gen;
mod sampling;

fn main() {
    // ->
    println!("Hello Sampler");

    // load default config, immutable
    let conf = config::load_default();

    // init midi
    let (midi_thread, midi_rx) = midi::initialize_midi(conf.clone());

    // init midi osc
    let (osc_thread, osc_rx) = osc::initialize_osc(conf.clone());

    // init audio
    let audio_thread = audio::initialize_audio(conf.clone(), midi_rx);

    // wait fo audio thread to exit
    match audio_thread.join() {
        Ok(_) => println!("Audio Thread Exited Successfully"),
        Err(_) => println!("Audio Thread Errored"),
    }

    // wait fo midi thread to exit
    match midi_thread.join() {
        Ok(_) => println!("Midi Thread Exited Successfully"),
        Err(_) => println!("Midi Thread Errored"),
    }
}
