#!/usr/bin/env python3
"""Dump visible Controls under a path substring as an indented tree (rect + text).

Usage: tree.py [--root all] [--path SubStr] [--min-w N] [--max-depth N]
Defaults: --root all --path PixelBuilder
"""
import json, subprocess, sys, tempfile, os

args = sys.argv[1:]
root = "all"; pathsub = "PixelBuilder"; min_w = 0; max_depth = 99
i = 0
while i < len(args):
    if args[i] == "--root": root = args[i + 1]; i += 2
    elif args[i] == "--path": pathsub = args[i + 1]; i += 2
    elif args[i] == "--min-w": min_w = int(args[i + 1]); i += 2
    elif args[i] == "--max-depth": max_depth = int(args[i + 1]); i += 2
    else: i += 1

out = tempfile.mktemp(suffix=".json")
subprocess.run(["python3", "qa/drive.py", "ui", "--root", root, "--filter", "", "--save", out],
               cwd="/home/clintonnguyen/g/code_examples/rrr-wt/r4base", check=True,
               stdout=subprocess.DEVNULL)
data = json.load(open(out))
os.unlink(out)
ctrls = [c for c in data.get("controls", []) if c.get("visible") and pathsub in c.get("path", "")]

# Build parent/child from path prefixes.
by_path = {c["path"]: c for c in ctrls}
def parent(c):
    p = c["path"]
    while "/" in p:
        p = p.rsplit("/", 1)[0]
        if p in by_path:
            return by_path[p]
    return None

roots = []
for c in ctrls:
    if parent(c) is None:
        roots.append(c)

def emit(c, depth):
    if depth > max_depth: return
    if c["rect"][2] < min_w: return
    r = ",".join(str(int(v)) for v in c["rect"])
    t = (c.get("text") or "").replace("\n", "\\n")[:40]
    tip = (c.get("tooltip") or "").replace("\n", "\\n")[:40]
    print(f"{'  '*depth}{c['name'][:32]:32s} [{r}] {t!r} {tip!r}")
    for ch in ctrls:
        if parent(ch) is c:
            emit(ch, depth + 1)

for r in roots:
    emit(r, 0)
