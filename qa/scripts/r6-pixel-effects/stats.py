#!/usr/bin/env python3
"""Print mean RGB of named rectangles in a PNG: stats.py IMG label:x,y,w,h ..."""
import sys
from PIL import Image

im = Image.open(sys.argv[1]).convert("RGB")
for spec in sys.argv[2:]:
    label, box = spec.split(":")
    x, y, w, h = (int(v) for v in box.split(","))
    px = list(im.crop((x, y, x + w, y + h)).getdata())
    n = len(px)
    print(f"{label}: mean=({sum(p[0] for p in px)/n:.1f},{sum(p[1] for p in px)/n:.1f},{sum(p[2] for p in px)/n:.1f})")
