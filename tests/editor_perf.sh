#!/usr/bin/env bash
# Runs the editor with tests/editor_perf.gd as a temporary @tool autoload and prints its
# EDITOR_PERF line. project.godot and the .import files the editor rewrites are restored.
#   tests/editor_perf.sh scene=res://demo/hex_island.tscn [preview=1] [select=PixelSky] [label=x]
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
# A run killed before its trap leaves the autoload behind: drop it before backing up.
sed -i '/^EditorPerf="\*res:\/\/tests\/editor_perf.gd"$/d' "$root/project.godot"
backup="$(mktemp -d)"
cp "$root/project.godot" "$backup/project.godot"
(cd "$root" && find . -name '*.import' -not -path './.godot/*' -print0 | tar --null -cf "$backup/imports.tar" --files-from=-) || true
# The probe may press the editor's Animate toggle, which is stored in the user's editor settings.
settings="${XDG_CONFIG_HOME:-$HOME/.config}/godot/editor_settings-4.7.tres"
[[ -f "$settings" ]] && cp "$settings" "$backup/editor_settings.tres"
restore() {
  cp "$backup/project.godot" "$root/project.godot"
  [[ -f "$backup/editor_settings.tres" ]] && cp "$backup/editor_settings.tres" "$settings"
  tar -xf "$backup/imports.tar" -C "$root" 2>/dev/null || true
  rm -rf "$backup"
}
trap restore EXIT
printf '\n[autoload]\n\nEditorPerf="*res://tests/editor_perf.gd"\n' >> "$root/project.godot"
# Private Xvfb (software GL: idle_fps / process are meaningful, frame_avg is CPU-rasterized).
# No Vulkan ICD, otherwise the editor probes a RenderingDevice on llvmpipe and crashes.
VK_ICD_FILENAMES=/nonexistent.json timeout "${PERF_TIMEOUT:-600}" xvfb-run -a -s "-screen 0 ${PERF_RES:-1600x900}x24" \
  godot --rendering-driver opengl3 --audio-driver Dummy --path "$root" --editor --resolution ${PERF_RES:-1600x900} -- editor_perf "$@" 2>&1 | tee "${PERF_LOG:-/dev/null}" | grep -E "^EDITOR_PERF|SCRIPT ERROR|panic|EDITOR_PERF:"
