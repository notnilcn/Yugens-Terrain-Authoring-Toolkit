#!/usr/bin/env bash
# Opens the editor under a private Xvfb, runs tests/editor_capture.gd as a temporary @tool
# autoload and saves the editor window to tests/captures/<name>.png.
#   tests/editor_capture.sh <display-num> scene=res://demo/hex_island.tscn name=w2_x preview=1 [select=PixelSky] ...
# The editor ignores autoloads from override.cfg, so the helper is appended to project.godot;
# project.godot and every .import file the editor rewrites are restored afterwards.
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
disp="$1"; shift
backup="$(mktemp -d)"
cp "$root/project.godot" "$backup/project.godot"
(cd "$root" && find . -name '*.import' -not -path './.godot/*' -print0 | tar --null -cf "$backup/imports.tar" --files-from=-) || true
restore() {
  cp "$backup/project.godot" "$root/project.godot"
  tar -xf "$backup/imports.tar" -C "$root" 2>/dev/null || true
  rm -rf "$backup"
}
trap restore EXIT
printf '\n[autoload]\n\nEditorCapture="*res://tests/editor_capture.gd"\n' >> "$root/project.godot"
# No Vulkan ICD: otherwise the editor probes a RenderingDevice on llvmpipe and crashes under Xvfb.
# Without pt-xvfb or xvfb-run the editor opens on the existing X display :<display-num>.
source "$(dirname "$0")/xvfb.sh"; xvfb_wrap "$disp"
VK_ICD_FILENAMES=/nonexistent.json timeout "${CAPTURE_TIMEOUT:-600}" "${wrap[@]}" stdbuf -oL -eL \
  godot --path "$root" --editor --rendering-driver opengl3 --resolution 1600x900 -- editor_capture "$@"
