#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 0 ]]; then
  echo "usage: ./gallery-galaxy-disk-2m.sh" >&2
  echo "all render settings are fixed inside this script; no arguments are accepted" >&2
  exit 2
fi

# One-shot 2M-particle gallery recipe.
# Run from this directory as:
#
#   ./gallery-galaxy-disk-2m.sh
#
# The script builds the release binary, renders into a private temporary
# directory, moves only the finished MP4 into this folder, and deletes
# temporary files on exit.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
name="gallery-galaxy-disk-2m"
tmp_dir="${script_dir}/.tmp-${name}"
tmp_mp4="${tmp_dir}/${name}.mp4"
out_mp4="${script_dir}/${name}.mp4"
log_file="${script_dir}/${name}.log"

# Scale from gallery-galaxy-disk-1m-v2:
# - 2x particles for a smoother disk.
# - 2x simulated duration at the same sampling cadence: 101 frames at 30 fps.
# - 4K frame size for inspection on a desktop display.
# - Half the 1M per-particle mass so total simulated mass stays comparable.
n=2000000
steps=6000
dt=0.000003
theta=0.7
epsilon=0.005
init=galaxy-disk
init_radius=1.0
disk_scale_length=0.25
disk_dispersion=0.04
mass_profile=lognormal
mass_mean=0.01
mass_stddev=0.0025
mass_min=0.005
mass_max=0.02
seed=424242
integrator=leapfrog
view_radius=1.4
threads=12
width=3840
height=2160
fps=30
every_steps=60
progress_every="${every_steps}"
color_by=speed
colormap=turbo
color_scale=asinh
color_auto=first-p95
color_headroom=1.35

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
  --disk-scale-length "${disk_scale_length}" \
  --disk-dispersion "${disk_dispersion}" \
  --mass-profile "${mass_profile}" \
  --mass-mean "${mass_mean}" \
  --mass-stddev "${mass_stddev}" \
  --mass-min "${mass_min}" \
  --mass-max "${mass_max}" \
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
  --color-auto "${color_auto}" \
  --color-headroom "${color_headroom}" \
  --progress-every "${progress_every}" \
  2>&1 | tee "${log_file}"

mv "${tmp_mp4}" "${out_mp4}"
