#!/usr/bin/env python3
"""List visible controls whose rect intersects a y band (and x band)."""
import json, sys
f = sys.argv[1]
y0 = float(sys.argv[2]); y1 = float(sys.argv[3])
x0 = float(sys.argv[4]) if len(sys.argv) > 4 else 0.0
x1 = float(sys.argv[5]) if len(sys.argv) > 5 else 1600.0
d = json.load(open(f))
rows = []
for c in d.get("controls", []):
    r = c.get("rect") or [0, 0, 0, 0]
    if not c.get("visible") or r[2] < 1 or r[3] < 1:
        continue
    cy = r[1] + r[3] / 2.0
    cx = r[0] + r[2] / 2.0
    if not (y0 <= cy <= y1):
        continue
    if not (x0 <= cx <= x1):
        continue
    rows.append(c)
rows.sort(key=lambda c: (round(c['rect'][1] / 4), c['rect'][0]))
for c in rows:
    t = (c.get("text", "") or "").replace("\n", "|")
    tip = (c.get("tooltip", "") or "").replace("\n", "|")
    flags = []
    if c.get("disabled"): flags.append("DIS")
    if c.get("toggle_mode"): flags.append("TOG" + ("!" if c.get("button_pressed") else ""))
    extra = ""
    if c.get("value") is not None: extra += f" val={c['value']}"
    if c.get("items"): extra += f" items={c['items'][:10]}"
    print(f"{t[:44]:44} {c.get('class','')[:16]:16} {[round(x,1) for x in c['rect']]} {' '.join(flags)}{extra} tip={tip[:26]}")
print("count", len(rows))
