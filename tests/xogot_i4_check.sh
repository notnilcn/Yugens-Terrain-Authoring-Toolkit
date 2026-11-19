#!/usr/bin/env bash
# Headless editor check of the I4 builder ports (tests/xogot_i4_check.gd): R1 fixes, paint / palettes / smoothing, UV extras, inspector refresh.
#   tests/xogot_i4_check.sh
# The helper is a temporary autoload (appended to project.godot). project.godot, the .import
# files, demo/, the editor settings file and .godot/editor/project_metadata.cfg are restored
# afterwards (tests/lib/editor_check_env.sh; it refuses to run next to another Godot instance).
# The full log is kept in .tmp/xogot_i4.log. Prints the check lines and "XOGOT I4 OK" / "XOGOT I4 FAIL".
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source "$root/tests/lib/editor_check_env.sh"
ecenv_begin "$root" 'XogotI4Check="*res://tests/xogot_i4_check.gd"'
log="$backup/xogot_i4.log"
timeout "${XOGOT_I4_TIMEOUT:-300}" godot --headless --editor --path "$root" -- xogot_i4=1 "$@" > "$log" 2>&1
code=$?
mkdir -p "$root/.tmp" && cp "$log" "$root/.tmp/xogot_i4.log"
grep -E "^xogot_i4|^XOGOT I4|SCRIPT ERROR|ERROR:|panic" "$log"
# Exit 0 only when the helper printed XOGOT I4 OK, no Rust panic was logged and the editor
# exited cleanly.
grep -q "^XOGOT I4 OK" "$log" && ! grep -q "\[panic" "$log" && [[ $code == 0 ]]
