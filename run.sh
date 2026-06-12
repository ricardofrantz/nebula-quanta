#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./run.sh [options]

Purpose:
  Physics-first baseline runner with all physics controls exposed as explicit flags.

Defaults:
  --mode=barnes_hut
  --n=10000
  --steps=800
  --dt=0.0005
  --theta=0.5
  --preset=fast

Physics controls:
  --mode <barnes_hut|direct>
  --n <particle_count>
  --steps <integration_steps>
  --dt <time_step>
  --theta <barnes_hut_opening_angle>
  --theta-policy <fixed|local-density>
  --theta-density-scale <scale>
  --softening-policy <fixed|local-density>
  --softening-density-scale <scale>
  --epsilon <softening>
  --g <gravity_constant>
  --integrator <leapfrog|verlet|rk2>
  --init <uniform|gaussian|plummer|disk|rotating-disk|keplerian-disk>
  --init-radius <radius>
  --init-spread <spread>
  --init-v-amp <velocity_amplitude>
  --init-lambda <shape_parameter>
  --init-center-x <x_center>
  --init-center-y <y_center>
  --mass-profile <uniform|lognormal|gaussian|pow-law>
  --mass-mean <mean>
  --mass-stddev <stddev>
  --mass-min <min_mass>
  --mass-max <max_mass>
  --mass-alpha <power_law_exponent>
  --seed <u64>
  --energy-drift <auto|on|off>
  --energy-sample-ratio <0..1>
  --dim <2|3> (3 is planned; 2 ready)
  --validate
  --gif        (generate animated GIF from rendered MP4)
  --max-memory-mib <MiB>   default: 20% of available memory
  --preset <fast|balanced|accurate>
  --threads <thread_count> (performance control, includes Barnes-Hut and preset overrides)
  --frames-dir <dir> --width <px> --height <px> --fps <fps> --every-steps <n>
  --progress-every <steps> (0 disables progress output)
  --color-by <golden|speed|accel|density|mass>[,...]
  --colormap <mode|gold|inferno|viridis|magma|plasma|turbo|blue-red>
  --color-scale <asinh|linear|log>
  --color-min <auto|value> --color-max <auto|value>
  --color-auto <first-p99|first-p95|first-minmax>
  --color-headroom <factor>

Utility:
  --show-config   print all resolved flags without running
  --help|-h       this message
  any additional flags are passed through to `bun run nq`
USAGE
}

MODE="barnes_hut"
N=10000
STEPS=800
DT=0.0005
THETA=0.5
THETA_POLICY="fixed"
THETA_DENSITY_SCALE=128.0
SOFTENING_POLICY="fixed"
SOFTENING_DENSITY_SCALE=128.0
EPSILON=0.01
G=1.0
INTEGRATOR="leapfrog"
INIT="uniform"
INIT_RADIUS=1.0
INIT_SPREAD=1.0
INIT_V_AMP=0.05
INIT_LAMBDA=1.0
INIT_CENTER_X=0.0
INIT_CENTER_Y=0.0
MASS_PROFILE="uniform"
MASS_MEAN=1.0
MASS_STDDEV=0.25
MASS_MIN=0.5
MASS_MAX=2.0
MASS_ALPHA=2.0
SEED=42
ENERGY_DRIFT="auto"
ENERGY_SAMPLE_RATIO=0.0
DIM=2
THREADS=2
MAX_MEMORY_MIB=""
AVAILABLE_MEMORY_MIB=""
MAX_MEMORY_SOURCE=""
VALIDATE=0
GIF=0
PRESET="fast"
FRAMES_DIR="frames"
WIDTH=1920
HEIGHT=1080
FPS=60
EVERY_STEPS=1
PROGRESS_EVERY=0
COLOR_BY="golden"
COLORMAP="mode"
COLOR_SCALE="asinh"
COLOR_MIN="auto"
COLOR_MAX="auto"
COLOR_AUTO="first-p99"
COLOR_HEADROOM=1.5
SHOW_CONFIG=0

