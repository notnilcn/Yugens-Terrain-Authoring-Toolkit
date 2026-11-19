#!/usr/bin/env bash
# Headless editor check of the Xogot Modeler UI ports in the level builder (tests/xogot_i2_check.gd).
#   tests/xogot_i2_check.sh
# The helper is a temporary autoload (appended to project.godot); project.godot, the .import
# files, demo/ and the editor's settings / layout files are restored afterwards
# (tests/lib/editor_check_env.sh; it refuses to run next to another Godot instance); the full log is kept in .tmp/xogot_i2.log. Prints the check lines and "XOGOT I2 OK" / "XOGOT I2 FAIL".
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source "$root/tests/lib/editor_check_env.sh"
ecenv_begin "$root" 'XogotI2Check="*res://tests/xogot_i2_check.gd"'
log="$backup/xogot_i2.log"
timeout "${XOGOT_I2_TIMEOUT:-300}" godot --headless --editor --path "$root" -- xogot_i2=1 "$@" > "$log" 2>&1
code=$?
mkdir -p "$root/.tmp" && cp "$log" "$root/.tmp/xogot_i2.log"
grep -E "^xogot_i2|^XOGOT I2|SCRIPT ERROR|ERROR:|panic" "$log"
# Exit 0 only when the helper printed XOGOT I2 OK, no Rust panic was logged and the editor
# exited cleanly.
grep -q "^XOGOT I2 OK" "$log" && ! grep -q "\[panic" "$log" && [[ $code == 0 ]]
