#!/usr/bin/env bash
# Reproduces every number in docs/adr/0024: 3 seeds per configuration, one process per run.
# Usage (repo root): bash spikes/s1-memory-model/run-all.sh > s1-results.txt
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --release --manifest-path spikes/s1-memory-model/Cargo.toml
bin=spikes/s1-memory-model/target/release/ferrum-spike-s1
for seed in 1 2 3; do
  for profile in mixed corpus entropy; do
    for step in 0 16384; do
      echo "=== model=ferrum profile=$profile seed=$seed hot_step=$step"
      "$bin" --model ferrum --profile "$profile" --seed "$seed" --hot-step "$step" --corpus-dir docs/spec
    done
  done
  echo "=== model=naive profile=mixed seed=$seed"
  "$bin" --model naive --seed "$seed"
  echo "=== model=alacritty profile=mixed seed=$seed"
  "$bin" --model alacritty --seed "$seed"
done