declare -a EXTRA_ARGS=()
declare -a MAX_MEMORY_ARG=()
declare -a VALIDATE_ARG=()
declare -a GIF_ARG=()
declare -a RECORD_ARGS=()
declare -a COLOR_ARGS=()

show_config() {
  cat <<CFG
mode=$MODE
n=$N
steps=$STEPS
dt=$DT
theta=$THETA
theta_policy=$THETA_POLICY
theta_density_scale=$THETA_DENSITY_SCALE
softening_policy=$SOFTENING_POLICY
softening_density_scale=$SOFTENING_DENSITY_SCALE
epsilon=$EPSILON
g=$G
integrator=$INTEGRATOR
init=$INIT
init_radius=$INIT_RADIUS
init_spread=$INIT_SPREAD
init_v_amp=$INIT_V_AMP
init_lambda=$INIT_LAMBDA
init_center_x=$INIT_CENTER_X
init_center_y=$INIT_CENTER_Y
mass_profile=$MASS_PROFILE
mass_mean=$MASS_MEAN
mass_stddev=$MASS_STDDEV
mass_min=$MASS_MIN
mass_max=$MASS_MAX
mass_alpha=$MASS_ALPHA
seed=$SEED
energy_drift=$ENERGY_DRIFT
energy_sample_ratio=$ENERGY_SAMPLE_RATIO
dim=$DIM
threads=$THREADS
max_memory_source=${MAX_MEMORY_SOURCE:-unresolved}
available_memory_mib=${AVAILABLE_MEMORY_MIB:-na}
max_memory_mib=${MAX_MEMORY_MIB:-na}
preset=$PRESET
validate=$VALIDATE
gif=$GIF
  frames_dir=$FRAMES_DIR
  width=$WIDTH
  height=$HEIGHT
  fps=$FPS
  every_steps=$EVERY_STEPS
  progress_every=$PROGRESS_EVERY
  color_by=$COLOR_BY
  colormap=$COLORMAP
  color_scale=$COLOR_SCALE
  color_min=$COLOR_MIN
  color_max=$COLOR_MAX
  color_auto=$COLOR_AUTO
  color_headroom=$COLOR_HEADROOM
  extra_args_count=${#EXTRA_ARGS[@]}
CFG
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --help|-h)
      usage
      exit 0
      ;;
    --show-config)
      SHOW_CONFIG=1
      shift
      ;;
    --mode)
      MODE=$2
      shift 2
      ;;
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
    --theta)
      THETA=$2
      shift 2
      ;;
    --theta-policy)
      THETA_POLICY=$2
      shift 2
      ;;
    --theta-density-scale)
      THETA_DENSITY_SCALE=$2
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
    --epsilon)
      EPSILON=$2
      shift 2
      ;;
    --g)
      G=$2
      shift 2
      ;;
    --integrator)
      INTEGRATOR=$2
      shift 2
      ;;
    --init)
      INIT=$2
      shift 2
      ;;
    --init-radius)
      INIT_RADIUS=$2
      shift 2
      ;;
    --init-spread)
      INIT_SPREAD=$2
      shift 2
      ;;
    --init-v-amp)
      INIT_V_AMP=$2
      shift 2
      ;;
    --init-lambda)
      INIT_LAMBDA=$2
      shift 2
      ;;
    --init-center-x)
      INIT_CENTER_X=$2
      shift 2
      ;;
    --init-center-y)
      INIT_CENTER_Y=$2
      shift 2
      ;;
    --mass-profile)
      MASS_PROFILE=$2
      shift 2
      ;;
    --mass-mean)
      MASS_MEAN=$2
      shift 2
      ;;
    --mass-stddev)
      MASS_STDDEV=$2
      shift 2
      ;;
    --mass-min)
      MASS_MIN=$2
      shift 2
      ;;
    --mass-max)
      MASS_MAX=$2
      shift 2
      ;;
    --mass-alpha)
      MASS_ALPHA=$2
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
    --energy-sample-ratio)
      ENERGY_SAMPLE_RATIO=$2
      shift 2
      ;;
    --dim)
      DIM=$2
      shift 2
      ;;
    --threads)
      THREADS=$2
      shift 2
      ;;
    --preset)
      PRESET=$2
      shift 2
      ;;
    --max-memory-mib)
      MAX_MEMORY_MIB=$2
      shift 2
      ;;
    --validate)
      VALIDATE=1
      shift
      ;;
    --gif)
      GIF=1
      shift
      ;;
    --frames-dir)
      FRAMES_DIR=$2
      shift 2
      ;;
    --width)
      WIDTH=$2
      shift 2
      ;;
    --height)
      HEIGHT=$2
      shift 2
      ;;
    --fps)
      FPS=$2
      shift 2
      ;;
    --every-steps)
      EVERY_STEPS=$2
      shift 2
      ;;
    --progress-every)
      PROGRESS_EVERY=$2
      shift 2
      ;;
    --color-by)
      COLOR_BY=$2
      shift 2
      ;;
    --colormap)
      COLORMAP=$2
      shift 2
      ;;
    --color-scale)
      COLOR_SCALE=$2
      shift 2
      ;;
    --color-min)
      COLOR_MIN=$2
      shift 2
      ;;
    --color-max)
      COLOR_MAX=$2
      shift 2
      ;;
    --color-auto)
      COLOR_AUTO=$2
      shift 2
      ;;
    --color-headroom)
      COLOR_HEADROOM=$2
      shift 2
      ;;
    --)
      shift
      while [[ $# -gt 0 ]]; do
        EXTRA_ARGS+=("$1")
        shift
      done
      ;;
    *)
      EXTRA_ARGS+=("$1")
      shift
      ;;
  esac
done

available_memory_mib() {
  local mib=""

  if command -v free >/dev/null 2>&1; then
    mib=$(free -m | awk '/^Mem:/ { if ($7 ~ /^[0-9]+$/) print $7; else if ($2 ~ /^[0-9]+$/) print $2 }')
  fi

  if [[ -z "$mib" && "$(uname -s)" == "Darwin" ]]; then
    mib=$(sysctl -n hw.memsize 2>/dev/null | awk '{ if ($1 ~ /^[0-9]+$/) printf "%d", $1 / 1024 / 1024 }')
  fi

  if [[ -z "$mib" && -r /proc/meminfo ]]; then
    mib=$(awk '
      /^MemAvailable:/ { print int($2 / 1024); exit }
      /^MemTotal:/ { total = $2 }
      END {
        if (total != "") {
          print int(total / 1024)
        }
      }' /proc/meminfo)
  fi

  if [[ "$mib" == "" || "$mib" == *[^0-9]* ]]; then
    printf ""
  else
    printf "%s" "$mib"
  fi
}

if [[ -z "$MAX_MEMORY_MIB" ]]; then
  AVAILABLE_MEMORY_MIB="$(available_memory_mib)"
  if [[ -n "$AVAILABLE_MEMORY_MIB" ]]; then
    MAX_MEMORY_MIB=$(( AVAILABLE_MEMORY_MIB * 20 / 100 ))
    MAX_MEMORY_SOURCE="auto-20percent"
  else
    MAX_MEMORY_SOURCE="unresolved"
    MAX_MEMORY_MIB=""
  fi
else
  MAX_MEMORY_SOURCE="user"
  AVAILABLE_MEMORY_MIB=""
fi

if [[ -n "$MAX_MEMORY_MIB" ]]; then
  MAX_MEMORY_ARG=(--max-memory-mib "$MAX_MEMORY_MIB")
else
  MAX_MEMORY_ARG=()
fi

if [[ "$VALIDATE" -eq 1 ]]; then
  VALIDATE_ARG=(--validate)
else
  VALIDATE_ARG=()
fi

if [[ "$GIF" -eq 1 ]]; then
  GIF_ARG=(--gif)
else
  GIF_ARG=()
fi

RECORD_ARGS=(
  --record
  --frames-dir "$FRAMES_DIR"
  --width "$WIDTH"
  --height "$HEIGHT"
  --fps "$FPS"
  --every-steps "$EVERY_STEPS"
  --progress-every "$PROGRESS_EVERY"
)
COLOR_ARGS=(
  --color-by "$COLOR_BY"
  --colormap "$COLORMAP"
  --color-scale "$COLOR_SCALE"
  --color-min "$COLOR_MIN"
  --color-max "$COLOR_MAX"
  --color-auto "$COLOR_AUTO"
  --color-headroom "$COLOR_HEADROOM"
)

if [[ "$SHOW_CONFIG" -eq 1 ]]; then
  show_config
  exit 0
fi

resolve_cargo_binary() {
  if [[ -n "${CARGO_BIN:-}" && -x "$CARGO_BIN" ]]; then
    printf "%s\n" "$CARGO_BIN"
    return
  fi

  if command -v cargo >/dev/null 2>&1; then
    command -v cargo
    return
  fi

  if [[ -n "${CARGO_HOME:-}" && -x "$CARGO_HOME/bin/cargo" ]]; then
    printf "%s\n" "$CARGO_HOME/bin/cargo"
    return
  fi

  if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    printf "%s\n" "$HOME/.cargo/bin/cargo"
    return
  fi

  return 1
}

if ! CARGO_BIN="$(resolve_cargo_binary)"; then
  echo "error: cargo executable not found."
  echo "supply one of:"
  echo "  - CARGO_BIN=/path/to/cargo ./run.sh"
  echo "  - source \$HOME/.cargo/env before running"
  echo "  - install Rust via rustup (https://rustup.rs)"
  exit 1
fi

export CARGO_BIN
export PATH="$(dirname "$CARGO_BIN"):$PATH"

bun run nq -- \
  --mode "$MODE" \
  --preset "$PRESET" \
  --n "$N" \
  --steps "$STEPS" \
  --dt "$DT" \
  --theta "$THETA" \
  --theta-policy "$THETA_POLICY" \
  --theta-density-scale "$THETA_DENSITY_SCALE" \
  --softening-policy "$SOFTENING_POLICY" \
  --softening-density-scale "$SOFTENING_DENSITY_SCALE" \
  --epsilon "$EPSILON" \
  --g "$G" \
  --integrator "$INTEGRATOR" \
  --init "$INIT" \
  --init-radius "$INIT_RADIUS" \
  --init-spread "$INIT_SPREAD" \
  --init-v-amp "$INIT_V_AMP" \
  --init-lambda "$INIT_LAMBDA" \
  --init-center-x "$INIT_CENTER_X" \
  --init-center-y "$INIT_CENTER_Y" \
  --mass-profile "$MASS_PROFILE" \
  --mass-mean "$MASS_MEAN" \
  --mass-stddev "$MASS_STDDEV" \
  --mass-min "$MASS_MIN" \
  --mass-max "$MASS_MAX" \
  --mass-alpha "$MASS_ALPHA" \
  --seed "$SEED" \
  --energy-drift "$ENERGY_DRIFT" \
  --energy-sample-ratio "$ENERGY_SAMPLE_RATIO" \
  --dim "$DIM" \
  --threads "$THREADS" \
  "${MAX_MEMORY_ARG[@]+"${MAX_MEMORY_ARG[@]}"}" \
  "${VALIDATE_ARG[@]+"${VALIDATE_ARG[@]}"}" \
  "${RECORD_ARGS[@]+"${RECORD_ARGS[@]}"}" \
  "${COLOR_ARGS[@]+"${COLOR_ARGS[@]}"}" \
  "${GIF_ARG[@]+"${GIF_ARG[@]}"}" \
  "${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"}"
