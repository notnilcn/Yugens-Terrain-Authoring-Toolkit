#!/usr/bin/env bash
# Headless editor check of the Foliage tool (tests/foliage_headless_check.gd): palette, painting
# through the real 3D-viewport input, erase, undo / redo. No window, no screenshots.
#   tests/foliage_headless_check.sh
# Prints "FOLIAGE HEADLESS OK" or "FOLIAGE HEADLESS FAIL: ..." (exit 1). project.godot, the
# .import files and demo/ are restored afterwards (the editor may save the painted demo).
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
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
printf '\n[autoload]\n\nFoliageHeadlessCheck="*res://tests/foliage_headless_check.gd"\n' >> "$root/project.godot"
out="$(timeout "${CHECK_TIMEOUT:-300}" godot --headless --editor --path "$root" -- foliage_headless=1 2>&1)"
code=$?
echo "$out" | grep -E "foliage_headless|ERROR|SCRIPT ERROR|panicked" | grep -v "^\s*at:"
echo "$out" | grep -q "FOLIAGE HEADLESS OK" && [[ $code == 0 ]]
