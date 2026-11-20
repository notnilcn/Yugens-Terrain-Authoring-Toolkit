#!/usr/bin/env bash
# Prepare the persistent QA scaffolding for the MarchingSquaresTerrain addon.
#
#   qa/setup.sh [--check] [--refresh] [--clean]
#
# The repository holds the harness, the driver and the session manager in qa/.
# Generated state (project template, isolated sessions, monitor logs) lives in
# $MST_QA_STATE (default /tmp/opencode/qa) and is rebuilt from this checkout.
#
#   --check     verify prerequisites and the generated template; change nothing
#   --refresh   rebuild the project template from the current working tree
#   --clean     remove the generated state tree (sessions, template, monitor)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
state="${MST_QA_STATE:-/tmp/opencode/qa}"
tpl="$state/project_base"
mode="setup"

for arg in "$@"; do
  case "$arg" in
    --check) mode="check" ;;
    --refresh) mode="refresh" ;;
    --clean) mode="clean" ;;
    -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

need() { # name, probe, hint
  if ! eval "$2" >/dev/null 2>&1; then
    echo "missing: $1 — $3" >&2
    return 1
  fi
  return 0
}

check_prereqs() {
  local ok=0
  need godot 'command -v godot' 'install Godot 4.7+ on PATH' || ok=1
  need python3 'command -v python3' 'install Python 3' || ok=1
  need Xvfb 'command -v Xvfb' 'install xvfb (apt install xvfb)' || ok=1
  need python-xlib 'python3 -c "import Xlib"' 'pip install python-xlib' || ok=1
  need pillow 'python3 -c "import PIL"' 'pip install pillow' || ok=1
  if [[ ! -f "$root/qa/harness/harness.gd" || ! -f "$root/qa/harness/plugin.cfg" ]]; then
    echo "missing: qa/harness (the in-editor harness)" >&2; ok=1
  fi
  if [[ ! -f "$root/qa/drive.py" || ! -f "$root/qa/session.sh" ]]; then
    echo "missing: qa/drive.py or qa/session.sh" >&2; ok=1
  fi
  if command -v godot >/dev/null; then
    local v; v="$(godot --version 2>/dev/null | head -1)"
    [[ "$v" == 4.* ]] || { echo "Godot 4.7+ required, found: $v" >&2; ok=1; }
  fi
  return "$ok"
}

build_template() {
  mkdir -p "$tpl"
  echo "building $tpl from $root (excluding .git, rust/target, qa screenshots/findings)"
  if command -v rsync >/dev/null; then
    rsync -a --delete \
      --exclude '.git' --exclude 'rust/target' \
      --exclude 'qa/screenshots' --exclude 'qa/findings' \
      "$root/" "$tpl/"
  else
    rm -rf "$tpl"; mkdir -p "$tpl"
    (cd "$root" && tar -cf - --exclude=.git --exclude=rust/target \
      --exclude=qa/screenshots --exclude=qa/findings .) | (cd "$tpl" && tar -xf -)
  fi
  du -sh "$tpl"
}

case "$mode" in
  clean)
    rm -rf "$state"
    echo "removed $state"
    ;;
  check)
    check_prereqs
    echo "prerequisites: ok"
    if [[ -d "$tpl" ]]; then
      echo "template: $tpl ($(du -sh "$tpl" | cut -f1))"
    else
      echo "template: not built (run qa/setup.sh)"
    fi
    if [[ -f "$state/rmon/monitor.log" ]]; then
      echo "monitor: $(tail -1 "$state/rmon/monitor.log" 2>/dev/null | head -c 300)"
    else
      echo "monitor: not started (run qa/resource-monitor.sh start)"
    fi
    ;;
  refresh)
    check_prereqs
    build_template
    ;;
  setup)
    check_prereqs
    if [[ -d "$tpl" ]]; then
      echo "template: exists at $tpl (use --refresh to rebuild)"
    else
      build_template
    fi
    cat <<EOF

Ready. Next:
  qa/resource-monitor.sh init      # limits for the wave guard (once)
  qa/resource-monitor.sh start     # background CPU/RAM samples
  qa/session.sh start mytest --display :41 --port 8821
  source /tmp/opencode/qa/sessions/mytest/env.sh
  python3 qa/drive.py info
  qa/session.sh stop mytest

State is regenerated and not committed: \$MST_QA_STATE=$state
EOF
    ;;
esac
