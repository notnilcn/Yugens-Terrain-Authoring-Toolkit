#!/usr/bin/env python3
"""Flat-band / void scan for the sea horizon (#38).

  sea.py SHOT.png [--from 0.4] [--tol 6] [--frac 0.9]

For every row from --from*h to the bottom, find the modal colour (6-bit quantised) and the
fraction of pixels within +-tol of it. A row is "flat" when that fraction >= --frac. Prints
each contiguous flat band ([y0,y1], height, colour) and the total flat height as a fraction
of the window. A straight sea-plane edge with a pale void below it shows up as one tall flat
band; a normal sea horizon shows a few rows of sky/water gradient only.
"""
import json
import sys
from collections import Counter

from PIL import Image


def main():
    path = sys.argv[1]
    start = 0.40
    tol = 6
    frac = 0.90
    args = sys.argv[2:]
    i = 0
    while i < len(args):
        if args[i] == "--from":
            start = float(args[i + 1]); i += 2
        elif args[i] == "--tol":
            tol = int(args[i + 1]); i += 2
        elif args[i] == "--frac":
            frac = float(args[i + 1]); i += 2
        else:
            raise SystemExit(f"unknown {args[i]}")
    img = Image.open(path).convert("RGB")
    w, h = img.size
    px = img.load()
    rows = []
    y0 = int(h * start)
    for y in range(y0, h):
        cnt = Counter()
        for x in range(0, w, 2):
            r, g, b = px[x, y]
            cnt[(r >> 2, g >> 2, b >> 2)] += 1
        (qr, qg, qb), n = cnt.most_common(1)[0]
        col = (qr * 4 + 2, qg * 4 + 2, qb * 4 + 2)
        ok = 0
        for x in range(0, w, 2):
            r, g, b = px[x, y]
            if abs(r - col[0]) <= tol and abs(g - col[1]) <= tol and abs(b - col[2]) <= tol:
                ok += 1
        total = len(range(0, w, 2))
        rows.append((ok / total >= frac, col, ok / total))
    bands = []
    y = 0
    while y < len(rows):
        if rows[y][0]:
            j = y
            while j < len(rows) and rows[j][0] and rows[j][1] == rows[y][1]:
                j += 1
            if j - y >= 3:
                bands.append({"y0": y0 + y, "y1": y0 + j - 1, "height": j - y,
                              "colour": list(rows[y][1]),
                              "flat_fraction": round(rows[y][2], 3)})
            y = j
        else:
            y += 1
    flat = sum(1 for r in rows if r[0])
    print(json.dumps({"shot": path, "size": [w, h], "tol": tol, "frac": frac,
                      "bands": bands,
                      "flat_rows": flat,
                      "flat_fraction_of_window": round(flat / h, 3)}))


if __name__ == "__main__":
    main()
