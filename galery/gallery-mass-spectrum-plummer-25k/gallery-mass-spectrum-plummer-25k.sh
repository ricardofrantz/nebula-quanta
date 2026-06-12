#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 0 ]]; then
  echo "usage: ./gallery-mass-spectrum-plummer-25k.sh" >&2
  echo "all render settings are fixed inside this script; no arguments are accepted" >&2
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
name="gallery-mass-spectrum-plummer-25k"
tmp_dir="${script_dir}/.tmp-${name}"
tmp_mp4="${tmp_dir}/${name}.mp4"
out_mp4="${script_dir}/${name}.mp4"
log_file="${script_dir}/${name}.log"

n=25000
steps=360
dt=0.00016
theta=0.7
epsilon=0.008
init=plummer
init_radius=1.0
init_v_amp=60
init_lambda=0.8
mass_profile=pow-law
mass_mean=1.0
mass_stddev=0.25
mass_min=0.15
mass_max=8.0
mass_alpha=1.35
seed=314159
integrator=leapfrog
view_radius=3.0
threads=12
width=960
height=540
fps=30
every_steps=4
progress_every="${every_steps}"
color_by=mass
colormap=plasma
color_scale=log
color_min=0.15
color_max=8.0

cleanup() {
  rm -rf "${tmp_dir}"
}
trap cleanup EXIT

cd "${repo_root}"
cargo build --release

rm -rf "${tmp_dir}"
mkdir -p "${tmp_dir}"

target/release/nq \
  --n "${n}" \
  --steps "${steps}" \
  --dt "${dt}" \
  --theta "${theta}" \
  --epsilon "${epsilon}" \
  --init "${init}" \
  --init-radius "${init_radius}" \
  --init-v-amp "${init_v_amp}" \
  --init-lambda "${init_lambda}" \
  --mass-profile "${mass_profile}" \
  --mass-mean "${mass_mean}" \
  --mass-stddev "${mass_stddev}" \
  --mass-min "${mass_min}" \
  --mass-max "${mass_max}" \
  --mass-alpha "${mass_alpha}" \
  --seed "${seed}" \
  --integrator "${integrator}" \
  --view-radius "${view_radius}" \
  --threads "${threads}" \
  --energy-drift off \
  --width "${width}" \
  --height "${height}" \
  --record \
  --output "${tmp_mp4}" \
  --fps "${fps}" \
  --every-steps "${every_steps}" \
  --color-by "${color_by}" \
  --colormap "${colormap}" \
  --color-scale "${color_scale}" \
  --color-min "${color_min}" \
  --color-max "${color_max}" \
  --progress-every "${progress_every}" \
  2>&1 | tee "${log_file}"

mv "${tmp_mp4}" "${out_mp4}"
