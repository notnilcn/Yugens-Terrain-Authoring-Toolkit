#!/usr/bin/env python3
"""Screenshot a running game window found on the X display.

    python3 docs/guide/recipes/_helpers/shot-game.py --title "Marching Squares Terrain (DEBUG)" \
        --out 40-window-1280x720.png [--crop 0,0,W,H] [--scale 1.0]

`shot` grabs the whole X screen; this helper finds the game window, crops to its live
position/size (or a local crop), optionally scales, and writes into $OUT.

Env: DISPLAY, OUT.
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile

from PIL import Image
from Xlib import display

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))))
DRIVE = [sys.executable, os.path.join(ROOT, "qa", "drive.py")]
DISPLAY = os.environ.get("DISPLAY", ":79")
OUT = os.environ.get("OUT", ".")


def wm_name(d, w):
    try:
        prop = w.get_full_property(d.intern_atom("_NET_WM_NAME"), d.intern_atom("UTF8_STRING"))
        if prop and prop.value:
            v = prop.value
            return v.decode("utf-8", "replace") if isinstance(v, bytes) else str(v)
    except Exception:
        pass
    try:
        return w.get_wm_name() or ""
    except Exception:
        return ""


def find(d, root, match, depth=0):
    if depth > 6:
        return None
    if match in wm_name(d, root):
        return root
    try:
        children = root.query_tree().children
    except Exception:
        return None
    for c in children:
        w = find(d, c, match, depth + 1)
        if w is not None:
            return w
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--title", default="Marching Squares Terrain (DEBUG)")
    ap.add_argument("--out", required=True)
    ap.add_argument("--crop", default=None, help="local x,y,w,h (default: whole window)")
    ap.add_argument("--scale", type=float, default=1.0)
    a = ap.parse_args()

    d = display.Display(DISPLAY)
    root = d.screen().root
    win = find(d, root, a.title)
    if win is None:
        print(json.dumps({"ok": False, "error": f"no X window matching {a.title!r}"}))
        return 1
    g = win.get_geometry()
    t = root.translate_coords(win, 0, 0)
    full = os.path.join(tempfile.gettempdir(), "mst-shot-game.png")
    p = subprocess.run(DRIVE + ["--display", DISPLAY, "shot", full],
                       cwd=ROOT, capture_output=True, text=True)
    if p.returncode != 0:
        print(json.dumps({"ok": False, "error": p.stderr[-200:] or p.stdout[-200:]}))
        return 1
    im = Image.open(full).convert("RGB")
    if a.crop:
        l, tp, w, h = [int(v) for v in a.crop.split(",")]
    else:
        l, tp, w, h = 0, 0, g.width, g.height
    im = im.crop((t.x + l, t.y + tp, t.x + l + w, t.y + tp + h))
    if a.scale != 1.0:
        im = im.resize((int(im.width * a.scale), int(im.height * a.scale)), Image.NEAREST)
    os.makedirs(os.path.dirname(os.path.abspath(os.path.join(OUT, a.out))), exist_ok=True)
    im.save(os.path.join(OUT, a.out))
    print(json.dumps({"ok": True, "window": [t.x, t.y, g.width, g.height], "size": im.size}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
