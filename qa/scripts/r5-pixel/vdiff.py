#!/usr/bin/env python3
"""Compare two captures, optionally inside a rect: vdiff.py A.png B.png [x y w h]
Prints changed-pixel count, mean RGB of each image (in the rect), and max channel delta."""
import sys
from PIL import Image, ImageChops


def main():
    a = Image.open(sys.argv[1]).convert("RGB")
    b = Image.open(sys.argv[2]).convert("RGB")
    if len(sys.argv) >= 7:
        x, y, w, h = map(int, sys.argv[3:7])
        box = (x, y, x + w, y + h)
        a = a.crop(box)
        b = b.crop(box)
    if a.size != b.size:
        print(f"size mismatch {a.size} vs {b.size}")
        return
    diff = ImageChops.difference(a, b)
    bbox = diff.getbbox()
    changed = 0
    if bbox:
        # count pixels differing at all
        gray = diff.convert("L")
        hist = gray.histogram()
        changed = sum(hist[1:])
    n = a.size[0] * a.size[1]

    def mean(img):
        px = list(img.getdata())
        return tuple(round(sum(p[i] for p in px) / len(px), 1) for i in range(3))

    ex = diff.getextrema()
    print(f"region={a.size[0]}x{a.size[1]} changed={changed}/{n} ({100.0 * changed / n:.1f}%)")
    print(f"meanA={mean(a)} meanB={mean(b)} maxdelta={[e[1] for e in ex]}")
    if bbox:
        print(f"bbox={bbox}")


if __name__ == "__main__":
    main()
