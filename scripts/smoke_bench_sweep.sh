#!/usr/bin/env bash

set -euo pipefail

tmp_csv="$(mktemp)"
trap 'rm -f "$tmp_csv"' EXIT

echo "Running benchmark CSV smoke test..."

bash scripts/bench_sweep.sh \
  --n 16 \
  --steps 2 \
  --dt 0.001 \
  --epsilon 0.01 \
  --theta 0.5 \
  --threads 1 \
  --mode direct \
  --csv "$tmp_csv"

if ! head -n 1 "$tmp_csv" | grep -q "^theta,threads,mode,n,steps,dt,theta_value,epsilon,build_ms,force_ms,integrate_ms,total_ms,avg_step_ms,steps_per_sec,ns_per_particle_force,peak_nodes,node_capacity,node_utilization,workspace_bytes,bytes_per_particle,particle_bytes,node_bytes,stack_bytes$"; then
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
