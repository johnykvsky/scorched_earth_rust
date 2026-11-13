#!/bin/bash
# Run the original Scorched Earth 1.5 in DOSBox with edited settings and
# screenshot round 1 of a 2-human-player game.
#
# Usage: GAME_DIR=~/games/scorch-test tools/re/run_original.sh OUT.png KEY=VALUE ...
#   GAME_DIR  a disposable copy of the shareware game that already contains a
#             SCORCH.CFG saved by the game (main menu: press "c" = Save Changes).
#   KEY=VALUE scorch.cfg settings to change first, e.g. LAND1=100 SKY=Cavern.
#
# Needs: dosbox, xdotool, ImageMagick (import). Leaves DOSBox running so you can
# keep driving it; stop it with: pkill -f "dosbox -conf $GAME_DIR/dosbox.conf"
#
# Notes learned the hard way:
#  - The game polls held keys, so very short key taps can be missed; hold keys
#    (xdotool keydown, sleep 0.15, keyup) when timing matters.
#  - Find the NEW DOSBox window: old window ids can linger after a restart.
#  - DOSBox is given a private HOME so it doesn't create ~/.dosbox.
set -e
OUT=$1; shift
: "${GAME_DIR:?set GAME_DIR to a disposable copy of the game}"
cfg=$GAME_DIR/SCORCH.CFG
[ -f "$cfg" ] || { echo "No $cfg: start the game once and press c (Save Changes)"; exit 1; }

python3 -I - "$cfg" "$@" <<'PY'
import sys
p = sys.argv[1]; kv = dict(a.split('=', 1) for a in sys.argv[2:])
lines = open(p, newline='').read().split('\r\n')
out = [f"{l.split('=',1)[0]}={kv.pop(l.split('=',1)[0])}" if l.split('=',1)[0] in kv else l for l in lines]
open(p, 'w', newline='').write('\r\n'.join(out))
PY

cat > "$GAME_DIR/dosbox.conf" <<EOF
[sdl]
windowresolution=original
output=surface
[render]
scaler=normal2x
[cpu]
cycles=20000
[autoexec]
mount c $GAME_DIR
c:
SCORCH.EXE
exit
EOF

export HOME=$GAME_DIR/.home; mkdir -p "$HOME"
BEFORE=$(xdotool search --name "DOSBox" 2>/dev/null | sort || true)
dosbox -conf "$GAME_DIR/dosbox.conf" >/dev/null 2>&1 &
for i in $(seq 40); do
  sleep 0.25
  WID=$(comm -13 <(echo "$BEFORE") <(xdotool search --name "DOSBox" 2>/dev/null | sort) | head -1)
  [ -n "$WID" ] && break
done
sleep 3
xdotool windowactivate --sync "$WID"; sleep 0.3
k(){ xdotool key --window "$WID" "$@"; sleep 0.6; }
k s                       # Start
k A; k Return; k Return   # player 1: name "A", Done
k B; k Return; k Return   # player 2: name "B", Done
sleep 4
import -window "$WID" "$OUT"
echo "screenshot: $OUT  (DOSBox window $WID still running)"
