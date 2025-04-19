# ujson

A simple, yet performant json parser implementaion based on parser combinators approach.

```
$ hyperfine -m 100 ./target/release/ours
Benchmark 1: ./target/release/ours
  Time (mean ± σ):      73.3 ms ±   2.1 ms    [User: 43.8 ms, System: 29.4 ms]
  Range (min … max):    69.7 ms …  80.2 ms    100 runs

$ hyperfine -m 100 ./target/release/serde
Benchmark 1: ./target/release/serde
  Time (mean ± σ):     152.5 ms ±   3.6 ms    [User: 76.8 ms, System: 75.5 ms]
  Range (min … max):   146.2 ms … 171.4 ms    100 runs
```
