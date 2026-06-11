# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed
- CI now actually builds, lints, and tests the code with a deterministic smoke run (the previous workflow only checked doc filenames and was red on every push).
- Barnes-Hut quadtree no longer zeroes a node's accumulated mass and center of mass when a leaf splits, which made approximate forces diverge from direct summation even at very small theta.
