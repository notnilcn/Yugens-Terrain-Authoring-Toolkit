#!/usr/bin/env bash
# Headless editor check of Godot's transform gizmo on the demo's nodes (tests/gizmo_editor_check.gd).
#   tests/gizmo_editor_check.sh [scene=res://demo/hex_island.tscn]
# The helper is a temporary autoload (appended to project.godot); project.godot, the .import
# files and demo/ are restored afterwards. Prints the check lines and "GIZMO OK" / "GIZMO FAIL".
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
line='GizmoCheck="*res://tests/gizmo_editor_check.gd"'
sed -i "/^GizmoCheck=/d" "$root/project.godot"
backup="$(mktemp -d)"
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
printf '\n[autoload]\n\n%s\n' "$line" >> "$root/project.godot"
log="$backup/gizmo.log"
timeout "${GIZMO_TIMEOUT:-300}" godot --headless --editor --path "$root" -- gizmo_check=1 "$@" > "$log" 2>&1
code=$?
grep -E "^gizmo_check|^GIZMO|SCRIPT ERROR|ERROR:|panic" "$log"
# Exit 0 only when the helper printed GIZMO OK and the editor exited cleanly.
grep -q "^GIZMO OK" "$log" && [[ $code == 0 ]]
