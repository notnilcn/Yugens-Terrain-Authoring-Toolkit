#!/usr/bin/env python3
"""Crop a screenshot: crop.py IN OUT X Y W H [SCALE]

Used for HUD / dialog detail evidence in the r7-builder pass.
"""
import sys
from PIL import Image

src, dst = sys.argv[1], sys.argv[2]
x, y, w, h = (int(v) for v in sys.argv[3:7])
scale = float(sys.argv[7]) if len(sys.argv) > 7 else 1.0
im = Image.open(src).crop((x, y, x + w, y + h))
if scale != 1.0:
    im = im.resize((int(im.width * scale), int(im.height * scale)), Image.NEAREST)
im.save(dst)
print(dst, im.size)
