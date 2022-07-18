extern crate cauldron;

use cauldron::audio::AudioSegment;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

fn decode(filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut audio_seg = AudioSegment::read(filename)?;
    let mut samples = audio_seg.samples::<i16>()?;

    loop {
        match samples.next() {
            None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decoders");
    group.sample_size(20).measurement_time(Duration::new(20, 0));
    group.bench_function("decode_wav", |b| {
        b.iter(|| decode(black_box("benchmark/MLKDream.wav")))
    });
    group.bench_function("decode_flac", |b| {
        b.iter(|| decode(black_box("benchmark/MLKDream.flac")))
    });
    group.finish();
}

criterion_group!(benches, bench_decode);
criterion_main!(benches);
