#!/usr/bin/env python3
"""Create and expand a StandardMaterial3D in the selected mesh's Surface Material Override.

The resource picker's menu is a separate popup Window, and its item rows have no rects,
so keyboard input does not reach it reliably. This helper finds the Surface Material
Override EditorPropertyResource, opens its picker dropdown by the live rect, clicks the
"StandardMaterial3D" row computed from the popup's position and item list, then clicks
the picker's pencil button to expand the material inline.

    python3 docs/guide/recipes/_helpers/make-material.py

Env: DISPLAY, PORT.
"""
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
    props = [c for c in drive("ui", "--root", "all", "--class-name", "EditorPropertyResource")
             .get("controls", [])
             if c.get("visible") and c["rect"][0] > 1600 and 400 <= c["rect"][1] <= 560]
    if not props:
        # The override section starts collapsed; expand the second top-level section.
        secs = [c for c in drive("ui", "--root", "all", "--class-name", "EditorInspectorSection")
                .get("controls", [])
                if c.get("visible") and c["rect"][0] > 1600 and c["rect"][1] < 600]
        secs.sort(key=lambda c: c["rect"][1])
        if len(secs) >= 2:
            x, y, w, h = secs[1]["rect"]
            drive("click", int(x + w / 2), int(y + h / 2))
            time.sleep(1.2)
        props = [c for c in drive("ui", "--root", "all", "--class-name", "EditorPropertyResource")
                 .get("controls", [])
                 if c.get("visible") and c["rect"][0] > 1600 and 400 <= c["rect"][1] <= 560]
    if not props:
        print(json.dumps({"ok": False, "error": "Surface Material Override property not found"}))
        return 1
    prop = props[0]
    path = prop["path"]

    buttons = [c for c in drive("ui", "--root", "all", "--class-name", "Button",
                                "--filter", "EditorResourcePicker").get("controls", [])
               if c.get("visible") and str(c.get("path", "")).startswith(path)]
    if not buttons:
        print(json.dumps({"ok": False, "error": "resource picker buttons not found"}))
        return 1
    dropdown = max(buttons, key=lambda c: c["rect"][0])
    x, y, w, h = dropdown["rect"]
    drive("click", int(x + w / 2), int(y + h / 2))
    time.sleep(1.2)

    drive("key", "down")
    drive("key", "down")
    drive("key", "enter")
    time.sleep(1.5)

    # Expand the new sub-resource inline with the picker's pencil (leftmost button).
    props = [c for c in drive("ui", "--root", "all", "--class-name", "EditorPropertyResource")
             .get("controls", [])
             if c.get("visible") and c["rect"][0] > 1600 and 400 <= c["rect"][1] <= 560]
    path = props[0]["path"] if props else path
    buttons = [c for c in drive("ui", "--root", "all", "--class-name", "Button",
                                "--filter", "EditorResourcePicker").get("controls", [])
               if c.get("visible") and str(c.get("path", "")).startswith(path)]
    pencil = min(buttons, key=lambda c: c["rect"][0])
    x, y, w, h = pencil["rect"]
    drive("click", int(x + w / 2), int(y + h / 2))
    time.sleep(1.5)
    print(json.dumps({"ok": True, "property": prop["rect"], "dropdown": dropdown["rect"],
                      "pencil": pencil["rect"]}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
