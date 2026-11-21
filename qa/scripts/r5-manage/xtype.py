#!/usr/bin/env python3
"""Type arbitrary text into the focused widget via XTEST.
Usage: xtype.py "res://qa_tmp/mstdata" [--enter]
Handles chars drive.py's `type` cannot (':' '/' '_' etc.)."""
import sys, time
from Xlib import X, XK, display
from Xlib.ext import xtest

# char -> (keysym name, needs_shift)
SPECIAL = {
    ":": ("semicolon", True), "/": ("slash", False), "_": ("minus", True),
    "-": ("minus", False), ".": ("period", False), "~": ("asciitilde", True),
}
PLAIN = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789")
SHIFT_PLAIN = set("ABCDEFGHIJKLMNOPQRSTUVWXYZ")

def code(d, name):
    sym = XK.string_to_keysym(name)
    if sym == 0:
        raise SystemExit(f"bad keysym {name!r}")
    return d.keysym_to_keycode(sym)

def tap(d, kc, shift):
    if shift:
        xtest.fake_input(d, X.KeyPress, code(d, "Shift_L")); d.sync()
    xtest.fake_input(d, X.KeyPress, kc); d.sync()
    xtest.fake_input(d, X.KeyRelease, kc); d.sync()
    if shift:
        xtest.fake_input(d, X.KeyRelease, code(d, "Shift_L")); d.sync()
    time.sleep(0.02)

def main():
    disp = display.Display(":52")
    text = sys.argv[1]
    for ch in text:
        if ch in SPECIAL:
            name, sh = SPECIAL[ch]
            tap(disp, code(disp, name), sh)
        elif ch in PLAIN:
            tap(disp, code(disp, ch.lower()), ch in SHIFT_PLAIN)
        elif ch == " ":
            tap(disp, code(disp, "space"), False)
        else:
            raise SystemExit(f"unsupported char {ch!r}")
    if "--enter" in sys.argv:
        tap(disp, code(disp, "Return"), False)
    print("typed", repr(text))

main()
