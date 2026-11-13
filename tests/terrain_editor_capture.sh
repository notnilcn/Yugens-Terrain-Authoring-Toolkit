#!/usr/bin/env bash
# Screenshots the Godot editor with tool buttons pressed (see tests/terrain_editor_capture.gd).
# Usage: tests/terrain_editor_capture.sh <xvfb-display> [project-dir] -- scene=res://demo/hex_world.tscn tools=Brush,Level out=hex [dir=/abs]
# Needs ~/.local/bin/pt-xvfb. The helper is registered as an autoload by temporarily appending
# to project.godot (the editor ignores autoloads from override.cfg); project.godot and any
# .import files the editor rewrites are restored afterwards.
set -euo pipefail
disp=$1; shift
proj="$(cd "$(dirname "$0")/.." && pwd)"
if [[ "${1:-}" != "--" && -n "${1:-}" ]]; then proj="$(cd "$1" && pwd)"; shift; fi
[[ "${1:-}" == "--" ]] && shift
here="$(cd "$(dirname "$0")" && pwd)"
helper="$here/terrain_editor_capture.gd"
res_helper="res://tests/terrain_editor_capture.gd"
if [[ "$proj" != "$(cd "$here/.." && pwd)" ]]; then
  # Foreign project: copy the helper next to its project root (removed afterwards).
  cp "$helper" "$proj/__terrain_editor_capture.gd"; res_helper="res://__terrain_editor_capture.gd"
fi
backup="$(mktemp -d)"
cp "$proj/project.godot" "$backup/project.godot"
(cd "$proj" && find . -name '*.import' -not -path './.godot/*' -print0 | tar --null -cf "$backup/imports.tar" --files-from=-) || true
restore() {
  cp "$backup/project.godot" "$proj/project.godot"
  tar -xf "$backup/imports.tar" -C "$proj" 2>/dev/null || true
  rm -rf "$backup" "$proj/__terrain_editor_capture.gd" "$proj/__terrain_editor_capture.gd.uid"
}
trap restore EXIT
printf '\n[autoload]\n\nEditorCapture="*%s"\n' "$res_helper" >> "$proj/project.godot"
# No Vulkan ICD: the editor otherwise probes a RenderingDevice on llvmpipe and crashes under Xvfb.
# Compatibility/OpenGL is forced so Forward+ projects (the original MST demo) open quickly too.
VK_ICD_FILENAMES=/nonexistent.json timeout ${CAPTURE_TIMEOUT:-240} ~/.local/bin/pt-xvfb "$disp" stdbuf -oL -eL \
  godot --editor --rendering-method gl_compatibility --rendering-driver opengl3 --path "$proj" --resolution 1600x900 -- capture=1 "$@" 2>&1 \
  | grep --line-buffered -vE "^\s*$|^\[[0-9]+\]|^\s+at: "
