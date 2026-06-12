#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 0 ]]; then
  echo "usage: ./gallery-test-plummer-2k.sh" >&2
  echo "all render settings are fixed inside this script; no arguments are accepted" >&2
  exit 2
fi

# Fast gallery test case.
#
# Run from this directory as:
#
#   ./gallery-test-plummer-2k.sh
#
# This is intentionally small: it exists to verify the gallery recipe shape
# quickly, not to be a million-body showcase render. It builds the release
# binary, renders into a private temporary directory, moves only the finished
# MP4 into this folder, and deletes temporary files.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
name="gallery-test-plummer-2k"
tmp_dir="${script_dir}/.tmp-${name}"
tmp_mp4="${tmp_dir}/${name}.mp4"
out_mp4="${script_dir}/${name}.mp4"

# Fixed simulation settings for this clip. Edit these values in this file to
# make a new gallery variant; do not pass command-line arguments.
n=5000
steps=240
dt=0.0008
theta=0.7213232
epsilon=0.014412
init=plummer
init_radius=1.0
init_v_amp=0.034
mass_profile=lognormal
mass_mean=1.1
mass_stddev=0.2
mass_min=0.5
mass_max=2.2
seed=12345
integrator=leapfrog
view_radius=2.5
threads=4
width=1280
height=720
fps=30
every_steps=2

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
  --init-v-amp "${init_v_amp}" \
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
