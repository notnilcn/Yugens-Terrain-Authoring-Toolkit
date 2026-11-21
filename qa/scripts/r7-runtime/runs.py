#!/usr/bin/env python3
"""Dark-panel runs down a column / edge profiles for runtime shots.

  runs.py SHOT.png X [--dark 90] [--min 4]
      Contiguous dark runs (luminance < dark) down column X: [y0, y1, height].

  runs.py SHOT.png row Y --x0 A --x1 B
      Dark run along row Y: [x0, x1, width] (panel horizontal extent at that row).

Used for dock bottoms vs the F1 help top, which probe.py merges when both are dark.
"""
import json
import sys

from PIL import Image


def lum(p):
    r, g, b = p[:3]
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def dark_runs(px, coords, dark, minimum, start=0):
    out = []
    i = 0
    n = len(coords)
    while i < n:
        if lum(px[coords[i]]) < dark:
            j = i
            while j < n and lum(px[coords[j]]) < dark:
                j += 1
            if j - i >= minimum:
                out.append([start + i, start + j - 1, j - i])
            i = j
        else:
            i += 1
    return out


def main():
    path = sys.argv[1]
    axis = sys.argv[2]
    value = int(sys.argv[3])
    dark = 90
    minimum = 3
    args = sys.argv[4:]
    x0 = x1 = None
    i = 0
    while i < len(args):
        if args[i] == "--dark":
            dark = int(args[i + 1]); i += 2
        elif args[i] == "--min":
            minimum = int(args[i + 1]); i += 2
        elif args[i] == "--x0":
            x0 = int(args[i + 1]); i += 2
        elif args[i] == "--x1":
            x1 = int(args[i + 1]); i += 2
        else:
            raise SystemExit(f"unknown arg {args[i]}")
    img = Image.open(path).convert("RGB")
    px = img.load()
    w, h = img.size
    if axis == "col":
        coords = [(value, y) for y in range(h)]
        label = "y"
        start = 0
    else:
        a = 0 if x0 is None else x0
        b = w if x1 is None else x1
        coords = [(x, value) for x in range(a, b)]
        label = "x"
        start = a
    print(json.dumps({"shot": path, "axis": label, "at": value, "dark": dark,
                      "runs": dark_runs(px, coords, dark, minimum, start)}))


if __name__ == "__main__":
    main()
