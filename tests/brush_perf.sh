#!/usr/bin/env bash
# Runs the editor (windowed) with tests/brush_perf.gd as a temporary @tool autoload and prints
# its BRUSH_PERF lines (plus MST_PROF breakdowns with MST_PROF=1).
#   [MST_PROF=1] [PERF_HEAD_DATA=1] tests/brush_perf.sh [scene=res://demo/hex_island.tscn] [tools=Brush,Smooth] [frames=120]
# The probe sculpts the scene: demo/ (scenes and terrain data, which the editor may save),
# project.godot and the .import files are backed up first and restored afterwards, so no stroke
# ever reaches the working tree. PERF_HEAD_DATA=1 runs on the committed demo/ files instead of
# the working copy (stable numbers while the demo data is being edited).
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
sed -i '/^BrushPerf="\*res:\/\/tests\/brush_perf.gd"$/d' "$root/project.godot"
backup="$(mktemp -d)"
cp "$root/project.godot" "$backup/project.godot"
(cd "$root" && find . -name '*.import' -not -path './.godot/*' -print0 | tar --null -cf "$backup/imports.tar" --files-from=-) || true
(cd "$root" && tar -cf "$backup/demo.tar" --exclude=demo/bin demo) || { echo "brush_perf: could not back up demo/" >&2; exit 1; }
restore() {
  cp "$backup/project.godot" "$root/project.godot"
  tar -xf "$backup/imports.tar" -C "$root" 2>/dev/null || true
  # Files the editor created under demo/ during the run go; everything else is put back.
  (cd "$root" && tar -tf "$backup/demo.tar" | sort > "$backup/before.txt" && find demo -path demo/bin -prune -o -type f -print | sort > "$backup/after.txt" \
    && comm -13 "$backup/before.txt" "$backup/after.txt" | while read -r f; do rm -f "$f"; done)
  tar -xf "$backup/demo.tar" -C "$root"
  rm -rf "$backup"
}
trap restore EXIT
if [[ "${PERF_HEAD_DATA:-0}" == 1 ]]; then
  # `git show` (not checkout): the index, and anything staged in it, stays untouched.
  (cd "$root" && for f in demo/*.tscn demo/*.res; do git show "HEAD:$f" > "$f" 2>/dev/null || true; done)
fi
printf '\n[autoload]\n\nBrushPerf="*res://tests/brush_perf.gd"\n' >> "$root/project.godot"
# Private X display when pt-xvfb / xvfb-run is available (tests/xvfb.sh), else $DISPLAY.
wrap=(); [[ -z "${DISPLAY:-}" || -n "${PT_XVFB:-}" || "${PERF_XVFB:-0}" == 1 ]] && { source "$root/tests/xvfb.sh"; xvfb_wrap "${PERF_DISPLAY:-97}"; }
VK_ICD_FILENAMES=/nonexistent.json timeout "${PERF_TIMEOUT:-600}" "${wrap[@]}" godot --path "$root" --editor --rendering-driver opengl3 --resolution 1600x900 -- brush_perf "$@" 2>&1 \
  | grep -E "^BRUSH_PERF|^MST_PROF|SCRIPT ERROR|panic"
# The editor's exit code (1 = no terrain / missing tool button), not grep's.
exit "${PIPESTATUS[0]}"
