#!/usr/bin/env python3
"""Drag the green Y scale handle of the selected 3D node, found at run time.

The 3D manipulator sits a fixed number of *screen* pixels from the node origin, so a
hard-coded start point drifts when the camera framing changes. This helper projects the
node origin, screenshots the editor, finds the green (+Y) handle square, and drags it.

    python3 docs/guide/recipes/_helpers/gizmo-y-drag.py --path round_tree/Tree --dy -135

Prints the projected origin, the handle centre and the node scale before/after. Env:
DISPLAY, PORT, OUT (recapture sets all three).
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile

from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))))
DRIVE = [sys.executable, os.path.join(ROOT, "qa", "drive.py")]
DISPLAY = os.environ.get("DISPLAY", ":79")
PORT = os.environ.get("PORT") or os.environ.get("MST_QA_PORT") or "8879"


def drive(*args):
    p = subprocess.run(DRIVE + ["--display", DISPLAY, "--port", PORT] + [str(a) for a in args],
                       cwd=ROOT, capture_output=True, text=True)
    try:
        return json.loads(p.stdout)
    except Exception:
        return {}


def is_handle(px):
    r, g, b = px
    return g >= 180 and b <= 20 and 90 <= r <= 160


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--path", required=True)
    ap.add_argument("--dy", type=float, default=-135.0)
    ap.add_argument("--probe", default=None)
    ap.add_argument("--band", type=int, default=60)
    a = ap.parse_args()

    pos = drive("prop", "global_position", "--path", a.path).get("value")
    if pos is None:
        print(json.dumps({"ok": False, "error": "no global_position"}))
        return 1
    proj = drive("project", "--point", json.dumps(pos)).get("points", [{}])[0]
    ox, oy = proj.get("window", [None, None])
    if ox is None:
        print(json.dumps({"ok": False, "error": "origin not projectable"}))
        return 1
    rect = drive("view3d").get("rect")
    probe = a.probe or os.path.join(tempfile.gettempdir(), "mst-gizmo-probe.png")
    os.makedirs(os.path.dirname(probe) or ".", exist_ok=True)
    drive("capture", probe)
    im = Image.open(probe).convert("RGB")
    x0, y0, x1, y1 = [int(v) for v in (rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3])]

    rows = {}
    for y in range(max(y0, 0), min(y1, im.height)):
        xs = [x for x in range(max(x0, int(ox) - a.band), min(x1, int(ox) + a.band))
              if is_handle(im.getpixel((x, y)))]
        if len(xs) >= 8:
            rows[y] = xs
    groups = []
    for y in sorted(rows):
        if groups and y - groups[-1][-1] <= 2:
            groups[-1].append(y)
        else:
            groups.append([y])
    above = [g for g in groups if (g[0] + g[-1]) / 2 < oy]
    if not above:
        print(json.dumps({"ok": False, "error": "green Y handle not found",
                          "origin": [ox, oy], "groups": groups}))
        return 1
    g = max(above, key=lambda gg: (gg[0] + gg[-1]) / 2)
    ys = rows[g[len(g) // 2]]
    hx, hy = int(sum(ys) / len(ys)), (g[0] + g[-1]) // 2

    before = drive("prop", "scale", "--path", a.path).get("value")
    drive("drag", hx, hy, hx, int(hy + a.dy), "--duration", "1.6")
    after = drive("prop", "scale", "--path", a.path).get("value")
    if not a.probe:
        try:
            os.unlink(probe)
        except OSError:
            pass
    print(json.dumps({"ok": True, "origin": [ox, oy], "handle": [hx, hy],
                      "scale_before": before, "scale_after": after, "probe": probe}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
