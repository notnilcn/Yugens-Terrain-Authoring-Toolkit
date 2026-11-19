#!/usr/bin/env bash
# Headless editor check of the builder UI layout switch (MST / Xogot) (tests/xogot_i3_check.gd).
#   tests/xogot_i3_check.sh
# The helper is a temporary autoload (appended to project.godot). project.godot, the .import
# files, demo/, the editor settings file and .godot/editor/project_metadata.cfg are restored
# afterwards (tests/lib/editor_check_env.sh; it refuses to run next to another Godot instance).
# The full log is kept in .tmp/xogot_i3.log. Prints the check lines and "XOGOT I3 OK" / "XOGOT I3 FAIL".
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source "$root/tests/lib/editor_check_env.sh"
ecenv_begin "$root" 'XogotI3Check="*res://tests/xogot_i3_check.gd"'
log="$backup/xogot_i3.log"
# XOGOT_I3_START_XOGOT=1: the editor starts in the Xogot layout (start-up path; restored after).
if [[ -n "${XOGOT_I3_START_XOGOT:-}" && -f "$ECENV_SETTINGS" ]]; then
  sed -i '/^marching_squares_terrain\/builder\/ui_layout = /d' "$ECENV_SETTINGS"
  sed -i '/^\[resource\]$/a marching_squares_terrain/builder/ui_layout = 1' "$ECENV_SETTINGS"
fi
timeout "${XOGOT_I3_TIMEOUT:-300}" godot --headless --editor --path "$root" -- xogot_i3=1 "$@" > "$log" 2>&1
code=$?
mkdir -p "$root/.tmp" && cp "$log" "$root/.tmp/xogot_i3.log"
grep -E "^xogot_i3|^XOGOT I3|SCRIPT ERROR|ERROR:|panic" "$log"
# Exit 0 only when the helper printed XOGOT I3 OK, no Rust panic was logged and the editor
# exited cleanly.
grep -q "^XOGOT I3 OK" "$log" && ! grep -q "\[panic" "$log" && [[ $code == 0 ]]
