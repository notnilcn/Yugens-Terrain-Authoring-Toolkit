#!/usr/bin/env python3
"""Click a control inside a visible editor dialog (embedded Window).

Dialogs report their controls' rects relative to the dialog, not the editor, so this
helper adds the dialog's position before clicking.

    python3 docs/guide/recipes/_helpers/click-dialog.py "Project Settings" Application
    python3 docs/guide/recipes/_helpers/click-dialog.py "Project Settings" --class LineEdit --index 0 --type stretch

Env: DISPLAY, PORT.
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
    ap.add_argument("text", nargs="?")
    ap.add_argument("--class-name", default=None, dest="class_name")
    ap.add_argument("--index", type=int, default=0)
    ap.add_argument("--exact", action="store_true")
    ap.add_argument("--type", default=None)
    ap.add_argument("--enter", action="store_true")
    ap.add_argument("--point", default=None, help="dialog-local x,y to click instead of a control")
    a = ap.parse_args()

    win = None
    for w in drive("windows").get("windows", []):
        if w.get("visible") and a.title.lower() in str(w.get("title", "")).lower():
            win = w
            break
    if win is None:
        print(json.dumps({"ok": False, "error": f"no visible window matching {a.title!r}"}))
        return 1

    if a.point:
        x, y = [int(v) for v in a.point.split(",")]
        ox, oy = win["position"]
        drive("click", ox + x, oy + y)
        print(json.dumps({"ok": True, "window": [ox, oy], "point": [x, y]}))
        return 0

    controls = drive("ui", "--root", "all").get("controls", [])
    matches = []
    for c in controls:
        if not c.get("visible"):
            continue
        if a.class_name and c.get("class") != a.class_name:
            continue
        if a.text is not None:
            blob = (str(c.get("text", "")) + "\n" + str(c.get("tooltip", "")) + "\n"
                    + str(c.get("name", "")))
            if a.exact and str(c.get("text", "")) != a.text:
                continue
            if not a.exact and a.text.lower() not in blob.lower():
                continue
        matches.append(c)
    # Keep only controls that live under the dialog's window path.
    dlgpath = str(win.get("path", ""))
    if dlgpath:
        inside = [c for c in matches if str(c.get("path", "")).startswith(dlgpath)]
        if inside:
            matches = inside
    if a.index >= len(matches):
        print(json.dumps({"ok": False, "error": f"no control matched ({len(matches)} found)"}))
        return 1
    c = matches[a.index]
    ox, oy = win["position"]
    x, y, w, h = c["rect"]
    cx, cy = int(ox + x + w / 2), int(oy + y + h / 2)
    info = {"ok": True, "match": c.get("class"), "text": c.get("text"),
            "window": [ox, oy], "rect": c["rect"], "screen": [cx, cy],
            "matches": len(matches)}
    drive("click", cx, cy)
    if a.type is not None:
        time.sleep(0.15)
        drive("key", "a", "--ctrl")
        drive("type", a.type)
    if a.enter:
        drive("key", "enter")
    print(json.dumps(info))
    return 0


if __name__ == "__main__":
    sys.exit(main())
