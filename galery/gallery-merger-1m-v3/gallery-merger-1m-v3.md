# gallery-merger-1m-v3

1M-body two-galaxy merger showcase render. The clip follows first passage and
tidal-stream formation using the deterministic `merger` initial condition,
which composes two `galaxy-disk` realizations in the center-of-mass frame.

## Output

- `gallery-merger-1m-v3.mp4`

## One-shot recipe

Run from this gallery folder:

```bash
cd galery/gallery-merger-1m-v3
./gallery-merger-1m-v3.sh
```

The script builds the release binary, writes the render to a temporary file in
this folder, moves only the final MP4 into place, and removes temporary files on
exit.

## Parameters

| Parameter | Value |
| --- | --- |
| bodies | 1,000,000 |
| steps | 6,000 |
| dt | 2.0e-5 |
| theta | 0.7 |
| epsilon | 2.0e-2 |
| init | merger |
| init radius | 1.0 |
| disk scale length | 0.25 |
| disk dispersion | 0.04 |
| merger mass ratio | 0.75 |
| merger separation | 3.0 |
| merger impact parameter | 0.5 |
| merger spin | prograde |
| mass profile | lognormal, mean 0.02, stddev 0.005, clamp [0.01, 0.04] |
| seed | 271828 |
| integrator | leapfrog |
| view radius | 3.4 |
| threads | 12 |
| resolution | 1920x1080 |
| fps | 30 |
| recorded frames | 201 |
| every steps | 30 |
| previous wall-clock | 35m14.060s |
| MP4 encode | H.264 CRF 28 |

## Command

```bash
target/release/nq --n 1000000 --steps 6000 --dt 0.00002 --theta 0.7 --epsilon 0.02 --init merger --init-radius 1.0 --disk-scale-length 0.25 --disk-dispersion 0.04 --merger-mass-ratio 0.75 --merger-separation 3.0 --merger-impact-parameter 0.5 --merger-spin prograde --mass-profile lognormal --mass-mean 0.02 --mass-stddev 0.005 --mass-min 0.01 --mass-max 0.04 --seed 271828 --integrator leapfrog --view-radius 3.4 --threads 12 --energy-drift off --width 1920 --height 1080 --record --output galery/gallery-merger-1m-v3/gallery-merger-1m-v3.mp4 --fps 30 --every-steps 30
```
