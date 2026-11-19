#!/usr/bin/env bash
# Runs the headless editor with tests/stroke_cost.gd as a temporary @tool autoload and prints its
# STROKE_COST lines (plus MST_PROF breakdowns with MST_PROF=1). Exit code 1 when a stroke costs
# more than max_ms per motion event.
#   [MST_PROF=1] tests/stroke_cost.sh [scene=res://demo/hex_island.tscn] [ops=level,raise,smooth] [frames=60] [events=2] [max_ms=15]
# The probe sculpts the scene: demo/ (scenes and terrain data), project.godot and the .import
# files are backed up first and restored afterwards, so no stroke reaches the working tree.
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
sed -i '/^StrokeCost="\*res:\/\/tests\/stroke_cost.gd"$/d' "$root/project.godot"
backup="$(mktemp -d)"
cp "$root/project.godot" "$backup/project.godot"
(cd "$root" && find . -name '*.import' -not -path './.godot/*' -print0 | tar --null -cf "$backup/imports.tar" --files-from=-) || true
(cd "$root" && tar -cf "$backup/demo.tar" --exclude=demo/bin demo) || { echo "stroke_cost: could not back up demo/" >&2; exit 1; }
restore() {
  cp "$backup/project.godot" "$root/project.godot"
  tar -xf "$backup/imports.tar" -C "$root" 2>/dev/null || true
  (cd "$root" && tar -tf "$backup/demo.tar" | sort > "$backup/before.txt" && find demo -path demo/bin -prune -o -type f -print | sort > "$backup/after.txt" \
    && comm -13 "$backup/before.txt" "$backup/after.txt" | while read -r f; do rm -f "$f"; done)
  tar -xf "$backup/demo.tar" -C "$root"
  rm -rf "$backup"
}
trap restore EXIT
printf '\n[autoload]\n\nStrokeCost="*res://tests/stroke_cost.gd"\n' >> "$root/project.godot"
timeout "${PERF_TIMEOUT:-600}" godot --headless --path "$root" --editor -- stroke_cost "$@" 2>&1 \
  | grep -E "STROKE_COST|^MST_PROF|SCRIPT ERROR|panic"
exit "${PIPESTATUS[0]}"
