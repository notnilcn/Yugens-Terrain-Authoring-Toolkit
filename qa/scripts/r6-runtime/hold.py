#!/usr/bin/env python3
"""Hold a key down for N seconds in the matched window (XTEST), then release.

  hold.py KEY SECONDS [--shift|--ctrl|--alt]

Used for movement keys (WASD) that need a sustained press to produce motion.
"""
import os
import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest

key = sys.argv[1]
secs = float(sys.argv[2])
mods = set(sys.argv[3:])
d = display.Display(os.environ.get("DISPLAY", ":0"))
kc = d.keysym_to_keycode(XK.string_to_keysym(key))
if not kc:
    raise SystemExit(f"no keycode for {key!r}")
MOD = {"--shift": "Shift_L", "--ctrl": "Control_L", "--alt": "Alt_L"}
mod_codes = [d.keysym_to_keycode(XK.string_to_keysym(MOD[m])) for m in mods if m in MOD]
for m in mod_codes:
    xtest.fake_input(d, X.KeyPress, m)
xtest.fake_input(d, X.KeyPress, kc)
d.sync()
time.sleep(secs)
xtest.fake_input(d, X.KeyRelease, kc)
for m in reversed(mod_codes):
    xtest.fake_input(d, X.KeyRelease, m)
d.sync()
print(f"held {key} {secs}s")
