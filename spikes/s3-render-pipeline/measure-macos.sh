#!/usr/bin/env bash
# Spike S3 external memory sampler for macOS (the spike has no unsafe code, docs/05 §4).
# Usage: measure-macos.sh <exe> [args...]. On every "RESULT phase label=L pid=N" line it samples N
# with `footprint` (phys_footprint, the Activity Monitor metric of docs/03 §1) and `ps` (RSS).
"$@" | while IFS= read -r line; do
  echo "$line"
  case "$line" in
    "RESULT phase label="*)
      label="${line#*label=}"; label="${label%% *}"
      pid="${line##*pid=}"
      fp="$((footprint -p "$pid" --format bytes 2>/dev/null || footprint -p "$pid" 2>&1) | grep -i -m1 'footprint:' | tr -s ' ')"
      rss="$(ps -o rss= -p "$pid" | tr -d ' ')"
      echo "RESULT mem phase=$label footprint_raw=[$fp] rss_kb=$rss"
      ;;
  esac
done
