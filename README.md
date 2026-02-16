# Nebula Quanta (`nebula-quanta`)

Performance-focused Barnes–Hut gravity simulation built in Rust, shipped through a tiny CLI called `nq`.

`nq` is for fast force solves, large-N experiments, and deterministic physics baselines you can actually benchmark without hand-waving.

## About

`nq` is a deliberately compact simulation toolchain:

- **One command**: `nq` is the executable name.
- **One engine**: Rust core with predictable performance and memory behavior.
- **One launcher**: Bun bridge (`run.ts`) for script and dev workflow convenience.
- **One mission**: make large-N force calculations tractable while preserving validation hooks.

## Topics

- Barnes-Hut, N-body gravity
- Quadtree acceleration (`s/d < θ`)
- SoA layout for cache-friendly numerics
- Deterministic simulation loops
- Validation-by-direct-force cross-check
- Rust CLI performance tooling

## Why the name

- **Nebula**: suggests a dense particle field and gravitational structure, matching the visual and conceptual model of many interacting masses.
- **Quanta**: points to small discrete units (particles) and a focus on clear, lightweight computation units.
- **Combined**: the name is short, distinctive, and science-themed while still being understandable for a performance-focused physics engine.
- **Repo branding**: unique and easy to remember for CLI/docs/registry usage (`nebula-quanta`).

## Vision

Compute large gravitating systems fast, with a codebase that stays small enough to reason about:
- Build one quadtree per step.
- Collapse distant clusters using center-of-mass approximation.
- Control accuracy with `θ` and numerical stability with `ε`.
- Keep the implementation readable, then push the fast paths.

## Architecture

- **Core simulation engine:** Rust (deterministic, fast, memory-conscious).
- **Orchestration layer:** Bun (small CLI runner + benchmark/task scripting).
- **Algorithm:** 2D quadtree first (Barnes–Hut), with `N²` direct-force path for verification.
- **Primary control knobs:** `θ` (approximation angle), `ε` (softening), `dt` (time step).
- **Validation:** `--validate` runs a direct-mode reference and reports RMS/maximum position and velocity deltas.

## Current repository map

```text
.
├── README.md
├── Cargo.toml
├── SPEC.md
├── package.json
├── plan.md
├── plan_2026-02-16.md
├── run.ts
└── src/
    ├── main.rs       # CLI entrypoint
    ├── config.rs     # CLI flags and modes
    ├── particle.rs   # SoA state and initialization helpers
    ├── tree.rs       # Quadtree and aggregate mass/COM logic
    ├── sim.rs        # force evaluation + integration + diagnostics
    └── direct.rs     # O(N^2) fallback implementation
```

## Topics in motion

- **`core`**: deterministic Rust simulation and force kernels
- **`ops`**: startup-to-finish CLI and launcher path
- **`perf`**: memory-safe preallocated buffers + phase timing
- **`verify`**: optional direct-force validation channel

## Bootstrap

```bash
nq --mode=barnes_hut --n 10000 --steps 200 --dt 0.001 --theta 0.6 --epsilon 0.01
```

The repository now contains a working Rust core with a Bun launcher:

- Barns–Hut mode (`--mode=barnes_hut`) with quadtree-based force approximation.
- Direct mode (`--mode=direct`) for verification.

## Planned features

- Barnes–Hut force evaluation (`θ` controlled)
- Distance softening (`ε`) for stability
- Deterministic seeded initial conditions
- Direct-force fallback for correctness validation
- Step timing split reporting (`build`, `force`, `integrate`)
- Optional RMS/max state validation diagnostics
- Reproducible benchmark mode

## CLI plan (planned)

- `--mode` : `barnes_hut | direct`
- `--n` : number of particles
- `--steps` : simulation steps
- `--dt` : timestep
- `--theta` : opening-angle threshold
- `--epsilon` : softening length
- `--seed` : RNG seed for reproducibility
- `--validate` : run direct-mode cross-check (small workloads)

Example usage:

```bash
nq --mode=barnes_hut --n 10000 --steps 200 --dt 0.001 --theta 0.6 --epsilon 0.01
nq --mode=direct --validate --n 1024 --steps 20
```

## Example output

```text
mode=barnes_hut n=10000 steps=200 theta=0.6 epsilon=0.01 dt=0.001 build_ms=12.34 force_ms=58.91 integrate_ms=4.21 peak_nodes=3801 node_capacity=40001
```

## Distribution

- Rust (crates): `cargo install nebula-quanta` then `nq`
- NPM: `bun run nq` (local CLI wrapper) for rapid local scripting

## Development philosophy

- Keep the hot path small and explicit.
- No hidden magic numbers for algorithm controls.
- Measure before optimizing.
- Keep implementation and spec in lockstep.

## References

- Barnes–Hut simulation overview: https://en.wikipedia.org/wiki/Barnes%E2%80%93Hut_simulation
- Barnes & Hut original method (1986): Nature 324(4):446–449
- Rust implementation pattern reference: https://docs.rs/nbody_barnes_hut/latest/nbody_barnes_hut/

## Status

Planning documents are complete and ready to drive implementation.
