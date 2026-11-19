#!/usr/bin/env bash
# Headless editor check of the Xogot Modeler features in the level builder (tests/xogot_i1_check.gd).
#   tests/xogot_i1_check.sh
# The helper is a temporary autoload (appended to project.godot); project.godot, the .import
# files, demo/ and the editor's settings / layout files are restored afterwards
# (tests/lib/editor_check_env.sh; it refuses to run next to another Godot instance); the full log is kept in .tmp/xogot_i1.log. Prints the check lines and "XOGOT I1 OK" / "XOGOT I1 FAIL".
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source "$root/tests/lib/editor_check_env.sh"
ecenv_begin "$root" 'XogotI1Check="*res://tests/xogot_i1_check.gd"'
log="$backup/xogot_i1.log"
timeout "${XOGOT_I1_TIMEOUT:-300}" godot --headless --editor --path "$root" -- xogot_i1=1 "$@" > "$log" 2>&1
code=$?
mkdir -p "$root/.tmp" && cp "$log" "$root/.tmp/xogot_i1.log"
grep -E "^xogot_i1|^XOGOT I1|SCRIPT ERROR|ERROR:|panic" "$log"
# Exit 0 only when the helper printed XOGOT I1 OK, no Rust panic was logged and the editor
# exited cleanly.
grep -q "^XOGOT I1 OK" "$log" && ! grep -q "\[panic" "$log" && [[ $code == 0 ]]
