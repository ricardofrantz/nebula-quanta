#!/usr/bin/env bash

set -euo pipefail

tmp_csv="$(mktemp)"
tmp_output="$(mktemp)"
frame_dir="./.smoke_frames"
trap 'rm -f "$tmp_csv" "$tmp_output"; rm -rf "$frame_dir"' EXIT

EXPECTED_HEADER="mode,n,steps,dt,theta,theta_policy,theta_density_scale,softening_policy,softening_density_scale,epsilon,g,threads,integrator,init,mass_profile,init_radius,init_spread,init_v_amp,init_lambda,init_center_x,init_center_y,mass_mean,mass_stddev,mass_min,mass_max,mass_alpha,seed,build_ms,force_ms,integrate_ms,total_ms,avg_step_ms,steps_per_sec,ns_per_particle_force,peak_nodes,node_capacity,node_utilization,workspace_bytes,bytes_per_particle,particle_bytes,node_bytes,stack_bytes,initial_ke,initial_pe,initial_te,initial_sampled_pairs,final_ke,final_pe,final_te,final_sampled_pairs,energy_drift_abs,energy_drift_rel,validate_force_rms,validate_force_max,validate_energy_abs,validate_energy_rel,p0_x,p0_y,p0_mag,lz0,p1_x,p1_y,p1_mag,lz1,dp_x,dp_y,dp_mag,dp_lz"

if ! rg -Fq "$EXPECTED_HEADER" README.md; then
  echo "README contract mismatch: expected CSV header not found"
  echo "Update README columns list if API changed."
  exit 1
fi

field_from_csv() {
  local csv_file="$1"
  local key="$2"
  awk -F',' -v key="$key" '
    NR==1 {
      idx = 0
      for (i = 1; i <= NF; i++) {
        if ($i == key) {
          idx = i
        }
      }
    }
    NR==2 {
      if (idx == 0) {
        exit 1
      }
      print $idx
      exit 0
    }
  ' "$csv_file"
}

run_bench_case() {
  local case_name="$1"
  local mode="$2"
  local threads="$3"
  local validate="$4"
  local theta="$5"
  local steps="$6"

  rm -f "$tmp_csv"
  rm -f "$tmp_output"
  local -a cmd=(
    bash scripts/bench_sweep_deterministic.sh
    --n 16
    --steps "$steps"
    --dt 0.001
    --epsilon 0.01
    --seed 42
    --theta "$theta"
    --threads "$threads"
    --mode "$mode"
    --energy-drift on
    --csv "$tmp_csv"
  )

  if [[ "$validate" == "1" ]]; then
    cmd+=(--validate)
  fi

  echo "===== bench-smoke: $case_name mode=$mode threads=$threads theta=$theta steps=$steps validate=$validate ====="
  if ! "${cmd[@]}" | tee "$tmp_output"; then
    echo "bench smoke run failed: $case_name" >&2
    exit 1
  fi

  if ! head -n 1 "$tmp_csv" | rg -q "^$EXPECTED_HEADER$"; then
    echo "unexpected CSV header format"
    echo "case: $case_name"
    cat "$tmp_csv"
    exit 1
  fi

  if ! awk -F',' 'END { exit (NR != 2) }' "$tmp_csv"; then
    echo "smoke output should contain exactly header + one row"
    echo "case: $case_name"
    cat "$tmp_csv"
    exit 1
  fi

  if ! rg -q '^validate=ok|^mode=' "$tmp_output"; then
    echo "simulation output missing expected summary lines"
    echo "case: $case_name"
    cat "$tmp_output"
    exit 1
  fi

  if [[ "$validate" == "1" ]]; then
    local force_rms
    local force_max
    force_rms=$(field_from_csv "$tmp_csv" validate_force_rms)
    force_max=$(field_from_csv "$tmp_csv" validate_force_max)
    if [[ "$force_rms" == "" || "$force_rms" == "na" ]]; then
      echo "validation missing validate_force_rms for $case_name" >&2
      cat "$tmp_csv"
      exit 1
    fi
    if [[ "$force_max" == "" || "$force_max" == "na" ]]; then
      echo "validation missing validate_force_max for $case_name" >&2
      cat "$tmp_csv"
      exit 1
    fi
  fi

}

run_record_case() {
  local steps=12
  local every_steps=2
  local fps=30
  local expected_frames=$((1 + steps / every_steps))

  rm -rf "$frame_dir"
  mkdir -p "$frame_dir"
  rm -f "$tmp_output"

  echo "===== run-smoke: record mode long-run capture ====="
  if ! cargo run --release -- \
    --mode barnes_hut \
    --n 16 \
    --steps "$steps" \
    --dt 0.001 \
    --epsilon 0.01 \
    --theta 0.6 \
    --record \
    --frames-dir "$frame_dir" \
    --width 96 \
    --height 64 \
    --fps "$fps" \
    --every-steps "$every_steps" \
    --seed 42 \
    | tee "$tmp_output"; then
    echo "record smoke run failed" >&2
    exit 1
  fi

  if ! rg -q '^record_frames=' "$tmp_output"; then
    echo "record run did not print record_frames marker" >&2
    exit 1
  fi

  local line
  line=$(rg '^record_frames=' "$tmp_output" | tail -n 1)
  local record_frames
  record_frames=$(echo "$line" | sed -E 's/.*record_frames=([0-9]+).*/\1/')
  if [[ "$record_frames" != "$expected_frames" ]]; then
    echo "unexpected record frame count. expected=$expected_frames got=$record_frames" >&2
    echo "line=$line"
    exit 1
  fi

  local rendered
  rendered=$(echo "$line" | sed -E 's/.*render_cmd="([^"]+)".*/\1/' )
  if [[ "$rendered" != *"ffmpeg -y -framerate $fps -i $frame_dir/frame_%06d.ppm"* ]]; then
    echo "unexpected render command"
    echo "line=$line"
    exit 1
  fi

  local frame_count
  frame_count=$(rg --files "$frame_dir" | rg 'frame_[0-9]{6}\.ppm$' | wc -l | tr -d ' ')
  if [[ "$frame_count" == "0" ]]; then
    echo "record run did not produce PPM frames"
    exit 1
  fi
  if [[ "$frame_count" != "$record_frames" ]]; then
    echo "frame files and printed record count differ"
    echo "recorded=$record_frames files=$frame_count"
    exit 1
  fi
}

echo "Running benchmark smoke test matrix..."

run_bench_case "direct-baseline" direct 1 0 0.5 2
run_bench_case "barnes-hut-single" barnes_hut 1 0 0.65 2
run_bench_case "barnes-hut-threaded-validate" barnes_hut 2 1 0.65 2
run_record_case

echo "Smoke matrix passed: direct + barnes-hut + threaded + record capture"
