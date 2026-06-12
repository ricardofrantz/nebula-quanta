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
- Commands (historical pre-`nebula-quanta-pf3` baseline):
  - `cargo bench --bench nbody -- bh_force`
  - `cargo run --release -- --mode barnes_hut --n {10000,50000} --steps 1 --dt 0.001 --theta 0.7 --epsilon 0.01 --init plummer --seed 42 --threads {1,4} --energy-sample-ratio 0.0`
- Current note: Barnes-Hut force evaluation now uses the strictly serial traversal only for `--threads 1`; every `--threads > 1` run uses the persistent rayon pool regardless of particle count.

| Group | N | Threads | Median / force_ms |
| --- | ---: | ---: | ---: |
| bh_force criterion | 1,000 | 1 | 696.32 µs |
| bh_force criterion | 1,000 | 4 (pre-pf3 serialized) | 699.35 µs |
| bh_force criterion | 10,000 | 1 | 11.263 ms |
| bh_force criterion | 10,000 | 4 (pre-pf3 serialized) | 10.836 ms |

Criterion rows are from a single back-to-back run on an idle host; the
pre-pf3 low-N `threads=4` rows matched `--threads 1` within run-to-run noise.
| one-step run | 10,000 | 1 | 22.204 ms |
| one-step run | 10,000 | 4 (pre-pf3 serialized) | 22.399 ms |
| one-step run | 50,000 | 1 | 132.612 ms |
| one-step run | 50,000 | 4 (pre-pf3 scoped threads) | 121.301 ms |

## BH force persistent-pool scaling (nebula-quanta-pf3)

- Date: 2026-06-12
- Machine: AMD Ryzen 9 9900X 12-Core Processor; 18 CPUs visible (`lscpu`: 1 thread/core, 1 socket); idle host (1-min load < 2)
- Command: `cargo bench --bench nbody 2>&1 | tee .sc/nebula-quanta-pf3.bench-idle.log`
- Change note: `--threads 1` remains strictly serial; `--threads > 1` uses a cached rayon pool with work-stealing over safe `par_chunks_mut` disjoint output chunks (64 particles per chunk) and per-worker traversal stacks from `for_each_init`. The 50,000-particle single-thread fallback is removed.
- Result: 12-thread vs 1-thread speedups of 2.37x at N=1k, 7.88x at N=10k, and 11.08x at N=100k. An earlier measurement taken while large unrelated jobs loaded the host (1-min load ~27) suggested a ~4.7x ceiling; that was load contamination, not a property of the code — idle scaling is near-linear at N>=10k.
- Serial check: a same-conditions A/B at N=10k threads=1 measured 22.33 ms on the pre-change commit vs 21.28 ms with this change — no serial regression. These rows are not comparable to the pre-pf3 baseline rows above (different runs; see the cross-run comparability note).

| Group | N | Threads | Median |
| --- | ---: | ---: | ---: |
| bh_force criterion | 1,000 | 1 | 1.0376 ms |
| bh_force criterion | 1,000 | 12 | 438.60 µs |
| bh_force criterion | 10,000 | 1 | 21.284 ms |
| bh_force criterion | 10,000 | 12 | 2.7019 ms |
| bh_force criterion | 100,000 | 1 | 267.30 ms |
| bh_force criterion | 100,000 | 12 | 24.126 ms |

## Direct-force SIMD honesty decision (nebula-quanta-0nn)

- Date: 2026-06-11
- Command: `cargo bench --bench nbody -- direct`
- Direct-force decision: Path B. The real `wide` f64x4 kernel benchmarked with `wide = "0.7.33"` was not at least 1.3x faster than the best existing path, so the misleading `simd` feature was renamed to `unrolled` and documented as manual scalar unrolling rather than SIMD. The scalar path remains the default.
- Accuracy parity command: `cargo test --features unrolled --lib direct::tests -- --nocapture`
- Observed feature-vs-scalar force diff: Plummer N=4096 max abs `4.274625e-11`, max normalized `2.750360e-14`; clustered/disk N=4096 max abs `8.731149e-10`, max normalized `1.880421e-14`.
- The normalized metric divides each component diff by `max(1.0, |f_scalar|)` for that particle, making the 1e-12 bound scale with force magnitude.

| Variant | N | Median |
| --- | ---: | ---: |
| scalar | 4,096 | 26.299 ms |
| shipped manual unrolled | 4,096 | 23.596 ms |
| real SIMD f64x4 (`wide` 0.7.33 trial) | 4,096 | 19.341 ms (earlier decision run) |

The scalar and unrolled rows are from one back-to-back run; the `wide` trial
median comes from the earlier decision run (scalar 21.061 ms, unrolled
18.226 ms, f64x4 19.341 ms in that run) and is not directly comparable to the
rows above it. Within its own run the f64x4 trial was slower than the
unrolled kernel, which is what decided Path B.
