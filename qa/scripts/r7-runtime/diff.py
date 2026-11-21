#!/usr/bin/env python3
"""Pixel diff between two client-area shots: bounding box + count of changed pixels.

  diff.py A.png B.png [--thr 24] [--region x,y,w,h]

Water and clouds animate, so a diff over the world always has noise. Use --region to
restrict to a patch (e.g. the player or a tree). Prints JSON: count, bbox, per-pixel max.
"""
import json
import sys

from PIL import Image


def main():
    a, b = sys.argv[1], sys.argv[2]
    thr = 24
    region = None
    args = sys.argv[3:]
    i = 0
    while i < len(args):
        if args[i] == "--thr":
            thr = int(args[i + 1]); i += 2
        elif args[i] == "--region":
            region = [int(v) for v in args[i + 1].split(",")]; i += 2
        else:
            raise SystemExit(f"unknown {args[i]}")
    ia = Image.open(a).convert("RGB")
    ib = Image.open(b).convert("RGB")
    if region:
        x, y, w, h = region
        ia = ia.crop((x, y, x + w, y + h))
        ib = ib.crop((x, y, x + w, y + h))
    px, py = ia.load(), ib.load()
    w, h = ia.size
    n = 0
    x0 = y0 = 10 ** 9
    x1 = y1 = -1
    for yy in range(h):
        for xx in range(w):
            pa, pb = px[xx, yy], py[xx, yy]
            d = max(abs(pa[0] - pb[0]), abs(pa[1] - pb[1]), abs(pa[2] - pb[2]))
            if d >= thr:
                n += 1
                x0 = min(x0, xx); x1 = max(x1, xx)
                y0 = min(y0, yy); y1 = max(y1, yy)
    off = (region[0], region[1]) if region else (0, 0)
    print(json.dumps({
        "a": a, "b": b, "thr": thr, "region": region,
        "changed": n,
        "bbox": None if n == 0 else [x0 + off[0], y0 + off[1], x1 - x0 + 1, y1 - y0 + 1],
    }))


if __name__ == "__main__":
    main()
