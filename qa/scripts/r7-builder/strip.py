#!/usr/bin/env python3
"""Report the builder strip: root rect, section tabs (toggled/visible), and the
visible text-bearing controls inside the strip body.

Usage: strip.py   (window-content px from `drive.py ui --root all`)
"""
import json, subprocess, tempfile, os

out = tempfile.mktemp(suffix=".json")
subprocess.run(["python3", "qa/drive.py", "ui", "--root", "all", "--filter", "",
                "--save", out],
               cwd="/home/clintonnguyen/g/code_examples/rrr-wt/r4base", check=True,
               stdout=subprocess.DEVNULL)
d = json.load(open(out)); os.unlink(out)

root = next((c for c in d["controls"]
             if c["name"] == "PixelBuilderStripRoot" and c.get("visible")), None)
if not root:
    print("strip absent"); raise SystemExit
rx, ry, rw, rh = [int(v) for v in root["rect"]]
print(f"strip_root [{rx},{ry},{rw},{rh}]")
tabs = [c for c in d["controls"]
        if c.get("visible") and c["name"] in ("Extrude", "Ops", "Move", "Paint")
        and abs(c["rect"][1] - ry) < 40 and c["rect"][0] >= rx]
print("tabs:", ", ".join(
    f"{t['name']}{'*' if t.get('toggled') else ''}" for t in tabs))
inside = []
for c in d["controls"]:
    if not c.get("visible"):
        continue
    x, y, w, h = c["rect"]
    if w <= 0 or h <= 0:
        continue
    if x < rx or y < ry - 1 or y > ry + rh + 1:
        continue
    if x > rx + rw:
        continue
    txt = (c.get("text") or "").strip()
    tip = (c.get("tooltip") or "").strip()
    label = txt or tip
    if not label:
        continue
    inside.append((int(y), int(x), c["class"], label[:38]))
inside.sort()
seen = set()
for y, x, cls, label in inside:
    k = (y, x, label)
    if k in seen:
        continue
    seen.add(k)
    print(f"  y={y:4d} x={x:4d} {cls:22s} {label!r}")
