#!/usr/bin/env bash
# Screenshots the Godot editor with tool buttons pressed (see tests/terrain_editor_capture.gd).
# Usage: tests/terrain_editor_capture.sh <xvfb-display> [project-dir] -- scene=res://demo/hex_island.tscn tools=Brush,Level out=hex [dir=/abs]
# Runs on a private X display (tests/xvfb.sh: pt-xvfb / PT_XVFB, else xvfb-run). The helper is registered as an autoload by temporarily appending
# to project.godot (the editor ignores autoloads from override.cfg); project.godot and any
# .import files the editor rewrites are restored afterwards.
set -euo pipefail
disp=$1; shift
proj="$(cd "$(dirname "$0")/.." && pwd)"
if [[ "${1:-}" != "--" && -n "${1:-}" ]]; then proj="$(cd "$1" && pwd)"; shift; fi
[[ "${1:-}" == "--" ]] && shift
here="$(cd "$(dirname "$0")" && pwd)"
# HELPER=<file in tests/> runs another editor script the same way (e.g. foliage_editor_check.gd).
helper="$here/${HELPER:-terrain_editor_capture.gd}"
res_helper="res://tests/${HELPER:-terrain_editor_capture.gd}"
if [[ "$proj" != "$(cd "$here/.." && pwd)" ]]; then
  # Foreign project: copy the helper next to its project root (removed afterwards).
  cp "$helper" "$proj/__terrain_editor_capture.gd"; res_helper="res://__terrain_editor_capture.gd"
  cp "$here/editor_ready.gd" "$proj/__editor_ready.gd"
fi
backup="$(mktemp -d)"
cp "$proj/project.godot" "$backup/project.godot"
(cd "$proj" && find . -name '*.import' -not -path './.godot/*' -print0 | tar --null -cf "$backup/imports.tar" --files-from=-) || true
# The checks sculpt / paint the demo and the editor may save it: demo/ (minus built libs) is
# put back afterwards, and files the run created there are removed.
[[ -d "$proj/demo" ]] && (cd "$proj" && tar -cf "$backup/demo.tar" --exclude=demo/bin demo)
restore() {
  cp "$backup/project.godot" "$proj/project.godot"
  tar -xf "$backup/imports.tar" -C "$proj" 2>/dev/null || true
  if [[ -f "$backup/demo.tar" ]]; then
    (cd "$proj" && tar -tf "$backup/demo.tar" | sort > "$backup/before.txt" && find demo -path demo/bin -prune -o -type f -print | sort > "$backup/after.txt" \
      && comm -13 "$backup/before.txt" "$backup/after.txt" | while read -r f; do rm -f "$f"; done)
    tar -xf "$backup/demo.tar" -C "$proj"
  fi
  rm -rf "$backup" "$proj/__terrain_editor_capture.gd" "$proj/__terrain_editor_capture.gd.uid" "$proj/__editor_ready.gd" "$proj/__editor_ready.gd.uid"
}
trap restore EXIT
printf '\n[autoload]\n\nEditorCapture="*%s"\n' "$res_helper" >> "$proj/project.godot"
XVFB_SCREEN="${XVFB_SCREEN:-${CAPTURE_RES:-1600x900}x24}"; source "$(dirname "$0")/xvfb.sh"; xvfb_wrap "$disp"
# No Vulkan ICD: the editor otherwise probes a RenderingDevice on llvmpipe and crashes under Xvfb.
# Compatibility/OpenGL is forced so Forward+ projects (the original MST demo) open quickly too.
VK_ICD_FILENAMES=/nonexistent.json timeout ${CAPTURE_TIMEOUT:-240} "${wrap[@]}" stdbuf -oL -eL \
  godot --editor --rendering-method gl_compatibility --rendering-driver opengl3 --path "$proj" --resolution "${CAPTURE_RES:-1600x900}" -- capture=1 "$@" 2>&1 \
  | grep --line-buffered -vE "^\s*$|^\[[0-9]+\]|^\s+at: "
