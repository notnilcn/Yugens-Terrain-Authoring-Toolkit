#!/usr/bin/env python3
"""Diff two PNGs: changed pixel count, bbox, mean colour of each (optionally a crop region)."""
import sys
from PIL import Image, ImageChops

a_path, b_path = sys.argv[1], sys.argv[2]
region = None
if len(sys.argv) > 3:
    region = tuple(int(v) for v in sys.argv[3].split(","))

a = Image.open(a_path).convert("RGB")
b = Image.open(b_path).convert("RGB")
if region:
    x, y, w, h = region
    a = a.crop((x, y, x + w, y + h))
    b = b.crop((x, y, x + w, y + h))

diff = ImageChops.difference(a, b)
bbox = diff.getbbox()
changed = sum(1 for px in diff.getdata() if px != (0, 0, 0))
total = a.size[0] * a.size[1]


def mean(img):
    px = list(img.getdata())
    n = len(px)
    return tuple(round(sum(p[i] for p in px) / n, 1) for i in range(3))


print(f"changed={changed}/{total} ({100.0*changed/total:.1f}%) bbox={bbox}")
print(f"mean_a={mean(a)} mean_b={mean(b)}")
if bbox:
    # tightened bbox: ignore isolated 1-2 px diffs by thresholding
    import numpy as np
    ar = np.asarray(diff).sum(axis=2)
    for thr in (10, 40, 90):
        ys, xs = np.where(ar > thr)
        if len(xs):
            print(f"thr>{thr}: {len(xs)} px bbox=({xs.min()},{ys.min()})-({xs.max()},{ys.max()})")
