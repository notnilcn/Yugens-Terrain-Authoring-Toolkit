#!/usr/bin/env python3
"""Live X11 window helper for runtime sessions (no harness, no WM).

  xwin.py geom [MATCH]              print every matched window: id, title, geometry, screen
  xwin.py resize W H [MATCH]        configure the matched window to W x H at 0,0, read back
  xwin.py settle W H [MATCH]        resize + wait for the client to reach W x H

MATCH defaults to $MST_WINDOW_MATCH or "Marching Squares Terrain". Runtime windows are
created without a window manager on Xvfb, so a plain ConfigureWindow is the client size.
"""
import os
import sys
import time

from Xlib import display

MATCH_DEFAULT = os.environ.get("MST_WINDOW_MATCH", "Marching Squares Terrain")


def name_of(d, w):
    try:
        p = w.get_full_property(d.intern_atom("_NET_WM_NAME"), d.intern_atom("UTF8_STRING"))
        if p and p.value:
            return p.value.decode()
        p = w.get_full_property(d.intern_atom("WM_NAME"), d.intern_atom("STRING"))
        if p and p.value:
            return p.value.decode() if isinstance(p.value, bytes) else str(p.value)
    except Exception:
        pass
    return ""


def walk(w):
    yield w
    try:
        for c in w.query_tree().children:
            yield from walk(c)
    except Exception:
        return


def find(match):
    d = display.Display()
    root = d.screen().root
    found = []
    for w in walk(root):
        n = name_of(d, w)
        if match.lower() in n.lower():
            g = w.get_geometry()
            found.append((d, w, n, g))
    return found


def geom(match):
    for d, w, n, g in find(match):
        print(f"{hex(w.id)} {g.width}x{g.height}+{g.x}+{g.y} title={n!r}")


def resize(width, height, match, settle=False):
    for d, w, n, g in find(match):
        print(f"before: {hex(w.id)} {g.width}x{g.height}+{g.x}+{g.y} title={n!r}")
        w.configure(x=0, y=0, width=width, height=height)
        d.sync()
        if settle:
            deadline = time.time() + 15
            while time.time() < deadline:
                g = w.get_geometry()
                if (g.width, g.height) == (width, height):
                    break
                time.sleep(0.1)
        else:
            time.sleep(0.3)
        g = w.get_geometry()
        print(f"after:  {hex(w.id)} {g.width}x{g.height}+{g.x}+{g.y}")
        return (g.width, g.height)
    raise SystemExit(f"no window matching {match!r}")


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "geom":
        geom(sys.argv[2] if len(sys.argv) > 2 else MATCH_DEFAULT)
    elif cmd in ("resize", "settle"):
        w, h = int(sys.argv[2]), int(sys.argv[3])
        m = sys.argv[4] if len(sys.argv) > 4 else MATCH_DEFAULT
        got = resize(w, h, m, settle=(cmd == "settle"))
        sys.exit(0 if got == (w, h) else 3)
    else:
        raise SystemExit(__doc__)
