#!/usr/bin/env bash
# Headless editor check of the I5 integration (tests/xogot_i5_check.gd): R2 fixes, Godot Mesh .res export, Mesh UV dock, HUD per viewport, Xogot confirmations.
#   tests/xogot_i5_check.sh
# The helper is a temporary autoload (appended to project.godot). project.godot, the .import
# files, demo/, the editor settings file and .godot/editor/project_metadata.cfg are restored
# afterwards (tests/lib/editor_check_env.sh; it refuses to run next to another Godot instance).
# The full log is kept in .tmp/xogot_i5.log. Prints the check lines and "XOGOT I5 OK" / "XOGOT I5 FAIL".
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source "$root/tests/lib/editor_check_env.sh"
ecenv_begin "$root" 'XogotI5Check="*res://tests/xogot_i5_check.gd"'
log="$backup/xogot_i5.log"
timeout "${XOGOT_I5_TIMEOUT:-300}" godot --headless --editor --path "$root" -- xogot_i5=1 "$@" > "$log" 2>&1
code=$?
mkdir -p "$root/.tmp" && cp "$log" "$root/.tmp/xogot_i5.log"
grep -E "^xogot_i5|^XOGOT I5|SCRIPT ERROR|ERROR:|panic" "$log"
# Exit 0 only when the helper printed XOGOT I5 OK, no Rust panic was logged and the editor
# exited cleanly.
grep -q "^XOGOT I5 OK" "$log" && ! grep -q "\[panic" "$log" && [[ $code == 0 ]]
