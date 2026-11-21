#!/usr/bin/env python3
"""Print a compact undo-state + terrain summary from drive.py outputs.
Usage: state.sh  (run with env.sh sourced; uses `d`)
"""
import json, subprocess, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

def run(*args):
    out = subprocess.run(["python3", "qa/drive.py", *args], capture_output=True, text=True)
    try:
        return json.loads(out.stdout)
    except Exception:
        return {"raw": out.stdout[-400:], "err": out.stderr[-400:]}

u = run("undo-state")
t = run("terrain")
print("undo:", {k: u.get(k) for k in ("action", "version", "undo_count", "redo_count", "has_undo", "has_redo") if k in u})
print("terrain:", {k: t.get(k) for k in ("digest", "chunk_count", "chunks") if k in t})
