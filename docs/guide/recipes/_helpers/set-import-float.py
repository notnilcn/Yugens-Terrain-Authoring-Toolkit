#!/usr/bin/env python3
"""Set a numeric field in the Import dock by its position in the property list.

The Import dock draws its setting names itself (EditorProperty rows), so `click-text
"Root Scale"` cannot find them. The numeric fields are EditorPropertyFloat controls,
listed top to bottom; this helper clicks the Nth one that is visible in the dock and
types a value.

    python3 docs/guide/recipes/_helpers/set-import-float.py --index 0 --value 2.0

Use it after selecting the model file (Import dock filled). Env: DISPLAY, PORT.
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
    ap.add_argument("--index", type=int, default=0)
    ap.add_argument("--value", required=True)
    ap.add_argument("--dock", default="DockSlotLeftUR")
    a = ap.parse_args()

    fields = [c for c in drive("ui", "--root", "all", "--class-name", "EditorPropertyFloat")
              .get("controls", [])
              if c.get("visible") and a.dock in str(c.get("path", ""))
              and 82 <= c["rect"][1] <= 600]
    fields.sort(key=lambda c: c["rect"][1])
    if a.index >= len(fields):
        print(json.dumps({"ok": False, "error": f"no visible float field at index {a.index} "
                                               f"({len(fields)} found)"}))
        return 1
    x, y, w, h = fields[a.index]["rect"]
    cx, cy = int(x + w / 2), int(y + h / 2)
    drive("click", cx, cy)
    time.sleep(0.15)
    drive("key", "a", "--ctrl")
    drive("type", a.value)
    drive("key", "enter")
    time.sleep(0.5)
    print(json.dumps({"ok": True, "field": fields[a.index]["rect"], "clicked": [cx, cy],
                      "value": a.value}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
