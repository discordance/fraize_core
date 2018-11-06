// local mid lib
mod mid;
// local audio lib
mod audio;

fn main() {
    // ->
    println!("Hello Sampler");

    // init midi inputs
    let midi_thread = mid::initialize_inputs();

    // init audio
    audio::initialize_audio();

    // wait fo midi thread to exit
    match midi_thread.join() {
        Ok(_) => println!("Midi Thread Exited Successfully"),
        Err(_) => println!("Midi Thread Errored"),
    }
}
