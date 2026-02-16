# Barnes–Hut Simulation Implementation Plan

## Goal
Implement a max-performance, memory-frugal Barnes–Hut engine with a clear speed-vs-accuracy knob (`θ`) while keeping the codebase small and understandable.

## Performance and memory doctrine (big-run target)
- Priority 1 is throughput and memory bound correctness:
  - Keep the hot loop branch-light and cache-friendly.
  - Keep simulation memory strictly \(O(N)\) with a small fixed factor.
  - Delay all expensive allocations and object creation until startup.
- Target shape:
  - particles: pure SoA, contiguous buffers
  - nodes: fixed-width packed struct array
  - traversal: explicit stack/queue arrays, no recursive allocation
- Big-run guardrails:
  - Node index type is compact (`u32`/`i32`), not pointers.
  - No per-step `Vec` growth; all containers are cleared and reused.
  - No cloning of full per-particle vectors per timestep.

## Assumptions to keep and verify (no hidden defaults)
- Performance baseline: target Barnes–Hut behavior as \(O(n \log n)\) vs direct \(O(n^2)\) for large N, then measure crossover point locally.
- Approximation control: use only documented MAC criterion `s/d < θ` from Barnes–Hut references.
- Accuracy fallback: `θ = 0` is treated as direct-sum behavior and is required for verification, not for production use.
- Parameter defaults are not fixed assumptions:
  - `θ` values are initial settings only (benchmark will decide defaults per workload).
  - Softening `ε` starts as a configuration default, then tuned by test stability.
- Rust core as implementation source of truth:
  - use the 2D/3D module structure (`barnes_hut_2d`, `barnes_hut_3d`) as a reusable API pattern only.
  - the force-closure pattern expects non-normalized distance vectors from tree traversal.
- Runtime model: keep `Bun` as orchestrator only; numeric loops must stay in Rust.
- Memory model: fixed memory pools with explicit capacity planning for worst expected `N`.
- Numeric model: default to `f64` if accuracy is critical, allow optional `f32` mode for throughput experiments.

## Current status (2026-02-16)
- ✅ 2D Barnes–Hut engine and direct-force baseline are in place.
- ✅ Memory telemetry and bounded-node diagnostics are implemented.
- ✅ Optional frame capture path is implemented (`--record`, `--frames-dir`, `--width`, `--height`, `--fps`, `--every-steps`) with built `ffmpeg` command output.
- ✅ Bun launcher remains a thin orchestrator; Rust stays authoritative for numerics.

## Reference source for core algorithm/math
- Canonical paper: *A hierarchical O(N log N) force-calculation algorithm* (Barnes & Hut, 1986), DOI `10.1038/324446a0`.
- Core idea reference page: https://en.wikipedia.org/wiki/Barnes%E2%80%93Hut_simulation
- Rust starter reference: https://docs.rs/nbody_barnes_hut/latest/nbody_barnes_hut/ (`barnes_hut_2d`, `OctTree`)

### Core math to follow (assumption-checked)
- Pair force with softening:
  - `r = p_j - p_i`
  - `r2 = dot(r, r) + ε²`
  - `f_ij = G * m_i * m_j * r / (r2 * sqrt(r2))`
- Center of mass for node:
  - `M = Σ m_k`
  - `CM = (Σ m_k * x_k) / M`
- Barnes–Hut acceptance:
  - Let `s = node_size`, `d = ||CM - p_i||`
  - Approximate node if `s/d < θ`
  - If `θ = 0`, algorithm degenerates to direct O(n²).

### Minimal code template (reference shape)
- 2D quad node fields:
  - bounds (`xmin`, `xmax`, `ymin`, `ymax`)
  - `mass`, `com_x`, `com_y`
  - `children[4]` indexes (`i32` sentinel `-1` for empty)
  - `body_idx` (`i32`, `-1` none)
- Particle storage (SoA for speed):
  - `x[] y[] vx[] vy[] m[]`
- Main loop per step:
  - `build_tree` → `compute_node_mass_com` → `accumulate_forces` → `integrate`
- Memory-aware variant:
  - `Node` and particle buffers are `Vec<T>` with `capacity` pre-reserved to worst-case frame needs.
  - traversal stack is one reusable `Vec<usize>` that is reset, not recreated.
  - no `String` in hot path; IDs/labels only outside benchmark loops.

## Milestone 0 — Scope and baseline contract (0.5 day)
- Decide dimensionality (start with 2D).
- Fix force model:
  - \(F = G m_i m_j / (r^2 + \epsilon^2)\)
  - Optional support for 3D can be deferred.
- Choose integration method (default: leapfrog/velocity Verlet).
- Add big-run budgets and capacity policy:
  - memory budget for particles and nodes in README/spec.
  - choose fallback behavior if node budget would be exceeded.
- Define runtime parameters:
  - `θ` (opening angle), `softening ε`, `dt`, optional max depth / rebuild policy.
