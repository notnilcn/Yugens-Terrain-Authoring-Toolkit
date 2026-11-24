#!/usr/bin/env python3
"""Type a hex colour into the open ColorPicker popup and close it.

Godot's colour picker is an untitled embedded Window; its Hex LineEdit reports a
dialog-local rect, so this helper adds the window position before clicking.

    python3 docs/guide/recipes/_helpers/set-color-hex.py e07a2d

Env: DISPLAY, PORT.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time

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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("hex")
    ap.add_argument("--close-at", default="1700,300")
    a = ap.parse_args()
    value = a.hex.lstrip("#")

    win = None
    for w in drive("windows").get("windows", []):
        if w.get("visible") and not str(w.get("title", "")).strip():
            win = w
            break
    if win is None:
        print(json.dumps({"ok": False, "error": "no open colour-picker window"}))
        return 1

    fields = [c for c in drive("ui", "--root", "all", "--class-name", "LineEdit")
              .get("controls", [])
              if c.get("visible") and re.fullmatch(r"[0-9a-fA-F]{6}", str(c.get("text", "")))]
    if not fields:
        print(json.dumps({"ok": False, "error": "no hex field in the colour picker"}))
        return 1
    x, y, w, h = fields[0]["rect"]
    ox, oy = win["position"]
    cx, cy = int(ox + x + w / 2), int(oy + y + h / 2)
    drive("click", cx, cy)
    time.sleep(0.15)
    drive("key", "a", "--ctrl")
    drive("type", value)
    drive("key", "enter")
    time.sleep(0.6)
    drive("key", "esc")
    time.sleep(0.4)
    print(json.dumps({"ok": True, "window": [ox, oy], "field": fields[0]["rect"],
                      "clicked": [cx, cy], "hex": value}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
