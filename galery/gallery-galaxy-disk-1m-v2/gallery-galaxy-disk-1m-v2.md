# gallery-galaxy-disk-1m-v2

1M-body disk instability and clump-formation showcase render. This clip uses
the `galaxy-disk` initial condition with a cold exponential disk, a central
mass component, and low velocity dispersion. It is not advertised as a stable
spiral-arm simulation; the visible structures are disk fragmentation and clump
formation.

## Output

- `gallery-galaxy-disk-1m-v2.mp4`

## One-shot recipe

Run from this gallery folder:

```bash
cd galery/gallery-galaxy-disk-1m-v2
./gallery-galaxy-disk-1m-v2.sh
```

The script builds the release binary, writes the render to a temporary file in
this folder, moves only the final MP4 into place, and removes temporary files on
exit.

## Parameters

| Parameter | Value |
| --- | --- |
| bodies | 1,000,000 |
| steps | 3,000 |
| dt | 3.0e-6 |
| theta | 0.7 |
| epsilon | 5.0e-3 |
| init | galaxy-disk |
| init radius | 1.0 |
| disk scale length | 0.25 |
| disk dispersion | 0.04 |
| mass profile | lognormal, mean 0.02, stddev 0.005, clamp [0.01, 0.04] |
| seed | 424242 |
| integrator | leapfrog |
| view radius | 1.4 |
| threads | 12 |
| resolution | 1920x1080 |
| fps | 30 |
| recorded frames | 51 |
| every steps | 60 |
| previous wall-clock | 15m18.548s |

## Command

```bash
target/release/nq --n 1000000 --steps 3000 --dt 0.000003 --theta 0.7 --epsilon 0.005 --init galaxy-disk --init-radius 1.0 --disk-scale-length 0.25 --disk-dispersion 0.04 --mass-profile lognormal --mass-mean 0.02 --mass-stddev 0.005 --mass-min 0.01 --mass-max 0.04 --seed 424242 --integrator leapfrog --view-radius 1.4 --threads 12 --energy-drift off --width 1920 --height 1080 --record --output galery/gallery-galaxy-disk-1m-v2/gallery-galaxy-disk-1m-v2.mp4 --fps 30 --every-steps 60
```
