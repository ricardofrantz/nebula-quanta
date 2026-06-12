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

## Parallel tree-build trial (nebula-quanta-p43)

- Date: 2026-06-12
- Machine: AMD Ryzen 9 9900X 12-Core Processor; 18 CPUs visible (`lscpu`: 1 thread/core, 1 socket)
- Rev 1 load at bench start: `/proc/loadavg` = `1.95 2.90 2.44 1/1397 2988249`
- Rev 1 command: `cargo bench --bench nbody 2>&1 | tee .sc/nebula-quanta-p43.bench.log`
- Rev 1 result: the prefix-partitioned parallel builder preserved bitwise force parity but did **not** meet the target speedup: N=100k tree-build 12-thread median 13.724 ms vs serial 34.514 ms (2.51x, target >=4x).
- Rev 2 load at bench start: `/proc/loadavg` = `1.27 1.95 2.12 3/1393 3092205`
- Rev 2 command: `cargo bench --bench nbody 2>&1 | tee .sc/nebula-quanta-p43.bench2.log`
- Rev 2 result: deterministic prefix depth now scales to depth 3 for 12 threads, but only the root quadrants were scheduled across rayon; each quadrant filled its leaf buckets serially. Threaded tree-build transient subtree/index allocations are charged in memory preflight. However the measured N=100k tree-build speedup is still only 3.31x (8.5393 ms vs 28.218 ms), below the >=4x target.
- Rev 2 one-off instrumented phase check for N=100k/thread=12 (`.sc/nebula-quanta-p43.profile2.log`): second build `bounds_ms=0.119`, `prefix_ms=4.470`, `parallel_fill_ms=5.662`, `merge_ms=3.156`, `total_ms=13.711`.
- Rev 3 load at bench start: `/proc/loadavg` = `1.13 1.35 0.83 2/1409 3365250`
- Rev 3 command: `cargo bench --bench nbody 2>&1 | tee .sc/nebula-quanta-p43.bench3.log`
- Rev 3 result: the prefix skeleton is built serially to depth 3, then all non-singleton leaf buckets from that prefix are scheduled in one `into_par_iter()` and merged deterministically by `node_idx`. The measured N=100k tree-build speedup was 2.02x (7.2808 ms vs 14.671 ms), below the >=4x target.
- Rev 3 one-off instrumented phase check for N=100k/thread=12: second build `leaf_buckets=60`, `bounds_ms=0.056`, `prefix_ms=1.761`, `parallel_fill_ms=2.633`, `merge_ms=1.983`, `total_ms=6.435`.

| Revision | Group | N | Threads | Median | Loadavg |
| --- | --- | ---: | ---: | ---: | --- |
| rev 1 | tree build | 100,000 | 1 | 34.514 ms | `1.95 2.90 2.44` |
| rev 1 | tree build | 100,000 | 12 | 13.724 ms | `1.95 2.90 2.44` |
| rev 1 | tree build | 10,000 | 1 | 2.0062 ms | `1.95 2.90 2.44` |
| rev 1 | tree build | 10,000 | 12 | 1.3897 ms | `1.95 2.90 2.44` |
| rev 1 | tree build | 1,000 | 1 | 159.91 µs | `1.95 2.90 2.44` |
| rev 1 | tree build | 1,000 | 12 | 251.53 µs | `1.95 2.90 2.44` |
| rev 2 | tree build | 100,000 | 1 | 28.218 ms | `1.27 1.95 2.12` |
| rev 2 | tree build | 100,000 | 12 | 8.5393 ms | `1.27 1.95 2.12` |
| rev 2 | tree build | 10,000 | 1 | 1.9230 ms | `1.27 1.95 2.12` |
| rev 2 | tree build | 10,000 | 12 | 796.07 µs | `1.27 1.95 2.12` |
| rev 2 | tree build | 1,000 | 1 | 119.02 µs | `1.27 1.95 2.12` |
| rev 2 | tree build | 1,000 | 12 | 247.10 µs | `1.27 1.95 2.12` |
| rev 3 | tree build | 100,000 | 1 | 14.671 ms | `1.13 1.35 0.83` |
| rev 3 | tree build | 100,000 | 12 | 7.2808 ms | `1.13 1.35 0.83` |
| rev 3 | tree build | 10,000 | 1 | 890.06 µs | `1.13 1.35 0.83` |
| rev 3 | tree build | 10,000 | 12 | 716.10 µs | `1.13 1.35 0.83` |
| rev 3 | tree build | 1,000 | 1 | 64.602 µs | `1.13 1.35 0.83` |
| rev 3 | tree build | 1,000 | 12 | 198.97 µs | `1.13 1.35 0.83` |

- Supervisor verdict (final, after rev 3): accepted as the best honest state. The literal >=4x-vs-same-code-serial criterion reads 2.02x, but only because the serial build itself got 2.4x faster during the work (34.5 ms -> 14.7 ms at N=100k); in absolute terms the 12-thread build at 7.28 ms beats the original target's implied bound (34.5 ms / 4 = 8.6 ms), and against the bead-start serial baseline the threaded build is 4.7x. Supervisor re-bench (loadavg `0.59`) reproduced 15.6 ms / 7.04 ms. Physics: serial and 12-thread runs are bitwise-identical to the pre-change commit over a 20-step N=10k A/B. Known trade-off: threaded build is slower than serial build below ~10k particles (N=1k: 199 µs vs 65 µs); a measured small-N build crossover is filed as follow-up.

- Rev 2 supervisor verification (loadavg `0.60`): 28.362 ms / 8.587 ms = 3.30x reproduced; serial and 12-thread runs produce bitwise-identical physics to the pre-change commit over a 20-step N=10k A/B (positions and momenta compared at full precision).
- Rev 3 verdict: leaf-bucket scheduling is in place and parity remains green, but the original >=4x target is still not met on this run. Known trade-off remains: at N=1k the threaded build is slower than serial build, though force-eval gains can keep `--threads 12` a net win.

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
