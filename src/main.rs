#[macro_use]
extern crate serde;

mod audio;
mod config;
mod control;
mod midi;
mod osc;
mod sample_gen;
mod sample_lib;

///
///     |~~~~~~~~~~~~|
///     |~~~~~~~~~~~~|
///     |            |
/// /~~\|        /~~\|
/// \__/         \__/
/// 
/// Musicalement votre. Pouet.
fn main() {
    // ->
    println!("Hello Sampler");

    // load default config, immutable
    let conf = config::load_default();

    // init midi
    let (midi_thread, midi_rx) = midi::initialize_midi(conf.clone());

    // init midi osc
    let (osc_thread, osc_in, osc_out_rx) = osc::initialize_osc(conf.clone());

    // init the control hub
    let (_control_hub, hub_rx) = control::ControlHub::new(conf.clone(), osc_in, osc_out_rx, midi_rx);

    // init audio
    let audio_thread = audio::initialize_audio(conf.clone(), hub_rx);

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

    // wait fo osc thread to exit
    match osc_thread.join() {
        Ok(_) => println!("OSC Thread Exited Successfully"),
        Err(_) => println!("OSC Thread Errored"),
    }
}
