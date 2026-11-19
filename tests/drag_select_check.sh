#!/usr/bin/env bash
# Headless check of the Brush tool's Drag mode from close / far / steep / top-down editor
# cameras (tests/drag_select_check.gd, registered as a temporary autoload; real 3D view input
# path, dummy renderer). Prints "DRAG SELECT OK" or the failures (exit 1). project.godot is
# restored afterwards and the drags are undone, so demo/ is never saved.
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
backup="$(mktemp -d)"
cp "$root/project.godot" "$backup/project.godot"
log="$backup/log"
restore() {
  cp "$backup/project.godot" "$root/project.godot"
  rm -rf "$backup"
}
trap restore EXIT
printf '\n[autoload]\n\nDragSelectCheck="*res://tests/drag_select_check.gd"\n' >> "$root/project.godot"
timeout "${CHECK_TIMEOUT:-300}" godot --headless --editor --path "$root" -- drag_select=1 > "$log" 2>&1
code=$?
grep -E "drag_select|DRAG SELECT|SCRIPT ERROR|^ERROR" "$log" | grep -v "^\s*at:"
grep -q "DRAG SELECT OK" "$log" && [[ $code == 0 ]]
