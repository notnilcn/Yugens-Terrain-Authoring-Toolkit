#!/usr/bin/env bash
# Start/stop an isolated Godot editor session for UI testing.
#
#   qa/session.sh start <name> [--display :NN] [--port N] [--scene res://...]
#                             [--project DIR] [--width W] [--height H]
#   qa/session.sh stop <name>
#   qa/session.sh status <name>
#   qa/session.sh env <name>        # prints exports for drive.py
#
# Each session gets its own Xvfb display, XDG config/data/cache and a disposable
# copy of the project (template at /tmp/opencode/qa/project_base). The editor runs
# the QA harness addon from qa/harness on 127.0.0.1:$PORT.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
base_state="${MST_QA_STATE:-/tmp/opencode/qa}"
tpl="$base_state/project_base"

usage() { sed -n '2,13p' "$0"; exit 2; }

copy_template() {
  if [[ ! -d "$tpl" ]]; then
    echo "creating project template at $tpl ..."
    mkdir -p "$tpl"
    if command -v rsync >/dev/null; then
      rsync -a --delete --exclude '.git' --exclude 'rust/target' \
        --exclude 'qa/screenshots' --exclude 'qa/findings' "$root/" "$tpl/"
    else
      (cd "$root" && tar -cf - --exclude=.git --exclude=rust/target \
        --exclude=qa/screenshots --exclude=qa/findings .) | (cd "$tpl" && tar -xf -)
    fi
  fi
}

start() {
  local name="$1"; shift
  local display=":21" port="8801" scene="res://demo/hex_island.tscn"
  local project="" width="1600" height="900" screen_w="1920" screen_h="1080"
  while (( $# )); do
    case "$1" in
      --display) display="$2"; shift 2 ;;
      --port) port="$2"; shift 2 ;;
      --scene) scene="$2"; shift 2 ;;
      --project) project="$2"; shift 2 ;;
      --width) width="$2"; shift 2 ;;
      --height) height="$2"; shift 2 ;;
      --screen-width) screen_w="$2"; shift 2 ;;
      --screen-height) screen_h="$2"; shift 2 ;;
      *) echo "unknown option $1" >&2; exit 2 ;;
    esac
  done
  local dir="$base_state/sessions/$name"
  if [[ -f "$dir/session.json" ]]; then
    if [[ -f "$dir/editor.pid" ]] && kill -0 "$(cat "$dir/editor.pid")" 2>/dev/null; then
      echo "session '$name' already exists and is running ($dir); stop it first" >&2
      exit 1
    fi
    echo "cleaning up stale session '$name'"
    rm -f "$dir/session.json"
    [[ -f "$dir/xvfb.pid" ]] && kill "$(cat "$dir/xvfb.pid")" 2>/dev/null || true
  fi
  mkdir -p "$dir/home/config" "$dir/home/data" "$dir/home/cache"
  copy_template
  if [[ -z "$project" ]]; then
    echo "copying project for $name ..."
    rm -rf "$dir/project"
    cp -a "$tpl" "$dir/project"
    project="$dir/project"
  fi
  # Keep the harness in the disposable copy in sync with the repo's working copy,
  # and enable it in that copy only (the repo's project.godot stays untouched).
  if [[ -d "$project/qa/harness" && "$project" != "$root" ]]; then
    cp -a "$root/qa/harness/." "$project/qa/harness/"
    python3 - "$project/project.godot" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
if 'res://qa/harness/plugin.cfg' not in text:
    text += '\n[editor_plugins]\n\nenabled=PackedStringArray("res://qa/harness/plugin.cfg")\n'
    open(path, "w", encoding="utf-8").write(text)
PY
  fi
  # Xvfb
  if [[ ! -e "/tmp/.X11-unix/X${display#:}" ]]; then
    echo "starting Xvfb on $display"
    setsid Xvfb "$display" -screen 0 "${screen_w}x${screen_h}x24" -nolisten tcp > "$dir/xvfb.log" 2>&1 < /dev/null &
    echo $! > "$dir/xvfb.pid"
    for _ in $(seq 1 50); do
      [[ -e "/tmp/.X11-unix/X${display#:}" ]] && break
      sleep 0.1
    done
  fi
  echo "launching editor ($display, port $port, scene $scene)"
  setsid env DISPLAY="$display" \
    XDG_CONFIG_HOME="$dir/home/config" XDG_DATA_HOME="$dir/home/data" XDG_CACHE_HOME="$dir/home/cache" \
    MST_QA_PORT="$port" \
    godot --editor --path "$project" --resolution "${width}x${height}" --position 0,0 --single-window "$scene" \
    > "$dir/editor.log" 2>&1 < /dev/null &
  echo $! > "$dir/editor.pid"
  python3 - "$name" "$display" "$port" "$project" "$dir" <<'PY'
import json, sys
name, display, port, project, d = sys.argv[1:]
json.dump({"name": name, "display": display, "port": int(port), "project": project, "dir": d},
          open(f"{d}/session.json", "w"), indent=1)
PY
  cat > "$dir/env.sh" <<EOF
export DISPLAY=$display
export MST_QA_PORT=$port
export MST_SESSION=$name
export MST_PROJECT=$project
EOF
  echo "waiting for harness ..."
  if python3 "$root/qa/drive.py" --display "$display" --port "$port" --timeout 180 ping > "$dir/ping.json" 2>"$dir/ping.err"; then
    python3 "$root/qa/drive.py" --display "$display" --port "$port" resize "$width" "$height" > "$dir/resize.json" 2>/dev/null || true
    sleep 2
    echo "session '$name' ready: display=$display port=$port project=$project"
    echo "  source $dir/env.sh"
  else
    echo "harness did not answer; tail of $dir/editor.log:" >&2
    tail -30 "$dir/editor.log" >&2 || true
    exit 1
  fi
}

stop() {
  local name="$1"
  local dir="$base_state/sessions/$name"
  [[ -d "$dir" ]] || { echo "no session $name" >&2; exit 1; }
  if [[ -f "$dir/editor.pid" ]]; then
    kill "$(cat "$dir/editor.pid")" 2>/dev/null || true
    sleep 1
    kill -9 "$(cat "$dir/editor.pid")" 2>/dev/null || true
  fi
  if [[ -f "$dir/xvfb.pid" ]]; then
    kill "$(cat "$dir/xvfb.pid")" 2>/dev/null || true
  fi
  rm -f "$dir/session.json"
  echo "stopped session $name"
}

status() {
  local name="$1"
  local dir="$base_state/sessions/$name"
  if [[ -f "$dir/session.json" ]]; then
    cat "$dir/session.json"
    if [[ -f "$dir/editor.pid" ]] && kill -0 "$(cat "$dir/editor.pid")" 2>/dev/null; then
      echo "editor: running (pid $(cat "$dir/editor.pid"))"
      echo "last log:"; tail -5 "$dir/editor.log" || true
    else
      echo "editor: not running"
    fi
  else
    echo "no session $name" >&2; exit 1
  fi
}

case "${1:-}" in
  start) shift; [[ $# -ge 1 ]] || usage; start "$@" ;;
  stop) shift; [[ $# -ge 1 ]] || usage; stop "$@" ;;
  status) shift; [[ $# -ge 1 ]] || usage; status "$@" ;;
  env) shift; [[ $# -ge 1 ]] || usage; cat "$base_state/sessions/$1/env.sh" ;;
  *) usage ;;
esac
