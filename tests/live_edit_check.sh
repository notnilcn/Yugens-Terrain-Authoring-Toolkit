#!/usr/bin/env bash
# End-to-end live-edit check: opens the editor (private Xvfb), plays the main scene, makes a
# terrain brush edit + undo + redo through the editor's undo history (like the MST brushes) and
# checks that the *running game* follows each step: its column map (what the HexPrism columns
# draw), surface height and physics collision. Captures: tests/captures/live_edit/live_*.png.
#   tests/live_edit_check.sh
# Prints "LIVE EDIT OK" or "LIVE EDIT FAIL ..." (exit 1). project.godot and the .import files
# the editor rewrites are restored afterwards.
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/tests/captures/live_edit"
backup="$(mktemp -d)"
cp "$root/project.godot" "$backup/project.godot"
(cd "$root" && find . -name '*.import' -not -path './.godot/*' -print0 | tar --null -cf "$backup/imports.tar" --files-from=-) || true
restore() {
  cp "$backup/project.godot" "$root/project.godot"
  tar -xf "$backup/imports.tar" -C "$root" 2>/dev/null || true
  rm -rf "$backup"
}
trap restore EXIT
printf '\n[autoload]\n\nLiveEditCheck="*res://tests/live_edit_check.gd"\n' >> "$root/project.godot"
KG_GODOT_IGPU=1 VK_ICD_FILENAMES=/nonexistent.json timeout "${CAPTURE_TIMEOUT:-600}" \
  xvfb-run -a -s "-screen 0 1600x900x24" stdbuf -oL -eL \
  godot --path "$root" --editor --rendering-driver opengl3 --resolution 1600x900 -- live_edit=1 \
  2>&1 | grep -E "live_edit|ERROR|SCRIPT" | grep -v "^\s*at:"
code=${PIPESTATUS[0]}
[[ $code == 0 ]] || { echo "LIVE EDIT FAIL: editor exited with $code"; exit 1; }
python3 - "$dir" <<'EOF'
import json, os, sys
d = sys.argv[1]
s = {}
for k in ("before", "after", "undone", "redone"):
    p = os.path.join(d, "ack_" + k)
    if not os.path.exists(p):
        print("LIVE EDIT FAIL: no game state for", k); sys.exit(1)
    s[k] = json.load(open(p))
fails = []
for key in ("column_map", "height", "collision", "map_height"):
    b, a, u, r = (s[k][key] for k in ("before", "after", "undone", "redone"))
    if a == b: fails.append(f"{key}: the edit did not reach the game ({b} -> {a})")
    if u != b: fails.append(f"{key}: undo did not restore the game ({b} -> {u})")
    if r != a: fails.append(f"{key}: redo differs from the edit ({a} vs {r})")
if abs(s["after"]["map_height"] - s["after"]["height"]) > 1e-3:
    fails.append(f"columns draw {s['after']['map_height']} but the surface is {s['after']['height']}")
print("game states:", json.dumps(s))
print("LIVE EDIT OK" if not fails else "LIVE EDIT FAIL: " + "; ".join(fails))
sys.exit(1 if fails else 0)
EOF
