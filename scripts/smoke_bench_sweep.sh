#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT_DIR/target/release/nq"

if [[ ! -x "$BIN" ]]; then
  echo "error: expected release binary at $BIN; run cargo build --release first" >&2
  exit 1
fi

output="$($BIN \
  --mode barnes_hut \
  --n 2000 \
  --steps 10 \
  --seed 42 \
  --threads 1 \
  --energy-drift off)"

printf '%s\n' "$output"

for field in 'mode=barnes_hut' 'n=2000' 'steps=10' 'total_ms=' 'steps_per_sec='; do
  if ! grep -q "$field" <<<"$output"; then
    echo "error: smoke output missing required field: $field" >&2
    exit 1
  fi
done
