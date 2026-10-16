#!/usr/bin/env bash
# The live comparison: one pad file played into the real AC1 and into ac1-rs at the same time, then lined up.
#
#   tools/live-compare.sh <pad file> [out dir]
#
# Needs: the real AC1 running with ac1-hook (`AC1 windowed.bat`) and loaded into the city the pad file starts in.
# Starts ac1-rs itself (the same city, windowed beside it, `AC1_PADPLAY` waiting for the go file), then sends the real
# game `live on` + `padplay` and makes the go file in the same moment, so both play the file together. When it has
# played out: the real game's live log and ours' recording in the out dir, and `livelog compare` of the two
# (compare.txt). Set AC1_GAME (the install), AC1_LEVEL (default damascus), AC1_EXE (default target/release/ac1.exe).
set -euo pipefail
pad=$(realpath "$1")
out=$(realpath -m "${2:-out/live-compare/$(basename "$pad" .pad)}")
game=${AC1_GAME:-/p/SteamLibrary/steamapps/common/Assassins Creed}
exe=${AC1_EXE:-target/release/ac1.exe}
mkdir -p "$out"
win() { cygpath -w "$1" 2>/dev/null || echo "$1"; }

# The file's length (game seconds).
secs=$(awk 'NR > 1 && $1 != "start" && $1 !~ /^#/ { s += $1 } END { printf "%.1f", s }' "$pad")
echo "pad file: $pad, $secs s"

# The real game must be up and drawing (its status file moving).
s1=$(cat "$game/ac1-hook.status" 2>/dev/null || true); sleep 2; s2=$(cat "$game/ac1-hook.status" 2>/dev/null || true)
[[ -n "$s2" && "$s1" != "$s2" ]] || { echo "the real AC1 is not running with ac1-hook"; exit 1; }

# ac1-rs: the city, waiting for the go file.
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

# The real player put at the start first, the live log on, to read where its camera looks from there: the stick is
# the camera's, so ours' camera starts the same way round (its yaw in the go file).
start=$(awk '$1 == "start" { print $2, $3, $4, $5; exit }' "$pad")
camyaw=""
if [[ -n "$start" ]]; then
  rm -f "$game/ac1-live.txt" "$game/ac1-live-events.txt"
  printf 'live on\ntp %s\n' "$start" > "$game/ac1-hook.cmd"
  sleep 2
  camyaw=$(grep -o 'camdir=[-0-9.]*,[-0-9.]*,[-0-9.]*' "$game/ac1-live.txt" | tail -1 |
    awk -F'[=,]' '{ printf "%.5f", atan2(-$2, -$4) }')
  printf 'live off\n' > "$game/ac1-hook.cmd"
  sleep 0.5
  echo "the real camera's yaw at the start: ${camyaw:-unknown} rad"
fi

# Both at once: the real game's command file and ours' go file.
rm -f "$game/ac1-live.txt" "$game/ac1-live-events.txt"
printf 'live on\npadplay %s\n' "$(win "$pad")" > "$game/ac1-hook.cmd"
printf '%s\n' "$camyaw" > "$go"
echo "playing in both"
sleep "$(awk -v s="$secs" 'BEGIN { print s + 2 }')"
printf 'live off\npad off\nwhere\n' > "$game/ac1-hook.cmd"
sleep 2
wait "$ours" || true
cp "$game/ac1-live.txt" "$out/real.txt"
cp "$game/ac1-live-events.txt" "$out/real-events.txt" 2>/dev/null || true
cargo run -q --release -p ac1 --example livelog -- compare "$out/real.txt" "$out/ours.txt" > "$out/compare.txt"
echo "done: $out (real.txt, ours.txt, compare.txt)"
head -40 "$out/compare.txt"
