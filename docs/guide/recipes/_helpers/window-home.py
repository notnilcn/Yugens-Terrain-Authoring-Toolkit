#!/usr/bin/env python3
"""Move the editor window back to (0,0) at the full screen size.

`Project > Reload Current Project` re-creates the editor window, which Godot then
centres at its default 1152x648 size; on an Xvfb screen with no window manager the
bottom docks can end up off-screen. This helper finds the Godot window and moves and
resizes it to (0,0,1920,1080).

    python3 docs/guide/recipes/_helpers/window-home.py [--width 1920 --height 1080]
"""
import argparse
import os
import sys

from Xlib import display


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
    ap.add_argument("--width", type=int, default=1920)
    ap.add_argument("--height", type=int, default=1080)
    a = ap.parse_args()
    d = display.Display(os.environ.get("DISPLAY", ":79"))
    root = d.screen().root
    win = find(d, root, "Godot Engine")
    if win is None:
        print("no Godot window found")
        return 1
    win.configure(x=0, y=0, width=a.width, height=a.height)
    d.sync()
    g = win.get_geometry()
    t = root.translate_coords(win, 0, 0)
    print(f"window at {t.x},{t.y} size {g.width}x{g.height}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
