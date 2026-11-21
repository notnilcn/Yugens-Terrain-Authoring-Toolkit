#!/usr/bin/env python3
"""Click a visible editor Control by exact `name` (or substring with --contains).

Usage: python3 clickctl.py ModeFace [--contains] [--mod ctrl] [--dry-run]
"""
import argparse
import json
import os
import subprocess
import sys

BUILD = "/home/clintonnguyen/g/code_examples/rrr-wt/r4base"
TMP = "/tmp/opencode/qa/_r5_t4_ui_dump.json"


def dump():
    subprocess.run([sys.executable, "qa/drive.py", "ui", "--root", "all", "--save", TMP],
                   cwd=BUILD, capture_output=True)
    return json.load(open(TMP))


def find(controls, name, contains):
    hits = []
    for c in controls:
        if not c.get("visible"):
            continue
        n = c.get("name") or ""
        if (name in n) if contains else (n == name):
            hits.append(c)
    return hits


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("name")
    ap.add_argument("--contains", action="store_true")
    ap.add_argument("--mod", action="append", default=[])
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()
    d = dump()
    hits = find(d["controls"], a.name, a.contains)
    if not hits:
        print(json.dumps({"ok": False, "error": "no visible control named %r" % a.name}))
        sys.exit(1)
    if len(hits) > 1:
        print(json.dumps({"ok": False, "error": "ambiguous", "matches": [
            {"name": c["name"], "rect": c["rect"]} for c in hits[:8]]}))
        sys.exit(2)
    c = hits[0]
    r = c["rect"]
    x, y = int(r[0] + r[2] / 2), int(r[1] + r[3] / 2)
    print("target", c["name"], c["class"], r, "->", x, y)
    if a.dry_run:
        return
    cmd = [sys.executable, "qa/drive.py", "click", str(x), str(y)]
    for m in a.mod:
        cmd += ["--mod", m]
    res = subprocess.run(cmd, cwd=BUILD, capture_output=True, text=True)
    print(res.stdout.strip() or res.stderr.strip())


if __name__ == "__main__":
    main()
