# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Two-body Kepler analytical oracle tests: leapfrog verified against closed-form circular orbits (position and energy-conservation bounds).

### Changed
- README hero video re-rendered with the corrected Barnes-Hut physics; the GIF now loops seamlessly.

### Fixed
- CI now actually builds, lints, and tests the code with a deterministic smoke run (the previous workflow only checked doc filenames and was red on every push).
- The `rk2` integrator is now true explicit midpoint (the previous hybrid scheme measured first-order convergence); measured order is 2.0 against the Kepler oracle.
- Barnes-Hut quadtree no longer zeroes a node's accumulated mass and center of mass when a leaf splits, which made approximate forces diverge from direct summation even at very small theta.
