# SPEC: Nebula Quanta

## 1) Purpose

Implement a high-performance, memory-efficient Barnes–Hut simulation engine centered on 2D gravity with a small code surface and a Bun orchestration layer.
Optional deterministic frame capture is supported for high-definition offline rendering of long runs without changing simulation math.

## 2) Scale target

- Primary objective: make `N` scale large enough that direct-force methods are no longer practical.
- Secondary objective: keep memory growth bounded and predictable after initialization.
- Hard constraints:
  - all core buffers preallocated once;
  - no unbounded growth in hot loops;
  - explicit failure when resource bounds are exceeded.

## 3) Goals

- Deliver Barnes–Hut speedups with measured speed/accuracy knobs.
- Keep behavior deterministic and auditable with a reference direct mode.
- Keep memory traffic low through SoA layout and fixed-size, packed node storage.
- Make per-run metrics first-class: time, structural memory occupancy, and optional validation deltas.
- Include optional deterministic PPM frame capture for reproducible post-run video encoding.

## 4) Definitions

- `N`: number of particles
- `θ` (theta): Barnes–Hut opening angle threshold
- `ε` (epsilon): softening length
- `dt`: time step
- SoA: Structure of Arrays
- COM: center of mass
- bytes_per_particle: memory used per particle at a fixed step
- node pool: preallocated contiguous array of quadtree nodes

## 5) In-scope / out-of-scope

### In-scope

- 2D Barnes–Hut implementation with quadtree.
- `s/d < θ` node acceptance and aggregated-node force model.
- Direct `O(N²)` mode for verification.
- Deterministic CLI-driven benchmark and simulation execution.
- Seeded initial conditions.
- Metrics: speed, force error, energy drift, and memory usage.
- Memory policy: no per-step core allocations.
- Optional `PPM` frame export pipeline (`frame_%06d.ppm`) with bounded incremental memory.

### Out-of-scope (initial release)

- Interactive real-time rendering pipeline.
- GPU compute implementation.
- 3D octree implementation.
- Distributed execution.

## 6) Functional requirements

1. The system shall generate particle initial states from CLI/config and a seed.
2. The system shall rebuild a bounded quadtree each step from bounds and particle positions.
3. Each node shall store aggregate `mass` and COM.
4. Force traversal per particle shall do one of:
   - use node approximation when `s/d < θ`,
   - else continue with child traversal.
5. Pair force model shall use:
   `f_ij = G m_i m_j (r_j - r_i) / (|r_j - r_i|^2 + ε²)^(3/2)`
6. The system shall provide a direct-force reference implementation using the same integrator.
7. The system shall integrate state with leapfrog/velocity Verlet (default).
8. The system shall accept CLI flags including:
    - `--mode`, `--n`, `--steps`, `--dt`, `--theta`, `--epsilon`, `--seed`, `--validate`, `--record`, `--frames-dir`, `--width`, `--height`, `--fps`, `--every-steps`.
9. The system shall emit per-run outputs:
   - phase timings (`build`, `force`, `integrate`),
   - optional validation metrics (RMS/max position and velocity deltas),
   - memory usage and occupancy.
10. The system shall allocate all major core buffers during initialization and reuse them.
11. The system shall cap node pool capacity and return a deterministic error if a step exceeds capacity.
12. The system shall support reproducible replay of scenarios from logged configuration.
13. The system shall support bounded per-step output cadence controls via `--every-steps`.
14. The optional frame recorder shall avoid allocations in hot force/build/integrate loops and use a fixed-size RGB buffer.

## 7) Performance and memory requirements

- Memory complexity target: `O(N)` with bounded constant factor.
- Avoid temporary allocations in hot loops, including force loop and integration loop.
- Particle data must be SoA arrays (`x`, `y`, `vx`, `vy`, `m`) and aligned for cache locality.
- Quadtree nodes must be a flat array with packed fields, not boxed pointer graphs.
- Child links must be compact indices, not heap pointers.
- Traversal and build should use reusable index stacks.
- Failure policy: if capacity is exceeded, fail fast with explicit diagnostics (N, node count, cause).
- Recording path must not mutate simulation state or force path timing behavior.
- Frame encoding must remain a post-run concern (`ffmpeg` external to simulation).

## 8) Validation strategy

### Correctness

- Compare Barnes–Hut against direct mode on fixed scenarios.
- Report max and mean position/velocity delta metrics for final state.
- Track total mechanical energy drift as a follow-up phase.

### Performance

- Measure and log phase timings:
  - bounds/build
  - force solve
  - integration
- Report memory metrics:
  - peak node pool utilization and slack.
- Record crossover sweep where Barnes–Hut overtakes direct across multiple `N`.

### Reproducibility

- Same seed + same configuration yields identical output.
- Logs include full run parameters and capability flags.
- Logs include memory-usage trend and buffer-capacity decisions.

## 9) Data model

### Particle state

- Arrays: `x[]`, `y[]`, `vx[]`, `vy[]`, `m[]`.
- Reused force accumulators: `ax[]`, `ay[]`.
- Optional `fx_cache[]`/`fy_cache[]` only if needed and reusable.

### Quadtree node

- Fields:
  - `x_min`, `x_max`, `y_min`, `y_max`
  - `mass`, `com_x`, `com_y`
- `children[4]` indices
- `body_index` / `leaf` marker
- Node storage is fixed-capacity, contiguous, reusable.

### Frame export (optional)

- State is only enabled when `--record` is set.
- Reusable state:
  - output directory
  - width/height
  - byte buffer sized `width*height*3` and reused per frame
  - frame counter and naming index
- Output format is binary PPM (`P6`) with filenames `frame_%06d.ppm`.

## 10) Acceptance criteria

- Direct mode executes and can be used as verification baseline.
- Barnes–Hut runs complete for target `N` with stable deterministic output.
- Logs include timing and memory metrics for every run, with optional error metrics when validation is enabled.
- No per-step allocations in release-critical loops.
- Node-pool and particle buffers do not grow after initialization.
- Frame capture must avoid per-step hot-loop allocations and writes through preallocated buffers.

## 11) Risks and mitigations

- Over-clustered particle sets can increase tree depth: mitigate with robust split policy and iterative traversal.
- Near-zero pair distances: mitigate with softening and minimum-distance guard.
- Memory cap overrun from unexpected growth: mitigate by preflight `N` checks and explicit hard-stop with actionable diagnostics.
- Recursion overhead and stack risk: mitigate by iterative traversal in phase one.

## 12) Future scope

- SIMD-friendly kernels for force math.
- Parallel force accumulation with read-only tree sharing.
- 3D octree and optional adaptive `θ` schedule.