- Define acceptance checks:
  - correctness metric vs direct O(n²),
  - wall-clock target per step,
  - energy drift trend threshold over short horizon.

## Milestone 1 — Baseline and validation harness (0.5–1 day)
- Implement or retain an O(n²) direct-force path behind a flag.
- Split benchmarks into two classes:
  - **Fast path benchmarks:** large-N, Barnes–Hut only.
  - **Validation benchmarks:** moderate N with direct baseline.
- Add deterministic seed and benchmark seeds.
- Add diagnostics:
  - max/mean force error against direct method,
  - total energy drift,
  - step time split (`build`, `force`, `integrate`).
- Add memory diagnostics:
  - peak RSS approximation or process memory delta by phase,
  - node pool utilization percentage,
  - bytes per particle estimate.
- Freeze one or two canonical scenarios for regression.

## Milestone 2 — Simple Barnes–Hut data model (1 day)
- Implement SoA particle storage:
  - `x[]`, `y[]`, `vx[]`, `vy[]`, `m[]`.
- Build fixed-size node pool reused every frame:
  - region bounds, child indices (4 for quad),
  - aggregate mass and COM,
  - leaf body index or empty flag.
- Add frame-local memory reuse for traversal stacks and temporary arrays.
- Force one representation for all steps:
  - `x`, `y`, `vx`, `vy`, `m` as aligned contiguous slices.
  - nodes as one packed struct with fixed-order fields.
- Add upper-bound sizing:
  - start with `4 * N + 1` node slots for worst practical tree growth.
  - no dynamic per-insertion allocation inside loop.

## Milestone 3 — Tree construction and force traversal (1–2 days)
- Per step:
  1. compute bounds,
  2. build quadtree (iterative or recursive insert),
  3. compute node mass + COM bottom-up.
- Force loop:
  - traverse from each particle,
  - if node satisfies `s/d < θ`, use aggregated node force,
  - else recurse children.
- Include softening in denominator and minimum-distance guard.
- Add explicit traversal stack with scratch buffers:
  - pop/push child nodes in a fixed array of indices.
  - never allocate per particle.
- Separate force pass into two stages to reduce cache misses:
  - read-only tree walk,
  - single write-back to `ax[]`, `ay[]`.
- Add optional Barnes–Hut early-exit fast path:
  - skip empty nodes quickly with sentinel checks.

## Milestone 4 — Performance pass (1 day)
- Remove avoidable allocations (preallocate once, clear/reuse).
- Replace recursion in all hot loops with iterative stack-based traversal.
- Co-locate frequently accessed arrays for cache friendliness.
- Add branch pruning and branch-order optimization:
  - compute `s/d` and distance once per node revisit.
  - cheap reject checks before expensive force math.
- Reduce memory traffic:
  - one force accumulator arrays `ax`, `ay` reused and zeroed with `fill`.
  - avoid temporary pairwise vectors for inner loops.
- Tune by measurement:
  - run short sweeps on candidate `θ` values (for example `0.3, 0.5, 0.7, 1.0`),
  - choose the smallest `θ` meeting speed/accuracy targets.
- Introduce optional parallelism in force accumulation (`rayon`) only after single-thread tuning is complete.
- Add capture I/O budgeting separately from physics timing:
  - measure frame write throughput and filesystem bottlenecks with fixed cadence.
  - confirm simulation timing is unchanged when recording is off.

## Milestone 5 — Verification and tuning (0.5–1 day)
- Run convergence checks over fixed datasets:
  - direct vs Barnes–Hut force vector error,
  - short-horizon position/energy divergence.
- Sweep `(θ, ε, dt)` and record Pareto points for speed/accuracy.
- Document recommended presets (fast / balanced / accurate).
- Add stress tests for large-`N` node capacity and deterministic hard-fail diagnostics.

## Milestone 5a — Video throughput hardening
- Add a short render helper command section in `README.md` for high-FPS exports.
- Include 60/120 fps and 4K profiles with explicit `ffmpeg` presets and codec options.
- Keep frame export outside benchmark hot path; support recording at `N=0` overhead when disabled.

## Milestone 6 — Optional next phase
- 3D octree variant.
- Thread-level parallel force evaluation.
- Optional adaptive `θ` by local density.
- Optional higher-order integrator option.
- SoA-only fixed-size SIMD-friendly layout.
- Persistent paging for huge runs (`N` in the high six/low seven digits) with chunked output if memory cap is exceeded.

## Deliverable checklist
- [ ] Reproducible CLI or script to run deterministic benchmark set.
- [ ] Barnes–Hut path is default for large N.
- [ ] O(n²) baseline retained for debug/verification.
- [x] Metrics logged each run (`N`, step ms, memory bytes, error, energy drift).
- [x] Memory pool usage stays bounded and does not grow after first allocation.
- [x] Optional deterministic frame capture path emits numbered PPM frames and ready-to-run render command.
- [x] Rendering helper for high-rate offline encode (`scripts/render_video.sh`) is documented in README.
