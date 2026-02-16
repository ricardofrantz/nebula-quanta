# Barnes–Hut Benchmark Sweep Results

## Command

```bash
scripts/bench_sweep.sh --n 5000 --steps 50 --dt 0.001 --epsilon 0.01 --theta 0.3,0.5,0.7,1.0 --threads 1,4
```

## Environment

- Date: 2026-02-16 (UTC local workspace)
- Binary: `cargo run --release`
- Build profile: `release` with LTO/codegen-units=1/strip symbols

## Results

| theta | threads | steps_per_sec | ns_per_particle_force | total_ms | build_ms | force_ms | integrate_ms | workspace_bytes | node_utilization |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.3 | 1 | 60.057 | 3217.4 | 832.548 | 27.715 | 804.348 | 0.485 | 2_040_088 | 87.78% |
| 0.3 | 4 | 181.816 | 975.2 | 275.003 | 30.747 | 243.809 | 0.446 | 2_520_112 | 87.78% |
| 0.5 | 1 | 121.196 | 1537.4 | 412.556 | 27.807 | 384.353 | 0.396 | 2_040_088 | 88.14% |
| 0.5 | 4 | 347.030 | 454.1 | 144.080 | 29.608 | 113.526 | 0.945 | 2_520_112 | 88.14% |
| 0.7 | 1 | 175.220 | 1027.3 | 285.356 | 28.138 | 256.823 | 0.395 | 2_040_088 | 88.98% |
| 0.7 | 4 | 465.545 | 312.1 | 107.401 | 28.947 | 78.029 | 0.425 | 2_520_112 | 88.98% |
| 1.0 | 1 | 283.606 | 593.1 | 176.301 | 27.665 | 148.287 | 0.350 | 2_040_088 | 88.24% |
| 1.0 | 4 | 603.730 | 208.6 | 82.818 | 30.217 | 52.142 | 0.459 | 2_520_112 | 88.24% |

## Observations

- Parallel worker mode (`threads=4`) consistently improved throughput in this workload.
- Higher `theta` reduced force work and improved wall-time.
- `workspace_bytes` increases with threaded mode due to per-thread traversal stack duplication.
