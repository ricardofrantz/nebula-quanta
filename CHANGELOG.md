# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Quantity-colored particle recordings: `--color-by` now supports `speed`,
  `accel`, `density`, and `mass`, with selectable colormaps and explicit or
  first-frame color normalization controls for MP4 renders.
- README Gallery: two million-body showcase clips with full parameter receipts and exact seeded commands — a 1M-body disk-instability render (the cold disk fragmenting into clumps, 3,000 steps) and a 1M-body two-galaxy merger through its first passage and tidal-stream phase (6,000 steps, 35 min on 12 threads). Every clip regenerates bit-reproducibly from its published command.
- `--init merger`: two-galaxy collision initial condition composing two `galaxy-disk` realizations in the center-of-mass frame — `--merger-mass-ratio` (secondary scaled by `sqrt(q)` in size, masses rescaled exactly to `q`), `--merger-separation`, `--merger-impact-parameter`, `--merger-v-rel` (default near-parabolic `sqrt(2GM/d)`), and `--merger-spin prograde|retrograde`. Bulk momenta cancel to f64 roundoff; each sub-galaxy passes the same rotation-curve checks as a standalone disk.
- `--init galaxy-disk`: exponential surface-density disk with a central point mass (`--disk-central-mass-frac`), circular speeds computed from the actual enclosed discrete mass `v_c(r)=sqrt(G*M(<r)/r)`, and a Gaussian velocity-dispersion knob (`--disk-dispersion`, a simple fraction of local `v_c` — not a Toomre-Q analysis). `--disk-scale-length` sets the exponential scale length; rotation-curve and bitwise-determinism tests included.
- `--record --output <file.mp4>` streams raw RGB frames straight into an ffmpeg child process — no PPM intermediates on disk (a long high-resolution render previously needed tens of GB of temporary frames). The `--frames-dir` PPM path remains as a fallback; the two flags are mutually exclusive. Missing ffmpeg or a failed encode surface as clean errors.
- Criterion benchmark suite (`cargo bench`) for tree build, Barnes-Hut force, and direct force, with a recorded baseline in `BENCHMARKS.md`; the crate now also exposes a library target.
- Two-body Kepler analytical oracle tests: leapfrog verified against closed-form circular orbits (position and energy-conservation bounds).
- `--view-radius <r>` flag: fixed square camera window centered on the origin (0 keeps the per-frame auto-fit), so recordings can hold a stable view while ejecta leave the frame.
- README opens with a didactic explainer: what an N-body simulation is, the cold-collapse physics behind the hero clip (with its full parameter table), and how the Barnes-Hut quadtree plus a zero-allocation hot loop make it fast.

### Changed
- Tree builds below a measured 16,000-particle crossover now route to the serial builder even with `--threads > 1` — the threaded build's partition overhead dominates below that point (up to 6x slower at N=1k). The crossover sweep and threshold rationale are recorded in `BENCHMARKS.md`; forces are unchanged.
- Quadtree construction got a deterministic prefix-partition restructure: the serial build is 2.4x faster at N=100k (34.5 ms → 14.7 ms), and `--threads > 1` additionally schedules all leaf buckets across the rayon pool for a 4.7x threaded build vs the old serial (7.3 ms at 12 threads), with forces bitwise-identical to the previous build. The bucket-contiguous node layout also speeds up force traversal ~2.4x at N=100k as a side effect. Transient build allocations are charged against `--max-memory-mib`.
- Barnes-Hut force evaluation now scales with `--threads`: a persistent rayon pool with work-stealing over disjoint 64-particle chunks replaces per-call thread spawning, and the 50,000-particle single-thread fallback is gone. Measured 7.9x at N=10k and 11.1x at N=100k on 12 threads with bitwise-identical forces across thread counts; `--threads 1` stays strictly serial. Memory telemetry now charges the traversal stacks actually allocated.
- Hero video re-rendered as 20,000 golden bodies on black (additive star-like dots), framed at the README banner's aspect ratio (1984x794), with the velocity amplitude recalibrated so the cold-collapse virial ratio stays 0.30 at the higher N; the README now flows banner → explainer → clip, with the parameter table and repro command in a dedicated "Reproducing the clip" section.
- Hero video replaced with a cold-collapse run (8000-body Plummer cloud at virial ratio 0.30): minimal scientific rendering of single-pixel black dots on white, fixed camera, and a tail-into-head crossfade loop instead of the forward-reverse bounce.
- The `simd` cargo feature is renamed to `unrolled`: it is manual 4-lane scalar unrolling, not SIMD. A real f64x4 trial benchmarked slower than the existing unrolling, so the honest name ships instead; the feature now carries an accuracy parity test against the scalar kernel and CI builds both configurations.
- README hero video re-rendered with the corrected Barnes-Hut physics; the GIF now loops seamlessly.

### Fixed
- `--threads > 1` no longer slows Barnes-Hut force evaluation at small N: runs below 50,000 particles use the single-thread path (per-call thread spawn overhead dominated there), while larger runs keep the scoped-thread path; the threaded chunking is now safe Rust (`split_at_mut` instead of raw pointers).
- CI now actually builds, lints, and tests the code with a deterministic smoke run (the previous workflow only checked doc filenames and was red on every push).
- The `rk2` integrator is now true explicit midpoint (the previous hybrid scheme measured first-order convergence); measured order is 2.0 against the Kepler oracle.
- Barnes-Hut quadtree no longer zeroes a node's accumulated mass and center of mass when a leaf splits, which made approximate forces diverge from direct summation even at very small theta.
