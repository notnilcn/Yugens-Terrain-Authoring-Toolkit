#!/usr/bin/env python3
"""Find bright/warm spots across frames.

brightspots.py OUT.png IN1.png [IN2.png ...] [--thresh 150] [--warm]
Prints, per frame, count of pixels brighter than thresh (and warm-biased if
--warm), their bbox, and writes OUT.png = max-brightness composite.
"""
import sys
from PIL import Image, ImageChops


def main():
    args = sys.argv[1:]
    thresh = 150
    warm = False
    rest = []
    i = 0
    while i < len(args):
        if args[i] == "--thresh":
            thresh = int(args[i + 1])
            i += 2
            continue
        if args[i] == "--warm":
            warm = True
            i += 1
            continue
        rest.append(args[i])
        i += 1
    out, files = rest[0], rest[1:]
    imgs = [Image.open(f).convert("RGB") for f in files]
    w, h = imgs[0].size
    best = Image.new("RGB", (w, h), (0, 0, 0))
    count = 0
    coords = []
    for x in range(w):
        for y in range(h):
            rmax = gmax = bmax = 0
            for im in imgs:
                r, g, b = im.getpixel((x, y))
                if r > rmax:
                    rmax, gmax, bmax = r, g, b
            lum = 0.299 * rmax + 0.587 * gmax + 0.114 * bmax
            ok = lum > thresh
            if warm:
                ok = ok and rmax > gmax + 10 and gmax > bmax
            if ok:
                count += 1
                coords.append((x, y))
            best.putpixel((x, y), (rmax, gmax, bmax))
    best.save(out)
    print(f"frames={len(imgs)} hot={count} thresh={thresh} warm={warm}")
    if coords:
        xs = [c[0] for c in coords]
        ys = [c[1] for c in coords]
        print(f"bbox=({min(xs)},{min(ys)})-({max(xs)},{max(ys)})")
        # crude clusters: print up to 20 sample coords
        print("sample:", coords[:20])


if __name__ == "__main__":
    main()
