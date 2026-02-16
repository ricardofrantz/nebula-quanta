#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/render_presets.sh <frames_dir> <output.mp4> [--preset <name>] [--help]

Optional presets:
  hd30      1920x1080 @ 30 fps, fast preset, crf 20, libx264
  hd60      1920x1080 @ 60 fps, fast preset, crf 18, libx264
  hq30      1920x1080 @ 30 fps, slow preset, crf 16, h264_nvenc
  hq60      1920x1080 @ 60 fps, slow preset, crf 16, h264_nvenc

Examples:
  scripts/render_presets.sh ./captured_run nebula-quanta-vid.mp4 --preset hd60
  scripts/render_presets.sh ./captured_run nebula-quanta-vid.mp4 --preset hq30
USAGE
}

if [[ $# -lt 2 ]]; then
  usage
  exit 1
fi

FRAMES_DIR=$1
OUTPUT=$2
shift 2

PRESET=hd60

while [[ $# -gt 0 ]]; do
  case "$1" in
    --preset)
      PRESET="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown arg: $1"
      usage
      exit 1
      ;;
  esac
done

if [[ ! -d "$FRAMES_DIR" ]]; then
  echo "missing frame directory: $FRAMES_DIR" >&2
  exit 1
fi

case "$PRESET" in
  hd30)
    FPS=30
    CRF=20
    ENCODER=libx264
    VPRESET=fast
    ;;
  hd60)
    FPS=60
    CRF=18
    ENCODER=libx264
    VPRESET=fast
    ;;
  hq30)
    FPS=30
    CRF=16
    ENCODER=h264_nvenc
    VPRESET=slow
    ;;
  hq60)
    FPS=60
    CRF=16
    ENCODER=h264_nvenc
    VPRESET=slow
    ;;
  *)
    echo "unknown preset: $PRESET" >&2
    usage
    exit 1
  ;;
esac

FRAME_COUNT=$(rg --files "$FRAMES_DIR" | rg -c 'frame_[0-9]{6}\.ppm$' || true)
if [[ "$FRAME_COUNT" == "0" ]]; then
  echo "no frames found in $FRAMES_DIR" >&2
  exit 1
fi

EST_SECONDS=$(awk -v frames="$FRAME_COUNT" -v fps="$FPS" 'BEGIN { printf "%.2f", (fps > 0 ? frames / fps : 0) }')
EST_MINUTES=$(awk -v sec="$EST_SECONDS" 'BEGIN { printf "%.2f", sec / 60.0 }')
echo "render_preset=${PRESET} frames=${FRAME_COUNT} fps=${FPS} est_seconds=${EST_SECONDS} est_minutes=${EST_MINUTES}"

if [[ "$ENCODER" == "h264_nvenc" ]] && ! ffmpeg -encoders | rg -q "h264_nvenc"; then
  echo "h264_nvenc not available, falling back to libx264"
  ENCODER=libx264
fi

bash "$(dirname "$0")/render_video.sh" \
  "$FRAMES_DIR" \
  "$OUTPUT" \
  "$FPS" \
  "$CRF" \
  "$VPRESET" \
  "$ENCODER"
