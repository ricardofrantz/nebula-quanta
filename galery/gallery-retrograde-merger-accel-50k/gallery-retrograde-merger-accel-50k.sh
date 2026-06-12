#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 0 ]]; then
  echo "usage: ./gallery-retrograde-merger-accel-50k.sh" >&2
  echo "all render settings are fixed inside this script; no arguments are accepted" >&2
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
name="gallery-retrograde-merger-accel-50k"
tmp_dir="${script_dir}/.tmp-${name}"
tmp_mp4="${tmp_dir}/${name}.mp4"
out_mp4="${script_dir}/${name}.mp4"

n=50000
# Longer than the first gallery pass: keep the liked encounter parameters, but
# carry the simulation through the bridge, tail growth, and compact remnant.
steps=1800
dt=0.00002
theta=0.7
epsilon=0.02
init=merger
init_radius=1.0
disk_scale_length=0.24
disk_dispersion=0.035
merger_mass_ratio=0.55
merger_separation=2.7
merger_impact_parameter=0.65
merger_spin=retrograde
mass_profile=lognormal
mass_mean=0.4
mass_stddev=0.1
mass_min=0.2
mass_max=0.8
seed=424200
integrator=leapfrog
view_radius=3.0
threads=12
width=1280
height=720
fps=30
# 1800 steps / 6 gives 301 frames including step zero: about 10 seconds at 30 fps.
every_steps=6
color_by=accel
colormap=inferno
color_scale=asinh
color_auto=first-p99
color_headroom=3.0

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
  --merger-mass-ratio "${merger_mass_ratio}" \
  --merger-separation "${merger_separation}" \
  --merger-impact-parameter "${merger_impact_parameter}" \
  --merger-spin "${merger_spin}" \
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
  --color-headroom "${color_headroom}"

mv "${tmp_mp4}" "${out_mp4}"
