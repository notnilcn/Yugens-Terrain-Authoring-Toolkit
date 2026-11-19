#!/usr/bin/env bash
# Editor vs game lighting parity (tests/editor_game_parity.gd): dumps the published pt_* globals,
# suns, effective environment / sky, camera environments and water materials of a scene once from
# a headless editor with the scene open and once from the scene running headless as a game, then
# compares them, as loaded and again after moving PixelDayNight to time=<t> (default 0.3) on both
# sides. Exit 0 and "PARITY OK" when they match (time-driven animation excepted).
#   tests/editor_game_parity.sh [scene=res://demo/hex_island.tscn] [frames=120]
# PARITY_KEEP=<dir> keeps the two JSON dumps there. project.godot, the .import files and demo/
# are restored afterwards.
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
scene="res://demo/hex_island.tscn"
for a in "$@"; do [[ $a == scene=* ]] && scene="${a#scene=}"; done
sed -i '/^EditorGameParity=/d' "$root/project.godot"
backup="$(mktemp -d)"
out="${PARITY_KEEP:-$backup}"
mkdir -p "$out"
cp "$root/project.godot" "$backup/project.godot"
(cd "$root" && find . -name '*.import' -not -path './.godot/*' -print0 | tar --null -cf "$backup/imports.tar" --files-from=-) || true
(cd "$root" && tar -cf "$backup/demo.tar" --exclude=demo/bin demo)
restore() {
  cp "$backup/project.godot" "$root/project.godot"
  tar -xf "$backup/imports.tar" -C "$root" 2>/dev/null || true
  tar -xf "$backup/demo.tar" -C "$root"
  rm -rf "$backup"
}
trap restore EXIT
printf '\n[autoload]\n\nEditorGameParity="*res://tests/editor_game_parity.gd"\n' >> "$root/project.godot"
filter='^PARITY|SCRIPT ERROR|panicked'
timeout "${PARITY_TIMEOUT:-300}" godot --headless --editor --path "$root" -- parity=editor out="$out/editor.json" "$@" > "$backup/editor.log" 2>&1
code=$?
grep -E "$filter" "$backup/editor.log"
[[ $code == 0 && -s "$out/editor.json" ]] || { echo "PARITY FAIL: editor run exited $code"; exit 1; }
timeout "${PARITY_TIMEOUT:-300}" godot --headless --path "$root" "$scene" -- parity=game out="$out/game.json" "$@" > "$backup/game.log" 2>&1
code=$?
grep -E "$filter" "$backup/game.log"
[[ $code == 0 && -s "$out/game.json" ]] || { echo "PARITY FAIL: game run exited $code"; exit 1; }
python3 - "$out/editor.json" "$out/game.json" <<'EOF'
import json, sys
ed, gm = (json.load(open(p)) for p in sys.argv[1:3])
# Not lighting, or editor-only by design:
#   pt_time / pt_cloud_b       time-driven animation (shared clock, cloud drift)
#   pt_fire_*                  the fire sim runs in the game only (PixelFireSystem.preview_in_editor)
#   pt_lod_*                   distant LOD is game-only unless MarchingSquaresTerrain.lod_in_editor
#   pt_texel_world             published by the pixel pipeline, which needs a real editor viewport
#   PixelDayNight.running      stopped by the game dump itself (same time of day on both sides)
SKIP_PREFIX = ("mode", "adapter", "global.pt_time", "rs.pt_time", "global.pt_cloud_b", "rs.pt_cloud_b",
               "global.pt_fire_", "rs.pt_fire_", "global.pt_lod_", "rs.pt_lod_",
               "global.pt_texel_world", "rs.pt_texel_world", "node.PixelDayNight.running")
def close(a, b):
    if isinstance(a, bool) or isinstance(b, bool) or a is None or b is None:
        return a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return abs(a - b) <= 1e-3 + 1e-3 * max(abs(a), abs(b))
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(close(x, y) for x, y in zip(a, b))
    return a == b
diff = []
for k in sorted(set(ed) | set(gm)):
    if k.split(".", 1)[-1].startswith(SKIP_PREFIX):
        continue
    if k not in ed or k not in gm:
        diff.append(f"{k}: editor={ed.get(k, '<missing>')} game={gm.get(k, '<missing>')}")
    elif not close(ed[k], gm[k]):
        diff.append(f"{k}: editor={ed[k]} game={gm[k]}")
print(f"PARITY compared {len(set(ed) & set(gm))} values")
for d in diff:
    print("PARITY diff", d)
print("PARITY OK" if not diff else f"PARITY FAIL: {len(diff)} difference(s)")
sys.exit(1 if diff else 0)
EOF
