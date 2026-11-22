#!/usr/bin/env bash
# Spike S3 driver. Usage: run.sh <font.ttf> [runs=3]   (env RUN_DIALOG=1: also open the folder dialog, Windows only)
# Builds the four variants (base, skia, dialog, both), prints binary sizes, then runs each
# measurement `runs` times. Needs a display (Xvfb, Wayland compositor, or a desktop session).
# Memory: Linux reads PSS in-process; Windows/macOS go through the external samplers.
set -u
FONT="${1:?font path}"
RUNS="${2:-3}"
cd "$(dirname "$0")" || exit 1
EXE=""
OS="$(uname -s)"
case "$OS" in MINGW* | MSYS* | CYGWIN*) EXE=".exe"; FONT="$(cygpath -w "$FONT")" ;; esac

# memrun <exe> <args...>: run a mem-mode process under the right sampler.
memrun() {
  case "$OS" in
    MINGW* | MSYS* | CYGWIN*)
      exe="$1"; shift
      powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w "$PWD/measure-windows.ps1")" -Exe "$(cygpath -w "$PWD/$exe")" -ArgLine "$*" ;;
    Darwin) bash measure-macos.sh "$@" ;;
    *) "$@" ;;
  esac
}

mkdir -p target/variants
for v in "base:" "skia:skia" "dialog:dialog" "both:skia,dialog"; do
  n="${v%%:*}"
  f="${v#*:}"
  cargo build --release --locked ${f:+--features "$f"} >target/build.log 2>&1 || { echo "build failed: $n"; tail -40 target/build.log; exit 1; }
  cp "target/release/ferrum-spike-s3$EXE" "target/variants/s3-$n$EXE"
  echo "RESULT binary_size variant=$n bytes=$(wc -c <"target/variants/s3-$n$EXE" | tr -d ' ')"
done
B=target/variants/s3-both$EXE
MEMGREP='RESULT (env|window|font|mem|present|skia_tiles|glyph|dialog)'
for i in $(seq 1 "$RUNS"); do
  echo "=== run $i: repaint 1440x960 physical, 200x60 ==="
  "$B" --mode repaint --size 1440x960 --rows 60 --physical --font "$FONT" | grep -E 'RESULT (env|window|font|repaint_setup|full_|damage_|glyph_)'
done
for i in $(seq 1 "$RUNS"); do
  for v in base dialog skia both; do
    extra=""
    if [ "$v" = skia ] || [ "$v" = both ]; then extra="--skia"; fi
    echo "=== run $i: mem 1440x900 physical, variant=$v ==="
    memrun "target/variants/s3-$v$EXE" --mode mem --size 1440x900 --rows 55 --physical --font "$FONT" --idle-secs 5 $extra | grep -E "$MEMGREP"
  done
done
for i in $(seq 1 "$RUNS"); do
  # Pixel-count equivalent of 1440x900 @2x (needs a display at least this large).
  echo "=== run $i: mem 2880x1800 physical (HiDPI-equivalent), variant=base ==="
  memrun "target/variants/s3-base$EXE" --mode mem --size 2880x1800 --rows 55 --physical --font "$FONT" --idle-secs 5 | grep -E "$MEMGREP"
done
if [ "${RUN_DIALOG:-0}" = 1 ]; then
  for i in $(seq 1 "$RUNS"); do
    echo "=== run $i: mem 1440x900 physical, variant=dialog, folder dialog opened then closed ==="
    memrun "target/variants/s3-dialog$EXE" --mode mem --size 1440x900 --rows 55 --physical --font "$FONT" --idle-secs 5 --dialog | grep -E "$MEMGREP"
  done
fi
for i in $(seq 1 "$RUNS"); do
  echo "=== run $i: tiny-skia primitives ==="
  "$B" --mode skia | grep -E 'RESULT skia'
done
