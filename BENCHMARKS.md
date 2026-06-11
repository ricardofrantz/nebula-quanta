# Benchmark baseline

Baseline for bead `nebula-quanta-83w` and future SIMD work.

- Date: 2026-06-11
- Machine: AMD Ryzen 9 9900X 12-Core Processor
- Cores visible to OS: 18 CPUs (`lscpu`: 18 CPUs, 1 thread/core, 1 socket)
- Rust: `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Criterion: `0.5.1` (`criterion = "0.5"`)
- Command: `cargo bench`
- Initial conditions: Plummer, seed 42

| Group | N | Threads | Median |
| --- | ---: | ---: | ---: |
| tree build | 1,000 | n/a | 92.529 µs |
| tree build | 10,000 | n/a | 1.338 ms |
| BH force eval | 1,000 | 1 | 539.247 µs |
| BH force eval | 1,000 | 4 | 3.336 ms |
| BH force eval | 10,000 | 1 | 9.548 ms |
| BH force eval | 10,000 | 4 | 11.352 ms |
| direct force eval | 1,000 | scalar | 1.521 ms |
| direct force eval | 4,000 | scalar | 24.780 ms |

## BH force threading fix check (nebula-quanta-y7f)

- Date: 2026-06-11
- Commands:
  - `cargo bench --bench nbody -- bh_force`
  - `cargo run --release -- --mode barnes_hut --n {10000,50000} --steps 1 --dt 0.001 --theta 0.7 --epsilon 0.01 --init plummer --seed 42 --threads {1,4} --energy-sample-ratio 0.0`
- Change note: Barnes-Hut force evaluation uses the scoped-thread path at and above 50,000 particles. Below that threshold, `--threads > 1` falls back to the same single-thread traversal and setup as `--threads 1`.

| Group | N | Threads | Median / force_ms |
| --- | ---: | ---: | ---: |
| bh_force criterion | 1,000 | 1 | 696.32 µs |
| bh_force criterion | 1,000 | 4 (fallback) | 699.35 µs |
| bh_force criterion | 10,000 | 1 | 11.263 ms |
| bh_force criterion | 10,000 | 4 (fallback) | 10.836 ms |

Criterion rows are from a single back-to-back run on an idle host; the
fallback rows match `--threads 1` within run-to-run noise.
| one-step run | 10,000 | 1 | 22.204 ms |
| one-step run | 10,000 | 4 (fallback) | 22.399 ms |
| one-step run | 50,000 | 1 | 132.612 ms |
| one-step run | 50,000 | 4 (scoped threads) | 121.301 ms |
