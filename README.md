# Nebula Quanta (`nebula-quanta`)

![CI](https://github.com/ricardofrantz/nebula-quanta/actions/workflows/ci.yml/badge.svg?branch=main)
[![Crates.io](https://img.shields.io/crates/v/nebula-quanta.svg)](https://crates.io/crates/nebula-quanta)

`nq` is a focused Barnes–Hut N-body simulation CLI in Rust.
It keeps core loops lean and memory bounded for large particle counts, with a built-in direct-force baseline for correctness checks.

## About

`nq` is the short command name for this project.

You get two layers:
- Rust engine (`nebula-quanta`) for compute and deterministic behavior.
- Bun launcher (`run.ts`) for scriptable local invocation and workflow glue.

## Topics

- N-body simulation
- Barnes-Hut quadtree
- Gravitational force solvers
- SoA particle layout
- Performance-first CLI tooling
- Deterministic benchmarks

## Why the name

- **Nebula** reflects dense particle clouds and large interacting fields.
- **Quanta** reflects individual bodies and lightweight computational units.
- **`nq`** gives a short, memorable command.

## Quick start

```bash
cargo install nebula-quanta
nq --mode=barnes_hut --n 10000 --steps 200 --dt 0.001 --theta 0.6 --epsilon 0.01
```

Local dev run:

```bash
bun run nq -- --mode=direct --n 1024 --steps 20 --validate --theta 0.7 --epsilon 0.01
```

## CLI controls

- `--mode barnes_hut|direct`
- `--n  <particle count>`
- `--steps <integration steps>`
- `--dt <time step>`
- `--theta <barnes-hut opening angle>`
- `--epsilon <softening>`
- `--seed <rng seed>`
- `--validate` (direct-force reference run)

## Example output

```text
mode=barnes_hut n=10000 steps=200 theta=0.6 epsilon=0.01 dt=0.001 build_ms=12.34 force_ms=58.91 integrate_ms=4.21 peak_nodes=3801 node_capacity=40001
```

## Core architecture

- Core simulator in Rust: `src/main.rs`, `src/config.rs`, `src/sim.rs`, `src/direct.rs`, `src/tree.rs`, `src/particle.rs`
- Metrics and diagnostics in `src/stats.rs`
- Bun launcher in `run.ts`

## Distribution

Crate path:

```bash
cargo install nebula-quanta
```

Bun path:

```bash
bun run nq -- --help
```

## Reproducibility

Simulations are deterministic by seed.
The same `--seed`, `--n`, `--steps`, and parameter set gives reproducible output.

## Validation

`--validate` runs a direct-force cross-check and prints:
- RMS/max position error
- RMS/max velocity error

Use this on smaller workloads to confirm Barnes-Hut accuracy in your parameter regime.

## References

- Barnes–Hut overview: https://en.wikipedia.org/wiki/Barnes%E2%80%93Hut_simulation
- Barnes & Hut original: Nature 324(4):446–449
- Rust implementation pattern reference: https://docs.rs/nbody_barnes_hut/latest/nbody_barnes_hut/

## Status

Planning and implementation are aligned in `SPEC.md` and `plan.md`.
