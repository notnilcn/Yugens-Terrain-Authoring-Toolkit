#!/usr/bin/env python3
"""Print the builder layout rects (viewport, strip, palette, rail) in one line each.

Usage: m.py            (window-content px; editor scale 1.0)
"""
import json, subprocess, tempfile, os

out = tempfile.mktemp(suffix=".json")
subprocess.run(["python3", "qa/drive.py", "ui", "--root", "all", "--filter", "", "--save", out],
               cwd="/home/clintonnguyen/g/code_examples/rrr-wt/r4base", check=True,
               stdout=subprocess.DEVNULL)
d = json.load(open(out)); os.unlink(out)
want = {
    "viewport": "@Node3DEditorViewportContainer@10229",
    "strip_root": "PixelBuilderStripRoot",
    "strip": "PixelBuilderStrip",
    "palette": "PixelBuilderPalette",
    "rail": "PixelBuilderToolbarRail",
    "rail_content": "PixelBuilderToolbar",
}
seen = {}
for c in d["controls"]:
    n = c["name"]
    if n in want.values() and c.get("visible") and n not in seen:
        seen[n] = c
for key, name in want.items():
    c = seen.get(name)
    if not c:
        print(f"{key:13s} (absent)")
        continue
    x, y, w, h = [int(v) for v in c["rect"]]
    print(f"{key:13s} [{x},{y},{w},{h}]")
# strip page tabs (names that look like tabs)
tabs = [c for c in d["controls"] if c.get("visible") and c["name"] in
        ("Face", "Edge", "Vertex", "Object", "Extrude", "Ops", "Move", "Paint", "Shape", "Select")]
print("tabs:", ", ".join(f"{t['name']}@{int(t['rect'][0])},{int(t['rect'][1])} {int(t['rect'][2])}x{int(t['rect'][3])}"
                         for t in tabs))
