#!/usr/bin/env python3
"""Click a control found by text / class / node-path substring, at run time.

Unlike click-text this can target a class (e.g. the FileSystem path LineEdit,
which shares the text `res://` with a tree row) and can type into the control
after focusing it.

    python3 docs/guide/recipes/_helpers/click-control.py --class LineEdit \
        --path-contains FileSystem --type "res://models/" --enter

Env: DISPLAY, PORT (as recapture sets them). Prints the matched rect/center.
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
    ap.add_argument("--text", default=None)
    ap.add_argument("--class-name", default=None, dest="class_name")
    ap.add_argument("--path-contains", default=None)
    ap.add_argument("--path-suffix", default=None)
    ap.add_argument("--tooltip-contains", default=None)
    ap.add_argument("--index", type=int, default=0)
    ap.add_argument("--type", default=None, help="text to type after focusing (ctrl+a first)")
    ap.add_argument("--enter", action="store_true")
    ap.add_argument("--rect-only", action="store_true")
    a = ap.parse_args()

    args = ["ui", "--root", "all"]
    if a.path_contains:
        args += ["--filter", a.path_contains]
    elif a.text:
        args += ["--filter", a.text]
    if a.class_name:
        args += ["--class-name", a.class_name]
    controls = drive(*args).get("controls", [])
    matches = []
    for c in controls:
        if not c.get("visible"):
            continue
        if a.class_name and c.get("class") != a.class_name:
            continue
        if a.text is not None and a.text.lower() not in str(c.get("text", "")).lower():
            continue
        if a.path_contains and a.path_contains not in str(c.get("path", "")):
            continue
        if a.path_suffix and not str(c.get("path", "")).endswith(a.path_suffix):
            continue
        if a.tooltip_contains and a.tooltip_contains.lower() not in str(c.get("tooltip", "")).lower():
            continue
        matches.append(c)
    if a.index >= len(matches):
        print(json.dumps({"ok": False, "error": f"no control matched ({len(matches)} matches)"}))
        return 1
    c = matches[a.index]
    x, y, w, h = c["rect"]
    sx, sy = int(x + w / 2), int(y + h / 2)
    info = {"ok": True, "match": c, "screen": [sx, sy], "matches": len(matches)}
    if not a.rect_only:
        drive("click", sx, sy)
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
