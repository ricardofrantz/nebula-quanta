# Nebula Quanta (`nebula-quanta`)

![CI](https://github.com/ricardofrantz/nebula-quanta/actions/workflows/ci.yml/badge.svg?branch=main)
[![Crates.io](https://img.shields.io/crates/v/nebula-quanta.svg)](https://crates.io/crates/nebula-quanta)

`nq` is a focused Barnes–Hut N-body simulation CLI in Rust.
It is engineered for fast, memory-frugal runs with a deterministic direct-force baseline.
The short executable name is `nq` and the Rust crate is `nebula-quanta`.

## About

- Rust core: compute-heavy Barnes–Hut and direct-force solvers.
- Bun launch layer: simple scriptable local runner in `run.ts`.

## Topics

- N-body simulation
- Barnes-Hut quadtree
- Gravitational force solvers
- SoA particle layout
- Performance-first CLI tooling
- Deterministic benchmarks
- High-rate frame capture
- Offline MP4 rendering
- Memory telemetry

## Why the name

- **Nebula** reflects dense interacting particle fields.
- **Quanta** reflects each independent body as a compact unit of mass and state.
- **`nq`** is a short, stable executable name for scripts and batch runs.

## Quick start

```bash
cargo install --path .
nq --mode=barnes_hut --n 10000 --steps 200 --dt 0.001 --theta 0.6 --epsilon 0.01
```

Local development invocation (release optimized by default):

```bash
bun run nq -- --mode=direct --n 1024 --steps 20 --validate --theta 0.7 --epsilon 0.01
```

Use a debug build when iterating quickly:

```bash
bun run run:debug -- --mode=direct --n 1024 --steps 20 --validate --theta 0.7 --epsilon 0.01
```

## High-definition + high-FPS workflow

Use `--record` to export raw PPM frames and then encode with `ffmpeg`:

```bash
bun run nq \
  -- --mode=barnes_hut --n 20000 --steps 400 --dt 0.0008 --theta 0.7 \
  --record --frames-dir ./capture --width 1920 --height 1080 --fps 60 --every-steps 1
```

The run prints `record_frames` and a ready-to-run `render_cmd`, for example:

```text
record_frames=401 render_cmd="ffmpeg -y -framerate 60 -i capture/frame_%06d.ppm -s 1920x1080 -c:v libx264 -pix_fmt yuv420p nebula-quanta-barnes_hut.mp4"
```

For very long runs, reduce I/O using `--every-steps K` and keep K tuned to your target duration.

```bash
ffmpeg -y -framerate 60 -i capture/frame_%06d.ppm -c:v libx264 -pix_fmt yuv420p nebula-quanta-barnes_hut.mp4
```

You can also use the repo helper:

```bash
scripts/render_video.sh capture nebula-quanta-barnes_hut.mp4 60 20 fast libx264
```

```bash
bun run render capture nebula-quanta-barnes_hut.mp4 60 20 fast libx264
```

## Benchmark sweep

Use the repository helper to sweep `θ` and thread counts for speed/accuracy tradeoffs:

```bash
scripts/bench_sweep.sh --n 20000 --steps 200 --dt 0.0008 --epsilon 0.01 --theta 0.3,0.5,0.7,1.0 --threads 1,4
```

The helper prints `nq` logs for each run so you can compare `steps_per_sec` and `ns_per_particle_force` directly.

You can also emit structured results as CSV for downstream analysis:

```bash
scripts/bench_sweep.sh \
  --n 20000 --steps 200 --dt 0.0008 --epsilon 0.01 \
  --theta 0.3,0.5,0.7,1.0 --threads 1,4 --csv bench_results.csv
```

The CSV includes all parsed fields from the benchmark profile line, including timing, throughput, and memory telemetry.

For deterministic repeatability in CI and local handoffs, use the deterministic wrapper script (seed defaults to `42` unless overridden):

```bash
scripts/bench_sweep_deterministic.sh --n 20000 --steps 200 --dt 0.0008 --epsilon 0.01 --seed 42 --theta 0.3,0.5,0.7,1.0 --threads 1,4 --mode barnes_hut --csv bench_results.csv
```

## CLI controls

- `--mode <barnes_hut|direct>` (default: `barnes_hut`; aliases: `bh`, `barneshut`)
- `--n <particle count>`
- `--steps <integration steps>`
- `--dt <time step>`
- `--theta <barnes-hut opening angle>`
- `--epsilon <softening>`
- `--seed <rng seed>`
- `--validate` (run direct-force reference check; run `--mode=direct` for full O(n²) baseline behavior)
- `--energy-drift <auto|on|off>` (default: `auto`, computes energy drift for `N <= 8192` only)
- `--record` (enable frame export)
- `--frames-dir <dir>` (default `frames`)
- `--width <pixels>`
- `--height <pixels>`
- `--fps <frames per second>`
- `--every-steps <n>` (record every nth step)
- `--threads <n>` (Barnes–Hut force threads; use 1 for deterministic single-thread baseline)
- `--max-memory-mib <size>` (hard cap on estimated workspace bytes)
- `--csv <path>` (write benchmark summary rows to a CSV file; includes all parsed timing/metric columns)

## Performance profile

```text
mode=barnes_hut n=10000 steps=200 theta=0.6 epsilon=0.01 dt=0.001 threads=1 build_ms=12.34 force_ms=58.91 integrate_ms=4.21 total_ms=75.46 avg_step_ms=0.377 steps_per_sec=2654.7 ns_per_particle_force=294.5 peak_nodes=3801 node_capacity=40001 node_utilization=34.5 workspace_bytes=1234567 bytes_per_particle=56.0 particle_bytes=560000 node_bytes=123456 stack_bytes=16384 energy_drift_abs=1.234567 energy_drift_rel=0.000012345
```

## Core architecture

- Rust simulator: `src/main.rs`, `src/config.rs`, `src/sim.rs`, `src/direct.rs`, `src/tree.rs`, `src/particle.rs`
- Telemetry: `src/stats.rs`
- Frame output: `src/frame.rs`
- Bun launcher: `run.ts`

## Reproducibility

Simulations are deterministic by seed.
Given the same `--seed`, `--n`, `--steps`, and runtime flags, output is repeatable.
For runs with modest system size (`N <= 8192`), each summary includes:
- `energy_drift_abs` and `energy_drift_rel`.
- `na` for larger runs where exact energy scan would add excessive O(N²) overhead.

In `--energy-drift=auto` (default), energy drift is computed for modest workloads and skipped for larger ones. Use `--energy-drift=on` to force it, or `--energy-drift=off` to suppress it.

## Validation

`--validate` compares Barnes–Hut against direct-force on reduced workloads and prints RMS/max deltas in position and velocity.

## References

- Barnes–Hut overview: https://en.wikipedia.org/wiki/Barnes%E2%80%93Hut_simulation
- Barnes & Hut original: Nature 324(4):446–449
- Rust reference: https://docs.rs/nbody_barnes_hut/latest/nbody_barnes_hut/
