# 🍓Fraize

An opinionated multitrack **phrase/loop sampler**, written in **Rust**, with live performance in mind.

## What ?!

This is an experimental project aiming to create a (time-synced) **multitrack loop player/mangler**.

The basic idea is you can load folders of audio loop files and use it as a kind of Ableton Live for the poor peasant that can run on your Raspberry PI, Mac OSX, Linux + everything it can compile for.

The layout is quite simple:

- **Tracks** are linked to folders containing samples (one Track / one Folder).
- Each **Track** have a dedicated and configurable Sampler Engine (**SampleGen**).
- A **SampleGen**, or Sampler Engine, just means `how to play this loop in sync with a clock`; 
    
    think **Timestretch** / **Beat Slicer** / **Repitch** ...
- Each **SampleGen** implements its own way to mangle samples for fun (x2 playback, beat repeats, freeze ...).
- Each **Track** can have a chain of FX on top ( Filters / Delays / Reverbs ...).
- All this machinery is configurable with just a **TOML** file. 

## How it works ?

### GUI

@TODO

### Config

@TODO

### Samples

Each sample/loop present in the folders is loaded in memory then analysed for BPM detection / Beat detection / Onsets detection.
You can ease the work by setting directly the bpm in the file name, as in **amen_break_180bpm.wav**.

For this purpose, the **aubio** library is used extensively as a rust wrapper around the **C** API.
[Aubio](https://aubio.org/)
[Aubio Rust bindings](https://github.com/discordance/aubio-rs)

## But why Rust ?

**Rust** is a very promising language for **realtime audio** because it provides:

- **high level of abstraction**, embrace software complexity with elegance and modernity.
- **memory safety**, never SEGFAULT, again.
- **compiler**, rustc is a pair programmer.
- **speed**, as in **C** ballpark if you are careful, + auto-vectorization, SIMD ...
- **fantastic tooling**, like compiling a CMake or a JUCE project is an old forgotten nightmare.
- **bounded execution times**, NO nondeterministic garbage collected latency.
- **community**, just as brillant as helpful.

But not everything is green in **Rustland**:

- **young**, libraries are mostly in infancy (unstable, sparse doc ...).
- **audio**, very small audio community, no Rust version of Julian Storer yet.
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
