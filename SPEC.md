# SPEC: Nebula Quanta

## 1) Purpose

Build a simple, fast Barnes–Hut simulation engine centered on 2D gravity with a small, predictable code surface and a Bun-based orchestration layer.

## 2) Goals

- Provide Barnes–Hut performance improvements over direct gravity force calculation.
- Keep algorithm behavior transparent and easy to reason about.
- Make correctness measurable by always retaining a direct-force verification path.
- Record deterministic benchmark runs for speed/accuracy tradeoffs.

## 3) Definitions

- `N`: number of particles
- `θ` (theta): Barnes–Hut opening angle threshold
- `ε` (epsilon): softening length
- `dt`: time step
- SoA: Structure of Arrays
- COM: center of mass

## 4) In-scope / out-of-scope

### In-scope

- 2D implementation with a quadtree.
- Barnes–Hut force path with node acceptance `s/d < θ`.
- Direct `O(N^2)` mode for validation and testing.
- CLI-run simulation and benchmark modes.
- Deterministic initial conditions and seeded random generation.
- Timing, energy, and error diagnostics.

### Out-of-scope (initial release)

- Real-time renderer.
- GPU-accelerated simulation path.
- 3D octree engine.
- Distributed/incremental update architecture.

## 5) Functional requirements

1. The system shall generate particle initial states from command-line input or config.
2. The system shall compute bounds for each timestep and rebuild a quadtree.
3. Each node shall store aggregate mass and COM.
4. For each particle, the system shall traverse the quadtree and apply either:
   - aggregated node contribution if `s/d < θ`
   - recursion to children otherwise.
5. Force law for pair evaluation shall be:
   `f_ij = G m_i m_j (r_j - r_i) / (|r_j - r_i|^2 + ε²)^(3/2)`
6. The system shall provide a direct-force reference mode using the same particle integrator.
7. The system shall integrate particle states with leapfrog/velocity Verlet as the default.
8. The system shall support CLI flags:
   - `--mode`, `--n`, `--steps`, `--dt`, `--theta`, `--epsilon`, `--seed`, `--output`.
9. The system shall emit per-run output including time splits and validation metrics.

## 6) Non-functional requirements

- Performance and memory usage shall be controlled by preallocated arrays and reusable buffers.
- Numeric behavior and simulation results shall be deterministic for a fixed seed.
- No silent fallback when critical simulation invariants are violated; return explicit errors.
- The implementation shall remain readable and minimally abstract for phase one.

## 7) Validation strategy

### Correctness

- Compare Barnes–Hut output to direct-force mode on fixed scenarios.
- Report force vector error statistics (max and mean).
- Report drift in total mechanical energy over short windows.

### Performance

- Report step timings by phase:
  - bounds/build
  - force solve
  - integration
- Record crossover behavior where Barnes–Hut overtakes direct-force mode as `N` grows.

### Reproducibility

- Same seed + same command must reproduce identical outputs.
- Logs must include parameters used for each run.

## 8) Data model

### Particle state

- Arrays: `x[]`, `y[]`, `vx[]`, `vy[]`, `m[]`.

### Quadtree node

- Spatial bounds.
- `mass`, `com_x`, `com_y`.
- `body_index` (leaf or internal marker).
- `child[4]` indices for NW, NE, SW, SE.

## 9) Acceptance criteria

- Direct mode is implemented and executable.
- Barnes–Hut mode completes runs for target `N` with stable output.
- Verified logs include requested metrics.
- Measured speedup (or non-regression at high `N`) and bounded error are demonstrated.

## 10) Risks and mitigations

- Over-clustered states causing deep recursion: mitigate with robust split rules and optional iterative traversal.
- Near-zero distances: mitigate with softening and minimum-distance guard.
- Over-aggressive `θ`: mitigate via benchmark-driven tuning and reporting defaults as provisional.

## 11) Future scope

- 3D octree extension.
- Thread-level parallel force accumulation.
- Optional adaptive `θ` scheduling.

