# Benchmark baseline

Baseline for bead `nebula-quanta-83w` and future SIMD work.

- Date: 2026-06-11
- Machine: AMD Ryzen 9 9900X 12-Core Processor
- Cores visible to OS: 18 CPUs (`lscpu`: 18 CPUs, 1 thread/core, 1 socket)
- Rust: `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Criterion: `0.5.1` (`criterion = "0.5"`)
- Command: `cargo bench`
- Initial conditions: Plummer, seed 42

| Group | N | Threads | Median |
| --- | ---: | ---: | ---: |
| tree build | 1,000 | n/a | 92.529 µs |
| tree build | 10,000 | n/a | 1.338 ms |
| BH force eval | 1,000 | 1 | 539.247 µs |
| BH force eval | 1,000 | 4 | 3.336 ms |
| BH force eval | 10,000 | 1 | 9.548 ms |
| BH force eval | 10,000 | 4 | 11.352 ms |
| direct force eval | 1,000 | scalar | 1.521 ms |
| direct force eval | 4,000 | scalar | 24.780 ms |
