#!/usr/bin/env bash
# Headless end-to-end check of editing a terrain with the MST tools while the game runs:
# tests/live_edit_ui_check.gd strokes through the editor's 3D view input (real plugin path),
# undoes through the editor's undo history, and a headless game started from the editor
# (play_main_scene, live edit on) must follow every step. No window: editor and game both run
# with --headless (the dummy renderer, so it checks node / CPU state, not pixels).
#   tests/live_edit_ui_check.sh            # editor + running game
#   tests/live_edit_ui_check.sh --no-game  # editor only (strokes, Ctrl+Z, rebuild comparison)
# Prints "LIVE EDIT UI OK" or the failures (exit 1). project.godot is restored afterwards.
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
game="game=1"; [[ "${1:-}" == "--no-game" ]] && game=""
backup="$(mktemp -d)"
cp "$root/project.godot" "$backup/project.godot"
log="$backup/log"
restore() {
  pkill -f -- "--path $root --remote-debug" 2>/dev/null  # a game left running by a failed run
  cp "$backup/project.godot" "$root/project.godot"
  rm -rf "$backup" "$root/tests/captures/live_edit_ui"
}
trap restore EXIT
printf '\n[autoload]\n\nLiveEditUiCheck="*res://tests/live_edit_ui_check.gd"\n\n[editor]\n\nrun/main_run_args="--headless"\n' >> "$root/project.godot"
timeout "${CHECK_TIMEOUT:-400}" godot --headless --editor --path "$root" -- live_edit_ui=1 $game > "$log" 2>&1
code=$?
grep -E "live_edit_ui|LIVE EDIT UI|SCRIPT ERROR|^ERROR" "$log" | grep -v "^\s*at:"
grep -q "LIVE EDIT UI OK" "$log" && [[ $code == 0 ]]
