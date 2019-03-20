mod midi;
mod audio;
mod sample_gen;

fn main() {
    // ->
    println!("Hello Sampler");

    // init midi inputs
    let (midi_thread, midi_rx) = midi::initialize_inputs();

    // init audio
    audio::initialize_audio(midi_rx);

    // wait fo midi thread to exit
    match midi_thread.join() {
        Ok(_) => println!("Midi Thread Exited Successfully"),
        Err(_) => println!("Midi Thread Errored"),
    }
}
