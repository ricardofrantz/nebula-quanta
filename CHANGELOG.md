# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Criterion benchmark suite (`cargo bench`) for tree build, Barnes-Hut force, and direct force, with a recorded baseline in `BENCHMARKS.md`; the crate now also exposes a library target.
- Two-body Kepler analytical oracle tests: leapfrog verified against closed-form circular orbits (position and energy-conservation bounds).

### Changed
- README hero video re-rendered with the corrected Barnes-Hut physics; the GIF now loops seamlessly.

### Fixed
- `--threads > 1` no longer slows Barnes-Hut force evaluation at small N: runs below 50,000 particles use the single-thread path (per-call thread spawn overhead dominated there), while larger runs keep the scoped-thread path; the threaded chunking is now safe Rust (`split_at_mut` instead of raw pointers).
- CI now actually builds, lints, and tests the code with a deterministic smoke run (the previous workflow only checked doc filenames and was red on every push).
- The `rk2` integrator is now true explicit midpoint (the previous hybrid scheme measured first-order convergence); measured order is 2.0 against the Kepler oracle.
- Barnes-Hut quadtree no longer zeroes a node's accumulated mass and center of mass when a leaf splits, which made approximate forces diverge from direct summation even at very small theta.
