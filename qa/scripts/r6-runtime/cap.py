#!/usr/bin/env python3
"""Screenshot the runtime game window (client area only) after an optional live resize.

  cap.py OUT.png [--size WxH] [--full] [--wait 1.5]

--size resizes the matched X11 window first (no WM on Xvfb, so ConfigureWindow is the
client size). --full also writes a sibling  .full.png  with the whole 1920x1080 screen.
Prints JSON: path, window rect, client size, full path.
"""
import json
import os
import sys
import time

from PIL import Image
from Xlib import X, display

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from xwin import find, MATCH_DEFAULT  # noqa: E402


def grab(display_name):
    d = display.Display(display_name)
    s = d.screen()
    raw = s.root.get_image(0, 0, s.width_in_pixels, s.height_in_pixels, X.ZPixmap, 0xFFFFFFFF)
    return Image.frombytes("RGB", (s.width_in_pixels, s.height_in_pixels), raw.data, "raw", "BGRX")


def main():
    out = sys.argv[1]
    size = None
    full = False
    wait = 1.6
    args = sys.argv[2:]
    i = 0
    while i < len(args):
        if args[i] == "--size":
            size = tuple(int(v) for v in args[i + 1].lower().split("x"))
            i += 2
        elif args[i] == "--full":
            full = True
            i += 1
        elif args[i] == "--wait":
            wait = float(args[i + 1])
            i += 2
        else:
            raise SystemExit(f"unknown arg {args[i]}")
    display_name = os.environ.get("DISPLAY", ":0")
    match = os.environ.get("MST_WINDOW_MATCH", MATCH_DEFAULT)

    found = find(match)
    if not found:
        raise SystemExit(f"no window matching {match!r}")
    d, w, title, g = found[0]
    if size:
        w.configure(x=0, y=0, width=size[0], height=size[1])
        d.sync()
        deadline = time.time() + 15
        while time.time() < deadline:
            gg = w.get_geometry()
            if (gg.width, gg.height) == size:
                break
            time.sleep(0.1)
    time.sleep(wait)
    img = grab(display_name)
    d2 = display.Display(display_name)
    t = d2.screen().root.translate_coords(w, 0, 0)
    g = w.get_geometry()
    rect = (t.x, t.y, g.width, g.height)
    crop = img.crop((rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]))
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    crop.save(out)
    info = {"path": os.path.abspath(out), "title": title, "rect": list(rect),
            "client": [g.width, g.height]}
    if full:
        fullpath = out.rsplit(".", 1)[0] + ".full.png"
        img.save(fullpath)
        info["full"] = os.path.abspath(fullpath)
    print(json.dumps(info))


if __name__ == "__main__":
    main()
