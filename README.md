# 🍓Fraize

An opinionated multitrack **phrase/loop sampler**, written in **Rust**, with live performance in mind.

## What ?!

This is an experimental project aiming to create a (time-synced) **multitrack loop player/mangler**.

The basic idea is you can load folders of audio loop files and use it as a kind of Ableton Live for the poor peasant that can run on your Raspberry PI, Mac OSX, Linux + everything it can compile for.

The layout is quite simple:

- **Tracks** are linked to folders containing samples (one Track / one Folder).
- Each **Track** have a dedicated and configurable Sampler Engine (**SampleGen**).
- A **SampleGen** is basically a way **to play a loop in sync with a clock according to a method**. 
    - think *Timestretch* / *Beat Slicer* / *Re-Pitch* ... or any other way to sync a bunch of samples with a tick.
- Each **SampleGen** implements its own way to mangle samples for fun (beat repeats, freeze, pitch shifts ...).
- Each **Track** can have a chain of **FXs** on top ( *Filters* / *Delays* / *Reverbs* ...).
- All this machinery is configurable with just a **TOML** file.
- All **Tracks** are synced to a *MIDI Clock*, *Internal Clock*, *Ableton Link* or even *CV Clock* ...

## It runs headless

The **UI** is a separate project. 

The sampler will run on the network and can be controlled by the UI via OSC API.

## Features

- [X] Onset / BPM analysis
- [X] Slicer sample player
- [X] Phase Vocoder sample player (basic timestretch)
- [X] RePitch sample player (linear interpolation)
- [X] MIDI Controls (CC)
- [X] MIDI Clock (Virtual Midi Device)
- [X] OSC API
- [X] Debug GUI via imgui

## How it works ?

### GUI

@TODO

### Config

@TODO

### Samples

Each sample/loop present in the folders is loaded in memory then analysed for BPM detection / Beat detection / Onsets detection.

You can ease the work by setting directly the bpm in the file name, as in **amen_break_180bpm.wav**.

For this purpose, the **aubio** library is used extensively as a rust wrapper around the **C** API.

- [lib aubio](https://aubio.org/)
- [aubio rust bindings](https://github.com/discordance/aubio-rs)

## But why Rust ?

**Rust** is a very promising language for **realtime audio** because it provides:

- **high level of abstraction**, embrace software complexity with elegance and modernity.
- **memory safety**, never SEGFAULT, again.
- **speed**, in **C** ballpark if you are careful, + auto-vectorization, SIMD ...
- **fantastic tooling**, like compiling a CMake or a JUCE project is an old forgotten nightmare.
- **bounded execution times**, NO nondeterministic garbage collected latency.
- **compiler**, rustc is a real pair programmer.
- **community**, just as brillant as helpful.

But not everything is green in **Rustland**:

- **young**, libraries are mostly in infancy (unstable, sparse doc ...).
- **audio**, very small audio community, no Rust rewrite of Julian Storer yet.
- **UI**, still very lacking on the GUI side (but lots of efforts).
- **learning curve**, not so easy to grasp. It is a complex language and asks dedication.
- **verbosity / ugliness**, this is very subjective :)


## Roadmap (rough)

- [ ] Get contributors.
- [ ] Sane (no clicks / pops), synchronized audio engine for all **SampleGen**.
- [ ] Implement FXs.
- [ ] Implement a proper Timestretch **SampleGen**.
- [ ] Get rid of any C dependency.
- [ ] Test on a variety of platforms.
- [ ] Live preformance tests.

## Bookmarks

- https://github.com/korken89/biquad-rs
