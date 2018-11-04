
// local mid lib
mod mid;

fn main() {

    // ->
    println!("Hello Sampler");

    // init midi inputs
    let midi_thread = mid::initialize_inputs();

    // wait fo midi thread to exit
    match midi_thread.join() {
        Ok(_) => println!("Midi Thread Exited Successfully"),
        Err(_) => println!("Midi Thread Errored")
    }
}
