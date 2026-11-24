#!/usr/bin/env python3
"""Click a button inside a visible editor dialog window, at run time.

Dialogs in this editor are separate (embedded) Windows: the harness reports a
button's rect relative to the dialog, so click-text lands at the wrong screen
position. This helper reads the dialog's position from `windows`, adds the
button's rect center, and clicks the real X11 coordinate.

    python3 docs/guide/recipes/_helpers/click-window-button.py "Please Confirm..." "Open Anyway"

Options: --index N (which matching button), --timeout S, --rect-only (print,
do not click). Env: DISPLAY, PORT (as recapture sets them).
"""
import argparse
import json
import os
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
    ap.add_argument("title")
    ap.add_argument("text")
    ap.add_argument("--index", type=int, default=0)
    ap.add_argument("--timeout", type=float, default=5.0)
    ap.add_argument("--rect-only", action="store_true")
    ap.add_argument("--optional", action="store_true",
                    help="exit 0 when the window or button is absent")
    a = ap.parse_args()

    win = None
    deadline = time.time() + a.timeout
    while time.time() < deadline:
        for w in drive("windows").get("windows", []):
            if w.get("visible") and a.title.lower() in str(w.get("title", "")).lower():
                win = w
                break
        if win:
            break
        time.sleep(0.2)
    if not win:
        print(json.dumps({"ok": False, "error": f"no visible window matching {a.title!r}"}))
        return 0 if a.optional else 1

    matches = [c for c in drive("ui", "--root", "all", "--text", a.text).get("controls", [])
               if c.get("visible") and str(c.get("text", "")) == a.text]
    if a.index >= len(matches):
        print(json.dumps({"ok": False, "error": f"no button {a.text!r} in {a.title!r} "
                                               f"({len(matches)} matches)"}))
        return 0 if a.optional else 1
    c = matches[a.index]
    ox, oy = win["position"]
    x, y, w, h = c["rect"]
    sx, sy = int(ox + x + w / 2), int(oy + y + h / 2)
    info = {"ok": True, "window": [ox, oy], "button": c["rect"], "screen": [sx, sy],
            "matches": len(matches)}
    if not a.rect_only:
        drive("click", sx, sy)
    print(json.dumps(info))
    return 0


if __name__ == "__main__":
    sys.exit(main())
