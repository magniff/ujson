use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

use ujson;

fn criterion_benchmark(c: &mut Criterion) {
    let input = std::fs::read_to_string("data.json").unwrap();
    c.bench_function("ujson", |b| b.iter(|| ujson::from_str(black_box(&input))));
    c.bench_function("serde", |b| {
        b.iter(|| serde_json::from_str::<serde_json::Value>(black_box(&input)).unwrap())
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
