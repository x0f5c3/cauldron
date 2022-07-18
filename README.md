# Cauldron

[![Build Status](https://travis-ci.com/deep110/cauldron.svg?branch=master)](https://travis-ci.com/deep110/cauldron)
[![Crates.io](https://img.shields.io/crates/v/cauldron.svg)](https://crates.io/crates/cauldron)
[![docs.rs](https://docs.rs/cauldron/badge.svg)](https://docs.rs/cauldron)

A lightweight implementation of decoders for popular used audio formats [Flac, Wav, Mp3, Ogg, etc.] in pure Rust.

## Features

Planned features are:

- Decode and maybe Encode support for the most popular audio codecs
- Providing a WASM API for web
- Try supporting `no_std` environment

## Codec Format Support Roadmap

| Format   | Flag         | Read        | Write       |
|----------|--------------|-------------|-------------|
| AAC      | `aac`        | -           | -           |
| Flac     | `flac`       | Done        | -           |
| MP3      | `mp3`        | InProgress  | -           |
| PCM      | `pcm`        | -           | -           |
| WAV      | `wav`        | Done        | InProgress  |
| Vorbis   | `vorbis`     | -           | -           |

## Usage

Add this to `Cargo.toml` file:

```toml
[dependencies]
cauldron = "0.0.2"
```

Example code:

```rust
use cauldron::audio::AudioSegment;
use cauldron::codecs::FormatFlag;

let mut audio_segment = match AudioSegment::read("<path-to-audio-file>", FormatFlag::WAV) {
Ok(f) => f,
Err(e) => panic ! ("Couldn't open example file: {}", e)
};

// display some audio info
println!("{}", audio_segment);


let samples: Vec<i32> = audio_segment.samples().unwrap().map( | r| r.unwrap()).collect();
println!("total samples {}", samples.len());
```

An example to play an audio can be found in `examples/play.rs`. To play any audio just run:

```shell
cargo run --example play <path-to-audio-file>
```

## Acknowledgements

* [Wav Reference Document](https://sites.google.com/site/musicgapi/technical-documents/wav-file-format#fmt)
* [Flac Reference Document](https://github.com/xiph/flac)
* [FFmpeg](https://trac.ffmpeg.org/wiki/AudioChannelManipulation) for some algorithm clarity and channel layout
  understanding
* [Claxon](https://github.com/ruuda/claxon) for code structure of Input streams

## Contributing

Right now project is still in very early stages, so I am not looking for any contributions, but if you see any bug or
improvement in existing implementation feel free to open an issue.
