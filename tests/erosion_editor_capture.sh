#!/usr/bin/env bash
# Opens the editor on demo/hex_island.tscn under a private Xvfb with
# tests/erosion_editor_capture.gd as a temporary @tool autoload, and checks the background
# erosion of MarchingSquaresErosion through the pixel UI: Erode press, progress row, completion,
# one undo step that restores the heights, and Cancel. Nothing is saved.
#   tests/erosion_editor_capture.sh <display-num> [name=erosion_editor] [select=Erosion] [iterations=1500000] [pixels_per_cell=8]
# Saves tests/captures/<name>_{running,done,undone}.png; exits 1 when a check fails.
# project.godot and every .import file the editor rewrites are restored afterwards, and the
# demo data is reverted (never run two editor capture scripts at once in one checkout).
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
disp="$1"; shift
backup="$(mktemp -d)"
cp "$root/project.godot" "$backup/project.godot"
(cd "$root" && find . -name '*.import' -not -path './.godot/*' -print0 | tar --null -cf "$backup/imports.tar" --files-from=-) || true
restore() {
  cp "$backup/project.godot" "$root/project.godot"
  tar -xf "$backup/imports.tar" -C "$root" 2>/dev/null || true
  (cd "$root" && git checkout -q -- demo/hex_island_data.res demo/hex_island.tscn 2>/dev/null) || true
  rm -rf "$backup"
}
trap restore EXIT
printf '\n[autoload]\n\nErosionEditorCapture="*res://tests/erosion_editor_capture.gd"\n' >> "$root/project.godot"
# No Vulkan ICD: otherwise the editor probes a RenderingDevice on llvmpipe and crashes under Xvfb.
VK_ICD_FILENAMES=/nonexistent.json timeout "${CAPTURE_TIMEOUT:-600}" ~/.local/bin/pt-xvfb "$disp" stdbuf -oL -eL \
  godot --path "$root" --editor --rendering-driver opengl3 --resolution 1600x900 -- erosion_editor_capture "$@"
