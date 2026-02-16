#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/bench_sweep.sh [options]

Options:
  --n <particle count>              (default: 20000)
  --steps <integration steps>        (default: 200)
  --dt <time step>                  (default: 0.001)
  --epsilon <softening>             (default: 0.01)
  --theta <comma-separated list>    (default: 0.3,0.5,0.7,1.0)
  --threads <comma-separated list>  (default: 1)
  --mode <barnes_hut|direct>        (default: barnes_hut)
  --help
USAGE
}

N=20000
STEPS=200
DT=0.001
EPSILON=0.01
THETAS=0.3,0.5,0.7,1.0
THREADS=1
MODE=barnes_hut

while [[ $# -gt 0 ]]; do
  case "$1" in
    --n)
      N=$2
      shift 2
      ;;
    --steps)
      STEPS=$2
      shift 2
      ;;
    --dt)
      DT=$2
      shift 2
      ;;
    --epsilon)
      EPSILON=$2
      shift 2
      ;;
    --theta)
      THETAS=$2
      shift 2
      ;;
    --threads)
      THREADS=$2
      shift 2
      ;;
    --mode)
      MODE=$2
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

IFS=',' read -r -a THE_LIST <<< "$THETAS"
IFS=',' read -r -a THR_LIST <<< "$THREADS"

for theta in "${THE_LIST[@]}"; do
  for thread_count in "${THR_LIST[@]}"; do
    if [[ -z "${theta}" ]]; then
      continue
    fi
    if [[ -z "${thread_count}" ]]; then
      continue
    fi

    CMD=(cargo run --release -- --mode "$MODE" --n "$N" --steps "$STEPS" --dt "$DT" --theta "$theta" --epsilon "$EPSILON")

    if [[ "$MODE" == "barnes_hut" ]]; then
      CMD+=(--threads "$thread_count")
    fi

    echo "===== theta=$theta threads=$thread_count ====="
    "${CMD[@]}"
    echo
  done
done
