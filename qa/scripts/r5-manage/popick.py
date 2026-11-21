#!/usr/bin/env python3
"""From `drive.py popups` JSON on stdin, print the center of item INDEX of popup PIDX.
Usage: popick.py INDEX [PIDX]  < popups.json"""
import json, sys
idx = int(sys.argv[1])
pidx = int(sys.argv[2]) if len(sys.argv) > 2 else 0
d = json.load(sys.stdin)
p = d["popups"][pidx]
pos = p["pos"]; size = p["size"]; n = len(p["items"])
h = size[1] / n
x = pos[0] + size[0] // 2
y = int(pos[1] + h * (idx + 0.5))
print(x, y)
for i, it in enumerate(p["items"]):
    print(f"  [{i}] {it['text']!r} checked={it['checked']} y={int(pos[1]+h*(i+0.5))}", file=sys.stderr)
