# gallery-test-plummer-2k

Fast gallery recipe test. This clip is a small 2,000-body Plummer run at
640x360 resolution. It exists to verify the gallery convention quickly: one
folder, one no-argument script, one markdown receipt, and one final MP4.

## Output

- `gallery-test-plummer-2k.mp4`

## One-shot recipe

Run from this gallery folder:

```bash
cd galery/gallery-test-plummer-2k
./gallery-test-plummer-2k.sh
```

The script builds the release binary, writes the render to a temporary file in
this folder, moves only the final MP4 into place, and removes temporary files on
exit.

## Parameters

| Parameter | Value |
| --- | --- |
| bodies | 2,000 |
| steps | 40 |
| dt | 8.0e-4 |
| theta | 0.7 |
| epsilon | 1.0e-2 |
| init | plummer |
| init radius | 1.0 |
| init velocity amplitude | 0.03 |
| mass profile | lognormal, mean 1.0, stddev 0.2, clamp [0.5, 2.0] |
| seed | 12345 |
| integrator | leapfrog |
| view radius | 2.5 |
| threads | 4 |
| resolution | 640x360 |
| fps | 30 |
| every steps | 2 |

## Command

```bash
target/release/nq --n 2000 --steps 40 --dt 0.0008 --theta 0.7 --epsilon 0.01 --init plummer --init-radius 1.0 --init-v-amp 0.03 --mass-profile lognormal --mass-mean 1.0 --mass-stddev 0.2 --mass-min 0.5 --mass-max 2.0 --seed 12345 --integrator leapfrog --view-radius 2.5 --threads 4 --energy-drift off --width 640 --height 360 --record --output galery/gallery-test-plummer-2k/gallery-test-plummer-2k.mp4 --fps 30 --every-steps 2
```
