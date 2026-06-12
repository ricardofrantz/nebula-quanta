#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 0 ]]; then
  echo "usage: ./gallery-galaxy-disk-1m-v2.sh" >&2
  echo "all render settings are fixed inside this script; no arguments are accepted" >&2
  exit 2
fi

# This script is the complete, one-shot recipe for this gallery MP4.
# Run it from this directory as:
#
#   ./gallery-galaxy-disk-1m-v2.sh
#
# It builds the release binary, renders into a private temporary directory,
# moves only the finished MP4 into this folder, and deletes temporary files.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
name="gallery-galaxy-disk-1m-v2"
tmp_dir="${script_dir}/.tmp-${name}"
tmp_mp4="${tmp_dir}/${name}.mp4"
out_mp4="${script_dir}/${name}.mp4"

# Fixed simulation settings for this clip. Edit these values in this file to
# make a new gallery variant; do not pass command-line arguments.
n=1000000
steps=3000
dt=0.000003
theta=0.7
epsilon=0.005
init=galaxy-disk
init_radius=1.0
disk_scale_length=0.25
disk_dispersion=0.04
mass_profile=lognormal
mass_mean=0.02
mass_stddev=0.005
mass_min=0.01
mass_max=0.04
seed=424242
integrator=leapfrog
view_radius=1.4
threads=12
width=1920
height=1080
fps=30
every_steps=60

cleanup() {
  rm -rf "${tmp_dir}"
}
trap cleanup EXIT

cd "${repo_root}"

# Uncomment these when you want the recipe to print repository provenance.
# git rev-parse HEAD
# git status --short

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
  --every-steps "${every_steps}"

mv "${tmp_mp4}" "${out_mp4}"
