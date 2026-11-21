#!/usr/bin/env python3
"""Resize the matched X11 window (runtime sessions have no harness).

  xresize.py 1024 600 ["Marching Squares Terrain"]
"""
import sys

from Xlib import display

W, H = int(sys.argv[1]), int(sys.argv[2])
match = sys.argv[3] if len(sys.argv) > 3 else "Marching Squares Terrain"

d = display.Display()
root = d.screen().root


def name_of(w):
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


found = []
for w in walk(root):
    n = name_of(w)
    if match.lower() in n.lower():
        g = w.get_geometry()
        found.append((w, n, g))

for w, n, g in found:
    print("found:", n, g.width, g.height, hex(w.id))
    w.configure(width=W, height=H)
d.sync()
print("requested", W, H)
