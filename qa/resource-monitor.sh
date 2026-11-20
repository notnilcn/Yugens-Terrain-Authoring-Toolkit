#!/usr/bin/env bash
# Wave guard for parallel QA subagents, using the shared ai-tools resource gate.
#
#   qa/resource-monitor.sh init [--cpu-percent N --ram-percent N --disk-percent N --streak N]
#   qa/resource-monitor.sh start        # detached JSON monitor, logs to $MST_QA_STATE/rmon
#   qa/resource-monitor.sh once         # one sample (prints the JSON line)
#   qa/resource-monitor.sh status       # last sample + whether the monitor is running
#   qa/resource-monitor.sh parallel     # recommended number of concurrent subagents
#   qa/resource-monitor.sh stop
#
# The tool itself is not vendored: it is read from the ai-tools checkout (the
# repository's `origin/linux` branch, which carries the bash port). Set
# MST_AI_TOOLS to that checkout, or clone it at ../Yugens-Terrain-Authoring-Toolkit/tools/ai-tools.
#
# The recommendation only counts unqueued work; it is a ceiling for a wave, not a
# promise. Read it again before every wave.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
state="${MST_QA_STATE:-/tmp/opencode/qa}"
rmon_dir="$state/rmon"
tools_dir="$state/rmon-tools"
tools="${MST_AI_TOOLS:-}"

usage() { sed -n '2,18p' "$0"; exit 2; }
[[ $# -ge 1 ]] || usage
cmd="$1"; shift

find_tools() {
  local candidates=()
  [[ -n "$tools" ]] && candidates+=("$tools")
  candidates+=(
    "$root/../Yugens-Terrain-Authoring-Toolkit/tools/ai-tools"
    "$root/../rr/tools/ai-tools"
    "$root/tools/ai-tools"
  )
  for d in "${candidates[@]}"; do
    if [[ -d "$d/.git" || -d "$d/resource-gate" ]]; then
      tools="$(cd "$d" && pwd)"; return 0
    fi
  done
  return 1
}

install_tool() {
  mkdir -p "$tools_dir" "$rmon_dir"
  if [[ ! -f "$tools_dir/resource-monitor.sh" ]]; then
    if ! find_tools; then
      echo "resource-monitor: cannot find the ai-tools checkout." >&2
      echo "  set MST_AI_TOOLS=/path/to/tools/ai-tools (or clone it beside this repo)." >&2
      exit 2
    fi
    if [[ -f "$tools/resource-gate/resource-monitor.sh" ]]; then
      cp "$tools/resource-gate/resource-monitor.sh" "$tools_dir/resource-monitor.sh"
    else
      # The default branch is the Windows port; the linux branch has the bash scripts.
      git -C "$tools" show origin/linux:resource-gate/resource-monitor.sh > "$tools_dir/resource-monitor.sh"
    fi
    chmod +x "$tools_dir/resource-monitor.sh"
    echo "resource-monitor: installed $tools_dir/resource-monitor.sh (from $tools)"
  fi
}

export RESOURCE_GATE_REPO_ROOT="$root"
export RESOURCE_MONITOR_DIR="$rmon_dir"

case "$cmd" in
  init)
    install_tool
    "$tools_dir/resource-monitor.sh" --init \
      --cpu-percent "${MST_CPU_PERCENT:-85}" \
      --ram-percent "${MST_RAM_PERCENT:-80}" \
      --disk-percent "${MST_DISK_PERCENT:-90}" \
      --streak "${MST_STREAK:-3}" \
      --state-dir "$rmon_dir"
    ;;
  start)
    install_tool
    if [[ -f "$rmon_dir/monitor.pid" ]] && kill -0 "$(cat "$rmon_dir/monitor.pid")" 2>/dev/null; then
      echo "resource-monitor: already running (pid $(cat "$rmon_dir/monitor.pid"))"
      exit 0
    fi
    mkdir -p "$rmon_dir"
    setsid "$tools_dir/resource-monitor.sh" --json --state-dir "$rmon_dir" \
      --log "$rmon_dir/monitor.log" \
      > "$rmon_dir/out.log" 2>&1 < /dev/null &
    echo $! > "$rmon_dir/monitor.pid"
    sleep 2
    tail -2 "$rmon_dir/monitor.log" 2>/dev/null || true
    ;;
  once)
    install_tool
    "$tools_dir/resource-monitor.sh" --once --json --state-dir "$rmon_dir"
    ;;
  status)
    if [[ -f "$rmon_dir/monitor.pid" ]] && kill -0 "$(cat "$rmon_dir/monitor.pid")" 2>/dev/null; then
      echo "monitor: running (pid $(cat "$rmon_dir/monitor.pid"))"
    else
      echo "monitor: not running"
    fi
    tail -1 "$rmon_dir/monitor.log" 2>/dev/null || echo "no samples yet"
    ;;
  parallel)
    line="$(grep '^{' "$rmon_dir/monitor.log" 2>/dev/null | tail -1 || true)"
    if [[ -z "$line" ]]; then
      "$tools_dir/resource-monitor.sh" --once --json --state-dir "$rmon_dir" 2>/dev/null \
        | python3 -c 'import json,sys; print(json.load(sys.stdin)["recommended_parallel_subagents"])'
    else
      printf '%s\n' "$line" | python3 -c 'import json,sys; print(json.load(sys.stdin)["recommended_parallel_subagents"])'
    fi
    ;;
  stop)
    if [[ -f "$rmon_dir/monitor.pid" ]]; then
      kill "$(cat "$rmon_dir/monitor.pid")" 2>/dev/null || true
      rm -f "$rmon_dir/monitor.pid"
      echo "resource-monitor: stopped"
    else
      echo "resource-monitor: no pid file"
    fi
    ;;
  *) usage ;;
esac
