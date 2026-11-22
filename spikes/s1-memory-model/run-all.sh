#!/usr/bin/env bash
# Reproduces every number in docs/adr/0024: 3 seeds per configuration, one process per run.
# Memory figures come from the default build (dhat heap accounting). Throughput figures come
# from a second build without dhat (`--no-default-features`), because dhat records a backtrace
# per allocation and distorts timing; lines named "=== throughput ..." are the ones to read.
# Usage (repo root): bash spikes/s1-memory-model/run-all.sh > s1-results.txt
set -euo pipefail
cd "$(dirname "$0")/../.."
manifest=spikes/s1-memory-model/Cargo.toml
cargo build --release --manifest-path "$manifest"
cargo build --release --manifest-path "$manifest" --no-default-features \
  --target-dir spikes/s1-memory-model/target/nodhat
# The prose corpus is the spec markdown as of the base commit, so later spec edits do not move it.
corpus=spikes/s1-memory-model/target/corpus
rm -rf "$corpus"
mkdir -p "$corpus"
git archive e4a3e52 docs/spec | tar -x -C "$corpus"
bin=spikes/s1-memory-model/target/release/ferrum-spike-s1
fast=spikes/s1-memory-model/target/nodhat/release/ferrum-spike-s1
for seed in 1 2 3; do
  for profile in mixed corpus entropy; do
    for step in 0 16384; do
      echo "=== model=ferrum profile=$profile seed=$seed hot_step=$step"
      "$bin" --model ferrum --profile "$profile" --seed "$seed" --hot-step "$step" --corpus-dir "$corpus/docs/spec"
    done
  done
  echo "=== model=naive profile=mixed seed=$seed"
  "$bin" --model naive --seed "$seed"
  echo "=== model=alacritty profile=mixed seed=$seed"
  "$bin" --model alacritty --seed "$seed"
  for step in 0 16384; do
    echo "=== throughput model=ferrum profile=mixed seed=$seed hot_step=$step"
    "$fast" --model ferrum --profile mixed --seed "$seed" --hot-step "$step"
  done
  echo "=== throughput model=naive profile=mixed seed=$seed"
  "$fast" --model naive --seed "$seed"
  echo "=== throughput model=alacritty profile=mixed seed=$seed"
  "$fast" --model alacritty --seed "$seed"
done
