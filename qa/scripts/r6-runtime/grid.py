#!/usr/bin/env python3
"""Integer-pixel-grid check for the pixel renderer.

  grid.py SHOT.png X Y W H [--label NAME]

Reports, for the patch, the horizontal run-length histogram of exactly equal RGB pixels and the
fraction of "intermediate" colours (nearest-neighbour upscales have none: every edge is a hard
step).  A crisp N-px grid shows a run-length mode at N (or a multiple, inside flat areas).
"""
import json
import sys

from PIL import Image


def main():
    path, x, y, w, h = sys.argv[1], *[int(v) for v in sys.argv[2:6]]
    label = ""
    if "--label" in sys.argv:
        label = sys.argv[sys.argv.index("--label") + 1]
    img = Image.open(path).convert("RGB")
    px = img.load()
    runs = {}
    total = 0
    for yy in range(y, y + h):
        x0 = x
        while x0 < x + w:
            x1 = x0 + 1
            c = px[x0, yy]
            while x1 < x + w and px[x1, yy] == c:
                x1 += 1
            runs[x1 - x0] = runs.get(x1 - x0, 0) + 1
            total += 1
            x0 = x1
    hist = sorted(runs.items(), key=lambda kv: -kv[1])[:8]
    print(json.dumps({"shot": path, "label": label, "patch": [x, y, w, h],
                      "runs": total, "hist": hist}))


if __name__ == "__main__":
    main()
