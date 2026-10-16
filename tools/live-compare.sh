#!/usr/bin/env bash
# The live comparison: one pad file played into the real AC1 and into ac1-rs at the same time, then lined up.
#
#   tools/live-compare.sh <pad file> [out dir]
#
# Needs: the real AC1 running with ac1-hook (`AC1 windowed.bat`) and loaded into the city the pad file starts in.
# 1. The real player is put at the file's start (the hook's `tp`, which sets the ground's own heading too) with the live
#    log on, and its camera's direction read.
# 2. The file's left stick is turned into that camera's frame (AC1_STICK_FROM_FACING=1, the default), so pushing it
#    forward runs the way the start faces in both games, whichever way the real camera looks; ours' camera starts at
#    the real one's yaw (the go file).
# 3. ac1-rs is started (the same city, windowed beside it, `AC1_PADPLAY` waiting for the go file); then the real game
#    gets `live on` + `padplay` and ours the go file in the same moment.
# 4. When it has played out: the real game's live log and ours' recording in the out dir, and `livelog compare` of
#    the two (compare.txt). AC1_GAME (the install), AC1_LEVEL (default damascus), AC1_EXE (target/release/ac1.exe).
set -euo pipefail
src=$(realpath "$1")
out=$(realpath -m "${2:-out/live-compare/$(basename "$src" .pad)}")
game=${AC1_GAME:-/p/SteamLibrary/steamapps/common/Assassins Creed}
exe=${AC1_EXE:-target/release/ac1.exe}
mkdir -p "$out"
win() { cygpath -w "$1" 2>/dev/null || echo "$1"; }

secs=$(awk '$1 ~ /^[0-9.]+$/ && NF == 6 { s += $1 } END { printf "%.1f", s }' "$src")
echo "pad file: $src, $secs s"

# The real game must be up and drawing (its status file moving).
s1=$(cat "$game/ac1-hook.status" 2>/dev/null || true); sleep 2; s2=$(cat "$game/ac1-hook.status" 2>/dev/null || true)
[[ -n "$s2" && "$s1" != "$s2" ]] || { echo "the real AC1 is not running with ac1-hook"; exit 1; }

# 1. The real player at the start; its camera's yaw (ours' convention: forward = (-sin, -cos)).
start=$(awk '$1 == "start" { print $2, $3, $4, $5; exit }' "$src")
camyaw=""
# (On the ground first: a teleport keeps the context, so a run that ended hanging would start the next one hanging.
# Let go with the empty hand, then put him down at the start and look again.)
context() {
  local n; n=$(wc -l < "$game/ac1-hook.log")
  printf 'where\n' > "$game/ac1-hook.cmd"; sleep 1
  tail -n +$((n + 1)) "$game/ac1-hook.log" | grep -o 'in [A-Za-z]*$' | tail -1 | cut -c4-
}
for _ in 1 2 3 4; do
  c=$(context)
  [[ "$c" == "HumanGround" ]] && break
  echo "the real player is in ${c:-?}: letting go"
  [[ -n "$start" ]] && printf 'tp %s\n' "$start" > "$game/ac1-hook.cmd" && sleep 0.5
  printf 'press hand 10\n' > "$game/ac1-hook.cmd"; sleep 2.5
done
if [[ -n "$start" ]]; then
  rm -f "$game/ac1-live.txt" "$game/ac1-live-events.txt"
  printf 'live on\ntp %s\n' "$start" > "$game/ac1-hook.cmd"
  sleep 2
  camyaw=$(grep -o 'camdir=[-0-9.]*,[-0-9.]*,[-0-9.]*' "$game/ac1-live.txt" | tail -1 |
    awk -F'[=,]' '{ printf "%.5f", atan2(-$2, -$4) }')
  printf 'live off\n' > "$game/ac1-hook.cmd"
  sleep 0.5
  echo "the real camera's yaw at the start: ${camyaw:-unknown} rad"
  printf '%s
' "$camyaw" > "$out/camyaw.txt"
fi

# 2. The stick in the real camera's frame.
pad="$src"
if [[ -n "$camyaw" && -n "$start" && "${AC1_STICK_FROM_FACING:-1}" == 1 ]]; then
  python -I - "$src" "$out/played.pad" "$camyaw" <<'PY'
import math, sys
src, dst, cam = sys.argv[1], sys.argv[2], float(sys.argv[3])
face = None
out = []
for line in open(src):
    w = line.split()
    if w and w[0] == "start":
        face = math.radians(float(w[4]) - 90.0)  # ours' turn about Y for the game yaw
    elif len(w) == 6 and face is not None and w[0][0].isdigit():
        lx, ly = float(w[2]), float(w[3])
        f = (-math.sin(face), -math.cos(face)); r = (math.cos(face), -math.sin(face))
        wx, wz = f[0] * ly + r[0] * lx, f[1] * ly + r[1] * lx
        fc = (-math.sin(cam), -math.cos(cam)); rc = (math.cos(cam), -math.sin(cam))
        w[2], w[3] = f"{wx * rc[0] + wz * rc[1]:.4f}", f"{wx * fc[0] + wz * fc[1]:.4f}"
        line = " ".join(w) + "\n"
    out.append(line)
open(dst, "w").write("".join(out))
PY
  pad=$(realpath "$out/played.pad")
  echo "the stick turned into the real camera's frame: $pad"
fi

# 3. ac1-rs, waiting for the go file.
go="$out/go"
rm -f "$go" "$out/ours.txt" "$out/ours-log.txt"
AC1_LEVEL=${AC1_LEVEL:-damascus} AC1_BACKGROUND=1 AC1_NO_CROWD=1 \
  AC1_PADPLAY="$(win "$pad")" AC1_PADPLAY_GO="$(win "$go")" AC1_PADPLAY_RECORD="$(win "$out/ours.txt")" AC1_PADPLAY_QUIT=1 \
  RUST_LOG=ac1=debug,wgpu=error NO_COLOR=1 "$exe" > "$out/ours-log.txt" 2>&1 &
ours=$!
echo "ac1-rs started (pid $ours), waiting for its city"
for _ in $(seq 1 240); do
  grep -q "ready in" "$out/ours-log.txt" 2>/dev/null && break
  sleep 1
done
sleep 5

# Both at once.
rm -f "$game/ac1-live.txt" "$game/ac1-live-events.txt"
printf 'live on\npadplay %s\n' "$(win "$pad")" > "$game/ac1-hook.cmd"
printf '%s\n' "$camyaw" > "$go"
echo "playing in both"
sleep "$(awk -v s="$secs" 'BEGIN { print s + 2 }')"
printf 'live off\npad off\nwhere\n' > "$game/ac1-hook.cmd"
sleep 2
wait "$ours" || true

# 4. The two lined up.
cp "$game/ac1-live.txt" "$out/real.txt"
cp "$game/ac1-live-events.txt" "$out/real-events.txt" 2>/dev/null || true
cargo run -q --release -p ac1 --example livelog -- compare "$out/real.txt" "$out/ours.txt" > "$out/compare.txt"
echo "done: $out (real.txt, ours.txt, compare.txt)"
grep -A40 "mode switches" "$out/compare.txt" | head -12
