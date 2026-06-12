# gallery-galaxy-disk-2m

2M-body 4K galaxy-disk preview rendered by particle speed. This clip keeps the
total simulated mass near the 1M disk showcase by halving the per-particle mass,
then uses the turbo colormap to expose the hot central rotation curve and the
cooler outer disk.

This is intentionally a short preview render rather than a long production
movie: the 2M-particle, 4K, speed-colored path is expensive enough that a
31-frame clip is the practical gallery target.

## Output

- `gallery-galaxy-disk-2m.mp4`

## One-shot recipe

Run from this gallery folder:

```bash
cd galery/gallery-galaxy-disk-2m
./gallery-galaxy-disk-2m.sh
```

The script builds the release binary, writes the render to a temporary file in
this folder, moves only the final MP4 into place, and writes progress plus the
final simulation summary to `gallery-galaxy-disk-2m.log`.

## Parameters

| Parameter | Value |
| --- | --- |
| bodies | 2,000,000 |
| steps | 360 |
| dt | 3.0e-6 |
| theta | 0.7 |
| epsilon | 5.0e-3 |
| init | galaxy-disk |
| init radius | 1.0 |
| disk scale length | 0.25 |
| disk dispersion | 0.04 |
| mass profile | lognormal, mean 0.01, stddev 0.0025, clamp [0.005, 0.02] |
| seed | 424242 |
| integrator | leapfrog |
| view radius | 1.4 |
| color | speed, turbo, asinh, first-p95, headroom 1.35 |
| threads | 12 |
| resolution | 3840x2160 |
| fps | 12 |
| recorded frames | 31 |
| every steps | 12 |
| measured wall-clock | 8:38.85 |

## Command

```bash
target/release/nq --n 2000000 --steps 360 --dt 0.000003 --theta 0.7 --epsilon 0.005 --init galaxy-disk --init-radius 1.0 --disk-scale-length 0.25 --disk-dispersion 0.04 --mass-profile lognormal --mass-mean 0.01 --mass-stddev 0.0025 --mass-min 0.005 --mass-max 0.02 --seed 424242 --integrator leapfrog --view-radius 1.4 --threads 12 --energy-drift off --width 3840 --height 2160 --record --output galery/gallery-galaxy-disk-2m/gallery-galaxy-disk-2m.mp4 --fps 12 --every-steps 12 --color-by speed --colormap turbo --color-scale asinh --color-auto first-p95 --color-headroom 1.35 --progress-every 12
```
