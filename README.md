# PHRASE SAMPLER

An experiment with Rust and Audio as a learning medium.
Rust is a promising new language to experiment with audio processing because it provides memory safety without the use of a Garbage Collector, which is a burden to many real time audio applications.

## Goals

This phrase sampler is an attempt to create a basic headless audio loops player, that can be synchronised via MIDI or any other (and more suitable) Sync mechanism in the future.

## TODO

- Implement features beyond simple sample player engines, as effects and mixers.
- Implement a proper headless sample lib manager.
- Implement MIDI controllable parameters.
- Implement a control API accessible via Network along with a spec to later control the sampler with a proper GUI.
- Get rid of any C dependency.
- Being Platform-independent.

