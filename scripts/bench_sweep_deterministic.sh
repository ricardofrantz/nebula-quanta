#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/bench_sweep_deterministic.sh [options]

Options:
  --n <particle count>              (default: 20000)
  --steps <integration steps>        (default: 200)
  --dt <time step>                  (default: 0.001)
  --epsilon <softening>             (default: 0.01)
  --seed <rng seed>                 (default: 42)
  --theta <comma-separated list>    (default: 0.3,0.5,0.7,1.0)
  --threads <comma-separated list>  (default: 1)
  --mode <barnes_hut|direct>        (default: barnes_hut)
  --csv <path>                      (required: output CSV path)
  --help
USAGE
}

N=20000
STEPS=200
DT=0.001
EPSILON=0.01
SEED=42
THETAS=0.3,0.5,0.7,1.0
THREADS=1
MODE=barnes_hut
CSV_OUT=""

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
    --seed)
      SEED=$2
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
    --csv)
      CSV_OUT=$2
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

if [[ -z "${CSV_OUT}" ]]; then
  echo "error: --csv is required"
  usage
  exit 1
fi

bash "$(dirname "$0")/bench_sweep.sh" \
  --n "$N" \
  --steps "$STEPS" \
  --dt "$DT" \
  --epsilon "$EPSILON" \
  --seed "$SEED" \
  --theta "$THETAS" \
  --threads "$THREADS" \
  --mode "$MODE" \
  --csv "$CSV_OUT"
