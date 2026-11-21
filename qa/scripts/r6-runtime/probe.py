#!/usr/bin/env python3
"""Measure the F3 tuner dock and the F1 help panel in a runtime screenshot.

  probe.py dock SHOT.png            right-dock edges (left/right/top/bottom), width, gaps
  probe.py boxes SHOT.png           bounding boxes of dark regions (panel candidates)
  probe.py rows SHOT.png            print luminance per row band inside the dock (for scroll)

Pure PIL; no game hooks. The tuner panel is a dark translucent StyleBoxFlat
(rgba 0.06/0.07/0.09 at 0.82) so it reads as a dark, low-saturation block; the world behind is
bright pixel art. Edges are found with a full-column mean |dL| gradient, which is robust to the
animated water because the panel border is a straight full-height discontinuity.
"""
import json
import sys

from PIL import Image


def lum(px):
    r, g, b = px[:3]
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def col_scores(img, y0, y1):
    w, _ = img.size
    px = img.load()
    scores = []
    for x in range(w - 1):
        s = 0.0
        n = 0
        for y in range(y0, y1, 4):
            s += abs(lum(px[x, y]) - lum(px[x + 1, y]))
            n += 1
        scores.append(s / max(n, 1))
    return scores


def dock(args):
    path = args[0]
    img = Image.open(path).convert("RGB")
    w, h = img.size
    y0, y1 = 80, h - 16
    scores = col_scores(img, y0, y1)
    # Candidate edges: the panel's left border and the right margin at w-8.
    right_edge = w - 9  # column index just inside the margin? test both sides below
    top = sorted(range(len(scores)), key=lambda x: scores[x], reverse=True)[:8]
    out = {"shot": path, "size": [w, h], "top_edges": []}
    for x in sorted(top):
        out["top_edges"].append({"x": x, "score": round(scores[x], 1)})
    # Heuristic: strongest edge in the right half that is not the margin.
    cand = [(x, scores[x]) for x in range(w // 2, w - 2)]
    cand.sort(key=lambda t: t[1], reverse=True)
    left = None
    for x, s in cand:
        if abs(x - (w - 8)) > 3:
            left = x
            break
    if left is None:
        print(json.dumps(out))
        return
    # Panel spans [left, w-8]; find vertical extent on a clean column.
    col = left + 4
    px = img.load()
    ys = [y for y in range(h) if lum(px[col, y]) < 70]
    out.update({
        "left": left, "right": w - 8, "width": (w - 8) - left,
        "panel_top": (min(ys) if ys else None), "panel_bottom": (max(ys) if ys else None),
        "height": (max(ys) - min(ys) + 1) if ys else None,
    })
    print(json.dumps(out))


def boxes(args):
    path = args[0]
    img = Image.open(path).convert("RGB")
    w, h = img.size
    px = img.load()
    mask = [[lum(px[x, y]) < 85 for x in range(w)] for y in range(h)]
    seen = [[False] * w for _ in range(h)]
    out = []
    for y in range(0, h, 2):
        for x in range(0, w, 2):
            if not mask[y][x] or seen[y][x]:
                continue
            stack = [(x, y)]
            seen[y][x] = True
            x0 = x1 = x
            y0 = y1 = y
            n = 0
            while stack:
                cx, cy = stack.pop()
                n += 1
                x0 = min(x0, cx); x1 = max(x1, cx)
                y0 = min(y0, cy); y1 = max(y1, cy)
                for nx, ny in ((cx + 2, cy), (cx - 2, cy), (cx, cy + 2), (cx, cy - 2)):
                    if 0 <= nx < w and 0 <= ny < h and mask[ny][nx] and not seen[ny][nx]:
                        seen[ny][nx] = True
                        stack.append((nx, ny))
            if n * 4 >= 1500:
                out.append({"rect": [x0, y0, x1 - x0 + 2, y1 - y0 + 2], "area": n * 4})
    out.sort(key=lambda b: -b["area"])
    print(json.dumps({"shot": path, "size": [w, h], "boxes": out[:12]}))


def hint(args):
    """Bottom-left F1 help panel: the widest dark run per row (dark/desaturated), fitted to a
    rectangle.  Reports the modal x-extent and the y-band where those rows are."""
    path = args[0]
    img = Image.open(path).convert("RGB")
    w, h = img.size
    px = img.load()

    def dark(x, y):
        r, g, b = px[x, y]
        return lum((r, g, b)) < 64 and (max(r, g, b) - min(r, g, b)) < 42

    runs = []
    xmax = w // 2
    ystart = int(h * 0.55)
    for y in range(ystart, h):
        best = (0, 0, 0)
        x = 0
        while x < xmax:
            if dark(x, y):
                x0 = x
                while x < xmax and dark(x, y):
                    x += 1
                if x - x0 > best[2]:
                    best = (x0, x, x - x0)
            else:
                x += 1
        if best[2] > 150:
            runs.append((y, best[0], best[1]))
    if not runs:
        print(json.dumps({"shot": path, "hint": None}))
        return
    xs0 = sorted(r[1] for r in runs)
    xs1 = sorted(r[2] for r in runs)
    x0, x1 = xs0[len(xs0) // 2], xs1[len(xs1) // 2]
    ys = [r[0] for r in runs if abs(r[1] - x0) < 12]
    print(json.dumps({"shot": path, "size": [w, h],
                      "hint": {"x0": x0, "x1": x1, "y0": min(ys), "y1": max(ys),
                               "width": x1 - x0, "height": max(ys) - min(ys) + 1,
                               "rows": len(ys)}}))


def rows(args):
    path = args[0]
    x0, y0, x1, y1 = [int(v) for v in args[1:5]]
    img = Image.open(path).convert("RGB")
    px = img.load()
    bands = []
    for y in range(y0, y1, 8):
        s = 0.0
        for yy in range(y, min(y + 8, y1)):
            for x in range(x0, x1, 3):
                s += lum(px[x, yy])
        bands.append(round(s / max(1, (min(y + 8, y1) - y) * ((x1 - x0 + 2) // 3)), 1))
    print(json.dumps({"x0": x0, "y0": y0, "x1": x1, "y1": y1, "band_lum": bands}))


if __name__ == "__main__":
    if sys.argv[1] == "dock":
        dock(sys.argv[2:])
    elif sys.argv[1] == "boxes":
        boxes(sys.argv[2:])
    elif sys.argv[1] == "rows":
        rows(sys.argv[2:])
    elif sys.argv[1] == "hint":
        hint(sys.argv[2:])
    else:
        raise SystemExit(__doc__)
