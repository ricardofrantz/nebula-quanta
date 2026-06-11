![Nebula Quanta banner](assets/readme-banner-v1.png)

[![Rust 2024](https://img.shields.io/badge/Rust-2024-blue.svg)](Cargo.toml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](Cargo.toml)

[![Watch the simulation](./nebula-quanta-barnes_hut.gif)](./nebula-quanta-barnes_hut.mp4)

[Download MP4](./nebula-quanta-barnes_hut.mp4)

The GIF plays the run forward then reversed so it loops seamlessly; the MP4 is the plain forward clip. Source reproduction command (seeded):

```bash
./run.sh --preset balanced --n 12000 --steps 600 --dt 0.001 --theta 0.6 --epsilon 0.008 --integrator leapfrog --init rotating-disk --init-radius 1.4 --init-v-amp 0.55 --init-lambda 0.45 --mass-profile lognormal --mass-mean 1.0 --mass-stddev 0.25 --mass-min 0.2 --mass-max 2.0 --seed 466369 --threads 1 --frames-dir captured_run_ker --width 1280 --height 720 --fps 30 --every-steps 1 --gif
```

`nq` is a focused Barnes–Hut N-body simulation CLI in Rust.
It is built for fast, memory-frugal runs with a deterministic direct-force baseline.
The short executable name is `nq` and the Rust crate is `nebula-quanta`.

## About

- Rust core: compute-heavy Barnes–Hut and direct-force solvers.
- Bun launch layer: `run.ts` is executed through `./run.sh` (performance-first defaults plus automatic MP4 export).
- Physics presets: gravity constant, initial-condition profiles, mass profiles, and integrator selection are built into the runtime CLI.
- Diagnostics: optional potential sampling, full energy snapshots, and momentum/angular momentum tracking.
- Softening controls: fixed and local-density adaptive profiles for clustered interactions.

## Topics

- N-body simulation
- Barnes-Hut quadtree
- Gravitational force solvers
- SoA particle layout
- Configurable profiles for positions, velocities, and masses
- Performance-first CLI tooling
- Preset launch profiles
- Deterministic benchmarks
- High-rate frame capture
- Offline MP4 rendering
- Memory telemetry
- Energy/momentum diagnostics

## Why the name

- **Nebula** reflects dense interacting particle fields.
- **Quanta** reflects each independent body as a compact unit of mass and state.
- **`nq`** is a short, stable executable name for scripts and batch runs.

## Quick start

```bash
cargo install --path .
nq --mode=barnes_hut --n 10000 --steps 200 --dt 0.001 --theta 0.6 --epsilon 0.01 --g 1.0 --init plummer --mass-profile pow-law
```

Use repository launcher defaults (frame capture + mp4 output on by default):

```bash
./run.sh
```

Generate an animated GIF preview instead of only MP4:

```bash
./run.sh --gif
```

Local development invocation (release optimized by default):

```bash
bun run nq -- --mode=direct --n 1024 --steps 20 --validate --theta 0.7 --epsilon 0.01
```

Use a debug build when iterating quickly:

```bash
bun run run:debug -- --mode=direct --n 1024 --steps 20 --validate --theta 0.7 --epsilon 0.01
```

Preset launcher shortcuts from `run.sh`:

```bash
./run.sh --preset fast --n 30000 --steps 180 --mass-profile uniform --mass-alpha 1.0
./run.sh --preset balanced --n 14000 --steps 280 --mass-profile pow-law --mass-alpha 2.4
./run.sh --preset accurate --n 6000 --steps 350 --mass-profile lognormal --mass-mean 1.0 --mass-alpha 2.2
```

Seeded profile example with fixed momentum and a higher-order integrator:

```bash
bun run nq \
  -- --mode=barnes_hut --n 60000 --steps 150 --dt 0.0007 --theta 0.6 --epsilon 0.01 \
  --init disk --init-radius 1.6 --init-v-amp 0.12 --init-lambda 0.8 \
  --mass-profile lognormal --mass-mean 1.0 --mass-stddev 0.35 --mass-min 0.1 --mass-max 3.0 \
  --integrator rk2 --seed 2026
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

`./run.sh` prints the render path automatically:

```text
Saving : nebula-quanta-barnes_hut.mp4
```

The printed `render_cmd` contains a working `ffmpeg` invocation; override/retune it as needed.

The optional direct-force acceleration feature is named `unrolled` because it is manual 4-lane scalar unrolling, not true SIMD; in the current N=4096 direct-force bench it ran 1.11x faster than scalar (23.596 ms vs 26.299 ms median). Enable it with:

```bash
cargo run --release --features unrolled -- --mode direct --n 4096 --steps 1 --theta 0
```

## Benchmark sweep

Use a shell loop to sweep `θ` and thread counts for speed/accuracy tradeoffs:

```bash
for theta in 0.3 0.5 0.7 1.0; do
  for th in 1 4; do
    ./run.sh --mode barnes_hut --n 20000 --steps 200 --dt 0.0008 --epsilon 0.01 \
      --theta "$theta" --threads "$th" --energy-sample-ratio 0.0 --seed 42
  done
done
```

The run prints `nq` logs and `--csv` rows when enabled, so you can compare `steps_per_sec` and `ns_per_particle_force` directly.

The log exposes parsed perf/physics fields, including timing, throughput, memory telemetry, energy, validation, and momentum diagnostics.
Current columns are:

- `mode,n,steps,dt,theta,theta_policy,theta_density_scale,softening_policy,softening_density_scale,epsilon,g,threads,integrator,init,mass_profile,init_radius,init_spread,init_v_amp,init_lambda,init_center_x,init_center_y,mass_mean,mass_stddev,mass_min,mass_max,mass_alpha,seed,build_ms,force_ms,integrate_ms,total_ms,avg_step_ms,steps_per_sec,ns_per_particle_force,peak_nodes,node_capacity,node_utilization,workspace_bytes,bytes_per_particle,particle_bytes,node_bytes,stack_bytes,initial_ke,initial_pe,initial_te,initial_sampled_pairs,final_ke,final_pe,final_te,final_sampled_pairs,energy_drift_abs,energy_drift_rel,validate_force_rms,validate_force_max,validate_energy_abs,validate_energy_rel,p0_x,p0_y,p0_mag,lz0,p1_x,p1_y,p1_mag,lz1,dp_x,dp_y,dp_mag,dp_lz

For deterministic replay, pass an explicit seed (for example, `--seed 42`).

## Ultra-long video-first workflow

For long runs where you want fixed frame-rate output budgets, capture every `N`th frame and post-encode:

```bash
bun run nq \
  -- --mode=barnes_hut --n 120000 --steps 20000 --dt 0.0005 --theta 0.7 --epsilon 0.01 \
  --record --frames-dir ./captured_run --width 1920 --height 1080 --fps 30 --every-steps 6 \
  --integrator rk2 --init disk --init-radius 1.6 --init-v-amp 0.08 --mass-profile pow-law --mass-alpha 2.5 --seed 2026
```

The run should produce around `ceil(steps / every_steps)` frames.
After capture, render with:

```bash
ffmpeg -y -framerate 30 -i captured_run/frame_%06d.ppm -c:v libx264 -pix_fmt yuv420p nebula-quanta-long.mp4
```

`--preset` presets are defaults in the launcher and can be overridden with direct flags:

```bash
./run.sh --preset fast --theta 0.35 --dt 0.0005 --threads 4
```

## CLI controls

- `--mode <barnes_hut|direct>` (default: `barnes_hut`; aliases: `bh`, `barneshut`)
- `--n <particle count>`
- `--steps <integration steps>`
- `--dt <time step>`
- `--theta <barnes-hut opening angle>`
- `--epsilon <softening>`
- `--g <gravity constant multiplier>` (default: `1`)
- `--integrator <leapfrog|verlet|rk2>` (default: `leapfrog`; `rk2` is explicit midpoint)
- `--init <uniform|gaussian|plummer|disk|rotating-disk|keplerian-disk>`
- `--init-radius <radius>`
- `--init-spread <spread>`
- `--init-v-amp <velocity amplitude>`
- `--init-lambda <shape parameter for plummer/disk>`
- `--init-center-x <x center>`
- `--init-center-y <y center>`
- `--mass-profile <uniform|lognormal|gaussian|pow-law>`
- `--mass-mean <mass mean>`
- `--mass-stddev <mass standard deviation>`
- `--mass-min <mass min clamp>`
- `--mass-max <mass max clamp>`
- `--mass-alpha <power-law exponent>`
- `--seed <rng seed>`
- `--preset <fast|balanced|accurate>` (`run.sh` launch profile defaults; explicit flags override this preset)
- `--dim <2|3>` (2D ready; 3D is scaffolded and currently guarded)
- `--theta-policy <fixed|local-density>` (default: `fixed`)
- `--theta-density-scale <scale>` (larger values reduce local-density adaptation strength)
- `--softening-policy <fixed|local-density>` (default: `fixed`)
- `--softening-density-scale <scale>` (larger values reduce softening growth in adaptive mode)
- `--validate` (run direct-force reference check; run `--mode=direct` for full O(n²) baseline behavior)
- `--energy-drift <auto|on|off>` (default: `auto`, computes energy drift for `N <= 8192` only)
- `--energy-sample-ratio <0..1>` (set >0 to sample potential energy on any size N)
- `--record` (enable frame export)
- `--gif` (generate animated GIF from rendered MP4)
- `--frames-dir <dir>` (default `frames`)
- `--width <pixels>`
- `--height <pixels>`
- `--fps <frames per second>`
- `--every-steps <n>` (record every nth step)
- `--threads <n>` (Barnes–Hut force threads; use 1 for deterministic single-thread baseline; runs below 50,000 particles intentionally use the single-thread force path, while runs at/above 50,000 particles use the scoped-thread Barnes–Hut path when `n > 1`)
- `--max-memory-mib <size>` (hard cap on estimated workspace bytes)
- `--csv <path>` (write benchmark summary rows to a CSV file; includes all parsed timing/metric columns)
- `--features unrolled` is a Cargo build feature (pass via `cargo run/build --features unrolled`) that enables an optional manual 4-lane unrolled direct-force path; it is not a SIMD implementation and measured 1.11x faster than scalar at N=4096 in the current direct-force bench.

## Performance profile

```text
mode=barnes_hut n=10000 steps=200 theta=0.6 theta_policy=fixed theta_density_scale=128.0 softening_policy=fixed softening_density_scale=128.0 epsilon=0.01 g=1 dt=0.001 integrator=leapfrog init=plummer mass_profile=pow-law threads=1 build_ms=12.34 force_ms=58.91 integrate_ms=4.21 total_ms=75.46 avg_step_ms=0.377 steps_per_sec=2654.7 ns_per_particle_force=294.5 peak_nodes=3801 node_capacity=40001 node_utilization=34.5 workspace_bytes=1234567 bytes_per_particle=56.0 particle_bytes=560000 node_bytes=123456 stack_bytes=16384 initial_ke=123.456 initial_pe=-678.901 initial_te=-555.445 initial_sampled_pairs=49500000 final_ke=123.489 final_pe=-678.872 final_te=-555.383 final_sampled_pairs=49500000 energy_drift_abs=0.062 energy_drift_rel=0.000112 validate_force_rms=0.000001 validate_force_max=0.000004 validate_energy_abs=0.0000001 validate_energy_rel=0.0000002 p0_x=1.2 p0_y=-0.4 p0_mag=1.27 lz0=2.9 p1_x=1.201 p1_y=-0.405 p1_mag=1.27 lz1=2.91 dp_x=0.001 dp_y=-0.005 dp_mag=0.005 dp_lz=0.01
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
- `energy_drift_abs` and `energy_drift_rel` measure global simulation drift.
- `validate_force_rms` and `validate_force_max` measure Barnes–Hut force mismatch against direct at final state when `--validate` runs.
- `validate_energy_abs` and `validate_energy_rel` measure energy mismatch between validated Barnes–Hut and direct final states.
- `initial_ke`, `initial_pe`, `initial_te`, `final_ke`, `final_pe`, `final_te`, and optional sampled pair counts.
- `na` for larger runs where exact energy scan would add excessive O(N²) overhead.
- Momentum and angular momentum (`p0_`, `p1_`, `dp_`) snapshots.

In `--energy-drift=auto` (default), energy drift is computed for modest workloads and skipped for larger ones. Use `--energy-drift=on` to force it, or `--energy-drift=off` to suppress it.

## Validation

`--validate` compares Barnes–Hut against direct-force on reduced workloads and prints RMS/max deltas in position and velocity.
It also computes force-field and final-state energy parity metrics (`validate_force_*`, `validate_energy_*`).

## References

- Barnes–Hut overview: https://en.wikipedia.org/wiki/Barnes%E2%80%93Hut_simulation
- Barnes & Hut original: Nature 324(4):446–449
- Rust reference: https://docs.rs/nbody_barnes_hut/latest/nbody_barnes_hut/
