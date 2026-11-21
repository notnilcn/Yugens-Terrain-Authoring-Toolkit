#!/usr/bin/env bash
# Plain (non-editor) game session for runtime testing.
#
#   qa/runtime.sh start <name> [--display :44] [--scene res://demo/hex_island.tscn]
#                                [--width 1280] [--height 720] [--project DIR]
#   qa/runtime.sh stop <name>
#   qa/runtime.sh env <name>
#
# Runs the project from a disposable copy of $MST_QA_STATE/project_base with
# isolated XDG directories, on its own Xvfb display. Screenshots and input use
# qa/drive.py --window-match "Marching Squares Terrain" (the env file sets it).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
state="${MST_QA_STATE:-/tmp/opencode/qa}"
base="$state/runtime"

usage() { sed -n '2,11p' "$0"; exit 2; }

start() {
  local name="$1"; shift
  local display=":44" scene="res://demo/hex_island.tscn" width="1280" height="720" project=""
  while (( $# )); do
    case "$1" in
      --display) display="$2"; shift 2 ;;
      --scene) scene="$2"; shift 2 ;;
      --width) width="$2"; shift 2 ;;
      --height) height="$2"; shift 2 ;;
      --project) project="$2"; shift 2 ;;
      *) echo "unknown option $1" >&2; exit 2 ;;
    esac
  done
  local dir="$base/$name"
  if [[ -f "$dir/session.json" ]]; then
    echo "runtime '$name' already exists; stop it first" >&2; exit 1
  fi
  mkdir -p "$dir/home/config" "$dir/home/data" "$dir/home/cache"
  if [[ -z "$project" ]]; then
    project="$base/project"
    if [[ ! -d "$project" ]]; then
      [[ -d "$state/project_base" ]] || { echo "run qa/setup.sh first" >&2; exit 1; }
      echo "copying runtime project ..."
      cp -a "$state/project_base" "$project"
    fi
  fi
  # project_base excludes .godot, so a freshly copied project has no import cache and the
  # GDExtension classes do not register (the scene loads with no addon). Import once.
  if [[ ! -d "$project/.godot" ]]; then
    echo "importing runtime project (no .godot cache) ..."
    godot --headless --path "$project" --import > "$dir/import.log" 2>&1 || true
  fi
  if [[ ! -e "/tmp/.X11-unix/X${display#:}" ]]; then
    setsid Xvfb "$display" -screen 0 "1920x1080x24" -nolisten tcp \
      > "$dir/xvfb.log" 2>&1 < /dev/null &
    echo $! > "$dir/xvfb.pid"
    for _ in $(seq 1 50); do
      [[ -e "/tmp/.X11-unix/X${display#:}" ]] && break
      sleep 0.1
    done
  fi
  setsid env DISPLAY="$display" \
    XDG_CONFIG_HOME="$dir/home/config" XDG_DATA_HOME="$dir/home/data" XDG_CACHE_HOME="$dir/home/cache" \
    godot --path "$project" --resolution "${width}x${height}" --position 0,0 "$scene" \
    > "$dir/game.log" 2>&1 < /dev/null &
  echo $! > "$dir/game.pid"
  python3 - "$name" "$display" "$project" "$dir" "$scene" "$width" "$height" <<'PY'
import json, sys
name, display, project, d, scene, w, h = sys.argv[1:]
json.dump({"name": name, "display": display, "project": project, "dir": d,
           "scene": scene, "width": int(w), "height": int(h)},
          open(f"{d}/session.json", "w"), indent=1)
PY
  cat > "$dir/env.sh" <<EOF
export DISPLAY=$display
export MST_WINDOW_MATCH="Marching Squares Terrain"
export MST_RUNTIME=$name
export MST_PROJECT=$project
EOF
  sleep 6
  echo "runtime '$name' on $display (${width}x${height}) — source $dir/env.sh"
  tail -6 "$dir/game.log" 2>/dev/null || true
}

stop() {
  local name="$1"; local dir="$base/$name"
  [[ -d "$dir" ]] || { echo "no runtime $name" >&2; exit 1; }
  if [[ -f "$dir/game.pid" ]]; then
    kill "$(cat "$dir/game.pid")" 2>/dev/null || true
    sleep 1
    kill -9 "$(cat "$dir/game.pid")" 2>/dev/null || true
  fi
  if [[ -f "$dir/xvfb.pid" ]]; then
    kill "$(cat "$dir/xvfb.pid")" 2>/dev/null || true
  fi
  rm -f "$dir/session.json"
  echo "stopped runtime $name"
}

case "${1:-}" in
  start) shift; [[ $# -ge 1 ]] || usage; start "$@" ;;
  stop) shift; [[ $# -ge 1 ]] || usage; stop "$@" ;;
  env) shift; [[ $# -ge 1 ]] || usage; cat "$base/$1/env.sh" ;;
  *) usage ;;
esac
