# gallery-shear-fragmentation-speed-35k

35K-body rotating disk experiment rendered by particle speed. This is not
advertised as a stable spiral-arm model: the intentionally cool disk shears
apart, fragments, and forms bright moving clumps. The turbo palette makes the
velocity structure visible as the disk loses its smooth opening shape.

## Output

- `gallery-shear-fragmentation-speed-35k.mp4`

## One-shot recipe

Run from this gallery folder:

```bash
cd galery/gallery-shear-fragmentation-speed-35k
./gallery-shear-fragmentation-speed-35k.sh
```

The script builds the release binary, writes the render to a temporary file in
this folder, moves only the final MP4 into place, and removes temporary files on
exit.

## Parameters

| Parameter | Value |
| --- | --- |
| bodies | 35,000 |
| steps | 360 |
| dt | 2.4e-4 |
| theta | 0.7 |
| epsilon | 1.0e-2 |
| init | rotating-disk |
| init radius | 1.45 |
| init velocity amplitude | 42 |
| init lambda | 1.4 |
| mass profile | lognormal, mean 0.6, stddev 0.18, clamp [0.25, 1.2] |
| seed | 8675309 |
| integrator | leapfrog |
| view radius | 2.1 |
| color | speed, turbo, asinh, first-p95, headroom 1.4 |
| threads | 12 |
| resolution | 960x540 |
| fps | 30 |
| recorded frames | 91 |
| every steps | 4 |
| measured wall-clock | 20.00s |

## Command

```bash
target/release/nq --n 35000 --steps 360 --dt 0.00024 --theta 0.7 --epsilon 0.01 --init rotating-disk --init-radius 1.45 --init-v-amp 42 --init-lambda 1.4 --mass-profile lognormal --mass-mean 0.6 --mass-stddev 0.18 --mass-min 0.25 --mass-max 1.2 --seed 8675309 --integrator leapfrog --view-radius 2.1 --threads 12 --energy-drift off --width 960 --height 540 --record --output galery/gallery-shear-fragmentation-speed-35k/gallery-shear-fragmentation-speed-35k.mp4 --fps 30 --every-steps 4 --color-by speed --colormap turbo --color-scale asinh --color-auto first-p95 --color-headroom 1.4
```
