#!/usr/bin/env python3
"""Drag a file from the FileSystem dock's split view into the 3D view.

The tree rows are not harness Controls, but the split view's `FileSystemList` is, and
its rows have fixed metrics (24.5 px apart, first row at y+11). This helper switches
the dock to split mode, points it at res://models/, computes the row from the live
FileSystemList rect and drags it to the given window point.

    python3 docs/guide/recipes/_helpers/drag-model.py --index 2 --to 700,380

--index counts rows in the list: 0 is `..`, 1 the first file, 2 the second. With the
page-11 fixture the order is `..`, exported_tree.glb, roblox_house.obj.

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
    ap.add_argument("--index", type=int, required=True)
    ap.add_argument("--to", default=None)
    ap.add_argument("--folder", default="res://models/")
    ap.add_argument("--click", action="store_true", help="click the row instead of dragging it")
    a = ap.parse_args()

    drive("click-text", "Change Split Mode")
    time.sleep(1.2)

    fields = [c for c in drive("ui", "--root", "all", "--class-name", "LineEdit",
                               "--filter", "DockSlotLeftBR/FileSystem").get("controls", [])
              if c.get("visible")]
    fields.sort(key=lambda c: c["rect"][1])
    if not fields:
        print(json.dumps({"ok": False, "error": "FileSystem path field not found"}))
        return 1
    x, y, w, h = fields[0]["rect"]
    drive("click", int(x + w / 2), int(y + h / 2))
    time.sleep(0.15)
    drive("key", "a", "--ctrl")
    drive("type", a.folder)
    drive("key", "enter")
    time.sleep(2)

    lists = [c for c in drive("ui", "--root", "all", "--class-name", "FileSystemList").get("controls", [])
             if c.get("visible")]
    if not lists:
        print(json.dumps({"ok": False, "error": "split-view FileSystemList not found"}))
        return 1
    lx, ly, lw, lh = lists[0]["rect"]
    sx, sy = int(lx + 50), int(round(ly + 11 + a.index * 24.5))
    if a.click:
        drive("click", sx, sy)
        time.sleep(1.2)
        print(json.dumps({"ok": True, "list": lists[0]["rect"], "clicked": [sx, sy],
                          "index": a.index}))
        return 0
    tx, ty = [int(v) for v in a.to.split(",")]
    drive("drag", sx, sy, tx, ty, "--duration", "2")
    time.sleep(0.8)
    print(json.dumps({"ok": True, "list": lists[0]["rect"], "from": [sx, sy], "to": [tx, ty],
                      "index": a.index}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
