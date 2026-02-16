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
  --softening-policy <fixed|local-density> (default: fixed)
  --softening-density-scale <scale>  (default: 128.0)
  --seed <rng seed>                 (default: 42)
  --energy-drift <auto|on|off>      (default: auto)
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
SOFTENING_POLICY=fixed
SOFTENING_DENSITY_SCALE=128.0
SEED=42
ENERGY_DRIFT=auto
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
    --softening-policy)
      SOFTENING_POLICY=$2
      shift 2
      ;;
    --softening-density-scale)
      SOFTENING_DENSITY_SCALE=$2
      shift 2
      ;;
    --seed)
      SEED=$2
      shift 2
      ;;
    --energy-drift)
      ENERGY_DRIFT=$2
      shift 2
      ;;
    --energy-drift=*)
      ENERGY_DRIFT=${1#*=}
      shift
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

    CMD=(cargo run --release -- --mode "$MODE" --n "$N" --steps "$STEPS" --dt "$DT" --theta "$theta" --epsilon "$EPSILON" --softening-policy "$SOFTENING_POLICY" --softening-density-scale "$SOFTENING_DENSITY_SCALE" --seed "$SEED" --energy-drift "$ENERGY_DRIFT")

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
          echo "mode,n,steps,dt,theta,theta_policy,theta_density_scale,softening_policy,softening_density_scale,epsilon,g,threads,integrator,init,mass_profile,init_radius,init_spread,init_v_amp,init_lambda,init_center_x,init_center_y,mass_mean,mass_stddev,mass_min,mass_max,mass_alpha,seed,build_ms,force_ms,integrate_ms,total_ms,avg_step_ms,steps_per_sec,ns_per_particle_force,peak_nodes,node_capacity,node_utilization,workspace_bytes,bytes_per_particle,particle_bytes,node_bytes,stack_bytes,initial_ke,initial_pe,initial_te,initial_sampled_pairs,final_ke,final_pe,final_te,final_sampled_pairs,energy_drift_abs,energy_drift_rel,validate_force_rms,validate_force_max,validate_energy_abs,validate_energy_rel,p0_x,p0_y,p0_mag,lz0,p1_x,p1_y,p1_mag,lz1,dp_x,dp_y,dp_mag,dp_lz"
        } > "$CSV_OUT"
      fi

      {
        echo "$(field mode "$summary_line"),$(field n "$summary_line"),$(field steps "$summary_line"),$(field dt "$summary_line"),$(field theta "$summary_line"),$(field theta_policy "$summary_line"),$(field theta_density_scale "$summary_line"),$(field softening_policy "$summary_line"),$(field softening_density_scale "$summary_line"),$(field epsilon "$summary_line"),$(field g "$summary_line"),$(field threads "$summary_line"),$(field integrator "$summary_line"),$(field init "$summary_line"),$(field mass_profile "$summary_line"),$(field init_radius "$summary_line"),$(field init_spread "$summary_line"),$(field init_v_amp "$summary_line"),$(field init_lambda "$summary_line"),$(field init_center_x "$summary_line"),$(field init_center_y "$summary_line"),$(field mass_mean "$summary_line"),$(field mass_stddev "$summary_line"),$(field mass_min "$summary_line"),$(field mass_max "$summary_line"),$(field mass_alpha "$summary_line"),${SEED},$(field build_ms "$summary_line"),$(field force_ms "$summary_line"),$(field integrate_ms "$summary_line"),$(field total_ms "$summary_line"),$(field avg_step_ms "$summary_line"),$(field steps_per_sec "$summary_line"),$(field ns_per_particle_force "$summary_line"),$(field peak_nodes "$summary_line"),$(field node_capacity "$summary_line"),$(field node_utilization "$summary_line"),$(field workspace_bytes "$summary_line"),$(field bytes_per_particle "$summary_line"),$(field particle_bytes "$summary_line"),$(field node_bytes "$summary_line"),$(field stack_bytes "$summary_line"),$(field initial_ke "$summary_line"),$(field initial_pe "$summary_line"),$(field initial_te "$summary_line"),$(field sampled_pairs "$summary_line"),$(field final_ke "$summary_line"),$(field final_pe "$summary_line"),$(field final_te "$summary_line"),$(field final_sampled_pairs "$summary_line"),$(field energy_drift_abs "$summary_line"),$(field energy_drift_rel "$summary_line"),$(field validate_force_rms "$summary_line"),$(field validate_force_max "$summary_line"),$(field validate_energy_abs "$summary_line"),$(field validate_energy_rel "$summary_line"),$(field p0_x "$summary_line"),$(field p0_y "$summary_line"),$(field p0_mag "$summary_line"),$(field lz0 "$summary_line"),$(field p1_x "$summary_line"),$(field p1_y "$summary_line"),$(field p1_mag "$summary_line"),$(field lz1 "$summary_line"),$(field dp_x "$summary_line"),$(field dp_y "$summary_line"),$(field dp_mag "$summary_line"),$(field dp_lz "$summary_line")"
      } >> "$CSV_OUT"
    fi

    rm -f "$tmp_output"
    echo
  done
done
