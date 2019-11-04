# MULTI MODAL PHRASE SAMPLER

An experiment using Rust for audio processing.
Rust is a promising new language to experiment with audio processing because it provides memory safety without the use of a Garbage Collector, which is a no-go with many real time audio applications.

## Goals

This phrase sampler is an attempt to create a basic headless audio loops player, that can be synchronised via MIDI or any other (and more suitable) Sync mechanism in the future.

## ROADMAP

- Having a sane audio engine beyond my knowledge.
- Properly use the sample crate, its gold.
- Implement features beyond simple sample player engines, as effects and mixers.
- Implement a control API accessible via Network along with a spec to later control the sampler with a proper GUI.
- Get rid of any C dependency.
- Being Platform-independent.
- Get rich.

## Links

- https://github.com/korken89/biquad-rs
