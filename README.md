# Nebula Quanta (`nebula-quanta`)

A fast, simple Barnes–Hut N-body simulation project in Rust with Bun orchestration.

## Why the name

- **Nebula**: suggests a dense particle field and gravitational structure, matching the visual and conceptual model of many interacting masses.
- **Quanta**: points to small discrete units (particles) and a focus on clear, lightweight computation units.
- **Combined**: the name is short, distinctive, and science-themed while still being understandable for a performance-focused physics engine.
- **Repo branding**: unique and easy to remember for CLI/docs/registry usage (`nebula-quanta`).

## Vision

Compute gravitational interactions efficiently with a minimal, readable implementation of the Barnes–Hut algorithm:
- Build a spatial tree.
- Replace far clusters by center-of-mass approximations.
- Tune accuracy by `θ`.
- Keep the implementation simple first, then optimize with measured passes.

## Architecture

- **Core simulation engine:** Rust (deterministic, fast, memory-conscious).
- **Orchestration layer:** Bun (small CLI runner + benchmark/task scripting).
- **Algorithm:** 2D quadtree first (Barnes–Hut), with `N²` direct-force path for verification.
- **Primary control knobs:** `θ` (approximation angle), `ε` (softening), `dt` (time step).

## Current repository map

```text
.
├── README.md
├── SPEC.md
├── plan.md
├── plan_2026-02-16.md
└── src/
    ├── main.rs       # CLI + config + run loop entry
    ├── particle.rs   # SoA state and initialization helpers
    ├── tree.rs       # Quadtree and aggregate mass/COM logic
    ├── sim.rs        # force evaluation + integration + diagnostics
    └── direct.rs     # O(N^2) fallback implementation
```

## Planned features

- Barnes–Hut force evaluation (`θ` controlled)
- Distance softening (`ε`) for stability
- Deterministic seeded initial conditions
- Direct-force fallback for correctness validation
- Step timing split reporting (`build`, `force`, `integrate`)
- Error and energy diagnostics
- Reproducible benchmark mode

## CLI plan (planned)

- `--mode` : `barnes_hut | direct`
- `--n` : number of particles
- `--steps` : simulation steps
- `--dt` : timestep
- `--theta` : opening-angle threshold
- `--epsilon` : softening length
- `--seed` : RNG seed for reproducibility
- `--report` : logging cadence

Example usage (after implementation):

```bash
bun run run.ts -- --mode=barnes_hut --n 10000 --steps 200 --dt 0.001 --theta 0.6 --epsilon 0.01
bun run run.ts -- --mode=direct --validate --n 1024 --steps 20
```

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
