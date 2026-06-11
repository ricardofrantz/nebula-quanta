# Vision

`nebula-quanta` (`nq`) aims to be a **credible mini physics tool**: a small,
fast 2D Barnes–Hut N-body simulator whose results can be trusted.

Quality over feature count. Every claim the tool makes — integrator order,
force accuracy versus the opening angle, energy conservation, deterministic
replay by seed — is backed by an automated, quantified check, not by
assertion. The direct O(n²) solver is the in-repo oracle; analytical
solutions (two-body Kepler orbits) sit above it where they apply.

What "improvement" means here, in priority order:

1. **Correctness floor** — the test suite compiles and runs, CI is green and
   actually exercises the code, and every integrator does what its name says.
2. **Numerical validity** — convergence-order tests, energy-drift regression
   baselines, force-accuracy-vs-theta sweeps with tolerances that fail loudly.
3. **Honest performance** — measured before optimized; features are named
   for what they actually do (the former `simd` feature is now `unrolled`
   for exactly this reason); speedups ship with an accuracy parity check.
4. **Capabilities** — only after the above: 3D, more integrators, richer
   output formats.
5. **Showcase at scale** — offline million-body renders (galaxy disks,
   mergers) that demonstrate the verified physics, every clip shipping with
   its exact seeded reproduction command and measured benchmark rows.

Non-goals: publication-scale astrophysics (no individual timesteps), GPU
compute, interactive GUI/real-time use, and SPH/fluids. The renders are a
demo of the physics, not the product — the receipts (benchmarks, parameter
tables, repro commands) are what make them worth publishing.
