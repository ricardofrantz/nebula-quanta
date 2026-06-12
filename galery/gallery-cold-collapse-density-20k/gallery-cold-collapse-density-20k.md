# gallery-cold-collapse-density-20k

20K-body cold Plummer collapse rendered by local density. The clip starts from
a low-velocity spherical cloud and shows core formation plus a diffuse halo.
The magma palette emphasizes the density contrast during violent relaxation.

## Output

- `gallery-cold-collapse-density-20k.mp4`

## One-shot recipe

Run from this gallery folder:

```bash
cd galery/gallery-cold-collapse-density-20k
./gallery-cold-collapse-density-20k.sh
```

The script builds the release binary, writes the render to a temporary file in
this folder, moves only the final MP4 into place, and removes temporary files on
exit.

## Parameters

| Parameter | Value |
| --- | --- |
| bodies | 20,000 |
| steps | 420 |
| dt | 1.8e-4 |
| theta | 0.7 |
| epsilon | 5.0e-3 |
| init | plummer |
| init radius | 1.0 |
| init velocity amplitude | 20 |
| mass profile | lognormal, mean 1.0, stddev 0.5, clamp [0.2, 5.0] |
| seed | 1902 |
| integrator | leapfrog |
| view radius | 3.0 |
| color | density, magma, asinh, first-p99, headroom 1.2 |
| threads | 12 |
| resolution | 960x540 |
| fps | 30 |
| recorded frames | 106 |
| every steps | 4 |
| measured wall-clock | 15.68s |

## Command

```bash
target/release/nq --n 20000 --steps 420 --dt 0.00018 --theta 0.7 --epsilon 0.005 --init plummer --init-radius 1.0 --init-v-amp 20 --init-lambda 1.0 --mass-profile lognormal --mass-mean 1.0 --mass-stddev 0.5 --mass-min 0.2 --mass-max 5.0 --seed 1902 --integrator leapfrog --view-radius 3.0 --threads 12 --energy-drift off --width 960 --height 540 --record --output galery/gallery-cold-collapse-density-20k/gallery-cold-collapse-density-20k.mp4 --fps 30 --every-steps 4 --color-by density --colormap magma --color-scale asinh --color-auto first-p99 --color-headroom 1.2
```
