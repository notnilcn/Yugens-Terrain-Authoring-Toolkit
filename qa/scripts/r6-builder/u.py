#!/usr/bin/env python3
"""Compact UI inspector for the r6-e2 builder pass.

Runs `qa/drive.py ui --save` and prints one line per visible control:
  name | rect | text | tooltip

Usage:
  u.py [--root all|scene] [--filter TEXT] [--min-w N] [--full]
"""
import json
import subprocess
import sys
import tempfile
import os

args = sys.argv[1:]
root = "all"
filt = "PixelBuilder"
min_w = 0
full = "--full" in args
i = 0
while i < len(args):
    if args[i] == "--root":
        root = args[i + 1]; i += 2
    elif args[i] == "--filter":
        filt = args[i + 1]; i += 2
    elif args[i] == "--min-w":
        min_w = int(args[i + 1]); i += 2
    else:
        i += 1

out = tempfile.mktemp(suffix=".json")
cmd = ["python3", "qa/drive.py", "ui", "--root", root, "--filter", filt, "--save", out]
subprocess.run(cmd, cwd="/home/clintonnguyen/g/code_examples/rrr-wt/r4base", check=True,
               stdout=subprocess.DEVNULL)
data = json.load(open(out))
os.unlink(out)
for c in data.get("controls", []):
    if not c.get("visible"):
        continue
    w = c["rect"][2]
    if w < min_w:
        continue
    r = ",".join(str(int(v)) for v in c["rect"])
    txt = (c.get("text") or "").replace("\n", "\\n")[:60]
    tip = (c.get("tooltip") or "").replace("\n", "\\n")[:60]
    extra = ""
    if full:
        extra = f" min={c.get('min')} cm={c.get('custom_min')} clipped={c.get('clipped')} disabled={c.get('disabled')} items={c.get('items')}"
    print(f"{c['name']:38s} [{r}] {txt!r} {tip!r}{extra}")
