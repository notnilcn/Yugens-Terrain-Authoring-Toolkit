#!/usr/bin/env python3
"""Print the Control(s) that report keyboard focus (ui --root all, focus=true)."""
import json, subprocess, tempfile, os

out = tempfile.mktemp(suffix=".json")
subprocess.run(["python3", "qa/drive.py", "ui", "--root", "all", "--filter", "", "--save", out],
               cwd="/home/clintonnguyen/g/code_examples/rrr-wt/r4base", check=True,
               stdout=subprocess.DEVNULL)
d = json.load(open(out)); os.unlink(out)
hits = [c for c in d["controls"] if c.get("focus")]
if not hits:
    print("(no focus owner reported)")
for c in hits:
    print(f"{c['class']:20s} {c['name']!r} rect={[int(v) for v in c['rect']]}\n  {c['path']}")
