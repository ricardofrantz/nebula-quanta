#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/render_video.sh <frames_dir> <output.mp4> [fps] [crf] [preset] [codec]

Examples:
  scripts/render_video.sh capture nebula-quanta-barnes_hut.mp4 60 20 fast libx264
  scripts/render_video.sh capture nebula-quanta-barnes_hut-4k.mp4 30 18 slow h264_nvenc
USAGE
}

if [[ $# -lt 2 ]]; then
  usage
  exit 1
fi

FRAMES_DIR=$1
OUTPUT=$2
FPS=${3:-60}
CRF=${4:-20}
PRESET=${5:-fast}
CODEC=${6:-libx264}

if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "ffmpeg not found in PATH" >&2
  exit 1
fi

if [[ ! -d $FRAMES_DIR ]]; then
  echo "missing frame directory: $FRAMES_DIR" >&2
  exit 1
fi

ffmpeg -y \
  -framerate "$FPS" \
  -i "$FRAMES_DIR/frame_%06d.ppm" \
  -c:v "$CODEC" \
  -pix_fmt yuv420p \
  -preset "$PRESET" \
  -crf "$CRF" \
  "$OUTPUT"

echo "rendered $OUTPUT"
