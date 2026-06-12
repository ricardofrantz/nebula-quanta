# gallery-mass-spectrum-plummer-25k

25K-body Plummer run with a heavy-tailed power-law mass spectrum. The motion is
deliberately calmer than the collapse and merger clips; the point is to make
the population structure visible. Particle color encodes mass with a log-scaled
plasma palette from 0.15 to 8.0.

## Output

- `gallery-mass-spectrum-plummer-25k.mp4`

## One-shot recipe

Run from this gallery folder:

```bash
cd galery/gallery-mass-spectrum-plummer-25k
./gallery-mass-spectrum-plummer-25k.sh
```

The script builds the release binary, writes the render to a temporary file in
this folder, moves only the final MP4 into place, and removes temporary files on
exit.

## Parameters

| Parameter | Value |
| --- | --- |
| bodies | 25,000 |
| steps | 360 |
| dt | 1.6e-4 |
| theta | 0.7 |
| epsilon | 8.0e-3 |
| init | plummer |
| init radius | 1.0 |
| init velocity amplitude | 60 |
| init lambda | 0.8 |
| mass profile | pow-law, alpha 1.35, clamp [0.15, 8.0] |
| seed | 314159 |
| integrator | leapfrog |
| view radius | 3.0 |
| color | mass, plasma, log, fixed [0.15, 8.0] |
| threads | 12 |
| resolution | 960x540 |
| fps | 30 |
| recorded frames | 91 |
| every steps | 4 |
| measured wall-clock | 8.80s |

## Command

```bash
target/release/nq --n 25000 --steps 360 --dt 0.00016 --theta 0.7 --epsilon 0.008 --init plummer --init-radius 1.0 --init-v-amp 60 --init-lambda 0.8 --mass-profile pow-law --mass-mean 1.0 --mass-stddev 0.25 --mass-min 0.15 --mass-max 8.0 --mass-alpha 1.35 --seed 314159 --integrator leapfrog --view-radius 3.0 --threads 12 --energy-drift off --width 960 --height 540 --record --output galery/gallery-mass-spectrum-plummer-25k/gallery-mass-spectrum-plummer-25k.mp4 --fps 30 --every-steps 4 --color-by mass --colormap plasma --color-scale log --color-min 0.15 --color-max 8.0
```
