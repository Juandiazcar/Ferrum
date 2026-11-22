#!/usr/bin/env bash
# Reproduces every number in docs/adr/0016. One build per allocator (cargo feature), N runs each
# (default 3), one process per run. Each process fills 10 sessions, idles 10 s, prints
# "STEADY pid=<pid>"; this script then samples the OS memory metric EXTERNALLY (docs/03 §1):
#   Linux   PSS from /proc/<pid>/smaps_rollup
#   macOS   phys_footprint via `footprint <pid>`
#   Windows Private Working Set via probe-windows.ps1 (PDH 'Working Set - Private')
# Throughput (S1 parse+encode lines/s, no dhat) is printed by the process itself.
# Output: one "RESULT ..." line per run. Usage (repo root): bash spikes/s2-allocator/run-all.sh [runs]
set -euo pipefail
cd "$(dirname "$0")"
runs=${1:-3}
case "$(uname -s)" in
  Linux*) os=linux ;;
  Darwin*) os=macos ;;
  MINGW* | MSYS* | CYGWIN*) os=windows ;;
  *) echo "unsupported OS" >&2; exit 1 ;;
esac
exe=ferrum-spike-s2
[ "$os" = windows ] && exe=$exe.exe
res=target/results
rm -rf "$res" && mkdir -p "$res"

# "name|cargo features|extra env". jemalloc is skipped on Windows MSVC (unsupported by the crate).
configs=("system||")
[ "$os" = linux ] && configs+=("system-arena1||MALLOC_ARENA_MAX=1")
configs+=("mimalloc|mimalloc|")
# Variant: mimalloc 2.3.2 (bundled) with transparent huge pages disabled by its own env option.
[ "$os" = linux ] && configs+=("mimalloc-nothp|mimalloc|MIMALLOC_ALLOW_THP=0")
if [ "$os" != windows ]; then configs+=("jemalloc|jemalloc|"); fi

# Prints the metric in bytes for <pid>; raw tool output is kept in $res for the record.
sample() {
  local pid=$1 tag=$2
  case "$os" in
    linux)
      cp "/proc/$pid/smaps_rollup" "$res/$tag.smaps_rollup"
      awk '/^Pss:/ { print $2 * 1024 }' "/proc/$pid/smaps_rollup" ;;
    macos)
      footprint "$pid" > "$res/$tag.footprint.txt" 2>&1
      # "phys_footprint" total line, e.g. "Footprint: 12 MB"; value and unit converted to bytes.
      awk '/Footprint:/ && !d { v = $(NF-4); u = $(NF-3);
             m = (u == "KB") ? 1024 : (u == "MB") ? 1048576 : (u == "GB") ? 1073741824 : 1;
             printf "%d\n", v * m; d = 1 }' "$res/$tag.footprint.txt" ;;
    windows)
      powershell.exe -NoProfile -ExecutionPolicy Bypass -File probe-windows.ps1 -ProcId "$pid" \
        | tee "$res/$tag.probe.txt" | awk '{ print $1 }' ;;
  esac
}

stop() { # kill the sampled process
  if [ "$os" = windows ]; then taskkill //PID "$1" //F > /dev/null 2>&1 || true; else kill "$1" 2> /dev/null || true; fi
}

for cfg in "${configs[@]}"; do
  IFS='|' read -r name features envs <<< "$cfg"
  tdir=target/$name
  [ "$name" = system-arena1 ] && tdir=target/system # same binary, env var differs
  [ "$name" = mimalloc-nothp ] && tdir=target/mimalloc
  cargo build --release --target-dir "$tdir" ${features:+--features "$features"} >&2
  for run in $(seq 1 "$runs"); do
    tag=$name.$run
    # shellcheck disable=SC2086
    env $envs "$tdir/release/$exe" --idle-secs 10 --hold-secs 120 > "$res/$tag.out" &
    for _ in $(seq 1 600); do grep -q STEADY "$res/$tag.out" 2> /dev/null && break; sleep 0.5; done
    pid=$(grep -o 'pid=[0-9]*' "$res/$tag.out" | cut -d= -f2)
    mem=$(sample "$pid" "$tag")
    lps=$(awk '/^throughput.lines_per_s/ { print $2 }' "$res/$tag.out")
    stop "$pid"
    wait 2> /dev/null || true
    echo "RESULT os=$os allocator=$name run=$run metric_bytes=$mem lines_per_s=$lps"
  done
done
