#!/usr/bin/env bash
# Windowed builder editor capture (tests/builder_editor_capture.gd via tests/builder_editor.tscn):
#   tests/builder_editor_capture.sh [extra user args]
# Needs a GPU display (on llvmpipe the helper prints BUILDER EDITOR SKIP). The editor opens
# tests/builder_editor.tscn, so its open scene tabs / docks (.godot/editor/editor_layout.cfg),
# builder preferences (project_metadata.cfg), editor settings, project.godot, the .import files
# and demo/ are restored afterwards (tests/lib/editor_check_env.sh; refuses to run next to
# another Godot instance). Screenshots go to tests/captures/, the log to .tmp/builder_editor.log.
# Exit 0 when the helper printed BUILDER EDITOR OK (or SKIP) and no Rust panic was logged.
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source "$root/tests/lib/editor_check_env.sh"
ecenv_begin "$root" ''
log="$backup/builder_editor.log"
timeout "${BUILDER_EDITOR_TIMEOUT:-600}" godot --path "$root" --editor res://tests/builder_editor.tscn -- capture "$@" > "$log" 2>&1
code=$?
mkdir -p "$root/.tmp" && cp "$log" "$root/.tmp/builder_editor.log"
grep -E "^(PASS|FAIL) |^BUILDER EDITOR|SCRIPT ERROR|ERROR:|panic" "$log"
grep -qE "^BUILDER EDITOR (OK|SKIP)" "$log" && ! grep -q "\[panic" "$log" && [[ $code == 0 ]]
