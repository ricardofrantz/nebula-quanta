#!/usr/bin/env bash

set -euo pipefail

tmp_csv="$(mktemp)"
trap 'rm -f "$tmp_csv"' EXIT

echo "Running benchmark CSV smoke test..."

bash scripts/bench_sweep_deterministic.sh \
  --n 16 \
  --steps 2 \
  --dt 0.001 \
  --epsilon 0.01 \
  --theta 0.5 \
  --threads 1 \
  --mode direct \
  --seed 42 \
  --csv "$tmp_csv"

if ! head -n 1 "$tmp_csv" | grep -q "^mode,n,steps,dt,theta,epsilon,g,threads,integrator,init,mass_profile,init_radius,init_spread,init_v_amp,init_lambda,init_center_x,init_center_y,mass_mean,mass_stddev,mass_min,mass_max,mass_alpha,seed,build_ms,force_ms,integrate_ms,total_ms,avg_step_ms,steps_per_sec,ns_per_particle_force,peak_nodes,node_capacity,node_utilization,workspace_bytes,bytes_per_particle,particle_bytes,node_bytes,stack_bytes,initial_ke,initial_pe,initial_te,initial_sampled_pairs,final_ke,final_pe,final_te,final_sampled_pairs,energy_drift_abs,energy_drift_rel,p0_x,p0_y,p0_mag,lz0,p1_x,p1_y,p1_mag,lz1,dp_x,dp_y,dp_mag,dp_lz$"; then
  echo "unexpected CSV header format"
  cat "$tmp_csv"
  exit 1
fi

if ! awk -F',' 'NR==2 { exit (NF<1 || $1=="" || $3=="" || $NF=="") }' "$tmp_csv"; then
  echo "invalid benchmark CSV row"
  cat "$tmp_csv"
  exit 1
fi

if ! awk -F',' 'END { exit (NR != 2) }' "$tmp_csv"; then
  echo "smoke output should contain exactly header + one row"
  cat "$tmp_csv"
  exit 1
fi

echo "CSV smoke test passed: $(wc -l < "$tmp_csv") line(s) in $tmp_csv"
