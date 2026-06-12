# gallery-retrograde-merger-accel-50k

50K-body unequal-mass retrograde disk encounter rendered by acceleration
magnitude. The run uses the deterministic `merger` initial condition with a
smaller retrograde secondary. The inferno palette highlights the high-force
knots, bridge material, and compact clumps that appear during the encounter.

## Output

- `gallery-retrograde-merger-accel-50k.mp4`

## One-shot recipe

Run from this gallery folder:

```bash
cd galery/gallery-retrograde-merger-accel-50k
./gallery-retrograde-merger-accel-50k.sh
```

The script builds the release binary, writes the render to a temporary file in
this folder, moves only the final MP4 into place, and removes temporary files on
exit.

## Parameters

| Parameter | Value |
| --- | --- |
| bodies | 50,000 |
| steps | 760 |
| dt | 2.0e-5 |
| theta | 0.7 |
| epsilon | 2.0e-2 |
| init | merger |
| init radius | 1.0 |
| disk scale length | 0.24 |
| disk dispersion | 0.035 |
| merger mass ratio | 0.55 |
| merger separation | 2.7 |
| merger impact parameter | 0.65 |
| merger spin | retrograde |
| mass profile | lognormal, mean 0.4, stddev 0.1, clamp [0.2, 0.8] |
| seed | 424200 |
| integrator | leapfrog |
| view radius | 3.0 |
| color | accel, inferno, asinh, first-p99, headroom 2.0 |
| threads | 12 |
| resolution | 960x540 |
| fps | 30 |
| recorded frames | 96 |
| every steps | 8 |
| measured wall-clock | 41.36s |

## Command

```bash
target/release/nq --n 50000 --steps 760 --dt 0.00002 --theta 0.7 --epsilon 0.02 --init merger --init-radius 1.0 --disk-scale-length 0.24 --disk-dispersion 0.035 --merger-mass-ratio 0.55 --merger-separation 2.7 --merger-impact-parameter 0.65 --merger-spin retrograde --mass-profile lognormal --mass-mean 0.4 --mass-stddev 0.1 --mass-min 0.2 --mass-max 0.8 --seed 424200 --integrator leapfrog --view-radius 3.0 --threads 12 --energy-drift off --width 960 --height 540 --record --output galery/gallery-retrograde-merger-accel-50k/gallery-retrograde-merger-accel-50k.mp4 --fps 30 --every-steps 8 --color-by accel --colormap inferno --color-scale asinh --color-auto first-p99 --color-headroom 2.0
```
