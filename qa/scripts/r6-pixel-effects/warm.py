#!/usr/bin/env python3
"""Count warm (firefly-ish) pixels in a region of a PNG."""
import sys
from PIL import Image

im = Image.open(sys.argv[1]).convert("RGB")
x, y, w, h = (int(v) for v in sys.argv[2].split(","))
px = list(im.crop((x, y, x + w, y + h)).getdata())
warm = sum(1 for (r, g, b) in px if r > 150 and g > 90 and b < 110)
hot = sum(1 for (r, g, b) in px if r > 200 and g > 150 and b < 140)
print(f"warm={warm} hot={hot} of {len(px)}")
