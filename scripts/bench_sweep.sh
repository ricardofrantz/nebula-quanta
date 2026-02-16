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
  --seed <rng seed>                 (default: 42)
  --theta <comma-separated list>    (default: 0.3,0.5,0.7,1.0)
  --threads <comma-separated list>  (default: 1)
  --mode <barnes_hut|direct>        (default: barnes_hut)
  --csv <path>                      (optional CSV output target)
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

    CMD=(cargo run --release -- --mode "$MODE" --n "$N" --steps "$STEPS" --dt "$DT" --theta "$theta" --epsilon "$EPSILON" --seed "$SEED")

    if [[ "$MODE" == "barnes_hut" ]]; then
      CMD+=(--threads "$thread_count")
    fi

    echo "===== theta=$theta threads=$thread_count ====="
    tmp_output=$(mktemp)

    if ! "${CMD[@]}" | tee "$tmp_output"; then
      rm -f "$tmp_output"
      exit 1
    fi

    if [[ -n "$CSV_OUT" ]]; then
      summary_line=$(rg '^mode=' "$tmp_output" | tail -n 1 || true)
      if [[ -z "$summary_line" ]]; then
        echo "unable to capture benchmark summary line for theta=$theta threads=$thread_count" >&2
        rm -f "$tmp_output"
        exit 1
      fi

      field() {
        local key="$1"
        local line="$2"
        echo "$line" | awk -v key="$key" '
          {
            for (i = 1; i <= NF; i++) {
              split($i, parts, "=");
              if (parts[1] == key) {
                print parts[2];
              }
            }
          }
        '
      }

      if [[ ! -s "$CSV_OUT" ]]; then
        {
          echo "theta,threads,mode,n,steps,dt,theta_value,epsilon,seed,build_ms,force_ms,integrate_ms,total_ms,avg_step_ms,steps_per_sec,ns_per_particle_force,peak_nodes,node_capacity,node_utilization,workspace_bytes,bytes_per_particle,particle_bytes,node_bytes,stack_bytes,energy_drift_abs,energy_drift_rel"
        } > "$CSV_OUT"
      fi

      {
        echo "$(field theta "$summary_line"),$(field threads "$summary_line"),$(field mode "$summary_line"),$(field n "$summary_line"),$(field steps "$summary_line"),$(field dt "$summary_line"),$(field theta "$summary_line"),$(field epsilon "$summary_line"),${SEED},$(field build_ms "$summary_line"),$(field force_ms "$summary_line"),$(field integrate_ms "$summary_line"),$(field total_ms "$summary_line"),$(field avg_step_ms "$summary_line"),$(field steps_per_sec "$summary_line"),$(field ns_per_particle_force "$summary_line"),$(field peak_nodes "$summary_line"),$(field node_capacity "$summary_line"),$(field node_utilization "$summary_line"),$(field workspace_bytes "$summary_line"),$(field bytes_per_particle "$summary_line"),$(field particle_bytes "$summary_line"),$(field node_bytes "$summary_line"),$(field stack_bytes "$summary_line"),$(field energy_drift_abs "$summary_line"),$(field energy_drift_rel "$summary_line")"
      } >> "$CSV_OUT"
    fi

    rm -f "$tmp_output"
    echo
  done
done
