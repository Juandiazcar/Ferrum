#!/usr/bin/env bash
# Throughput only (S1 parse+encode lines/s, no dhat), allocators INTERLEAVED round by round so that
# machine drift (thermal state, background load, neighbours on shared CI runners) hits every
# allocator equally. Added because three back-to-back runs per allocator (run-all.sh) showed
# run-to-run noise larger than the 10 % margin of docs/03 §7 on Windows and macOS (ADR 0016).
# Output: one "THROUGHPUT ..." line per process. Usage (repo root):
#   bash spikes/s2-allocator/run-throughput.sh [rounds]
set -euo pipefail
cd "$(dirname "$0")"
rounds=${1:-10}
case "$(uname -s)" in
  Linux*) os=linux ;;
  Darwin*) os=macos ;;
  MINGW* | MSYS* | CYGWIN*) os=windows ;;
  *) echo "unsupported OS" >&2; exit 1 ;;
esac
exe=ferrum-spike-s2
[ "$os" = windows ] && exe=$exe.exe

# "name|cargo features|extra env" (same candidates as run-all.sh; no THP variant needed here).
configs=("system||")
[ "$os" = linux ] && configs+=("system-arena1||MALLOC_ARENA_MAX=1")
configs+=("mimalloc|mimalloc|")
[ "$os" != windows ] && configs+=("jemalloc|jemalloc|")

for cfg in "${configs[@]}"; do
  IFS='|' read -r name features _ <<< "$cfg"
  [ "$name" = system-arena1 ] && continue # same binary as system
  cargo build --release --target-dir "target/$name" ${features:+--features "$features"} >&2
done

for round in $(seq 1 "$rounds"); do
  for cfg in "${configs[@]}"; do
    IFS='|' read -r name _ envs <<< "$cfg"
    tdir=target/$name
    [ "$name" = system-arena1 ] && tdir=target/system
    # shellcheck disable=SC2086
    lps=$(env $envs "$tdir/release/$exe" --idle-secs 0 --hold-secs 0 | awk '/^throughput.lines_per_s/ { print $2 }')
    echo "THROUGHPUT os=$os allocator=$name round=$round lines_per_s=$lps"
  done
done
