#!/usr/bin/env python3
"""Click the terrain Storage page's Export button and grab the inline progress bar.

The bake is fast in this session; the only visible progress frame is the "Writing
file" phase, so the still is taken `--delay` seconds after the click.

    python3 docs/guide/recipes/_helpers/export-progress.py --out 36-glb-export-progress.png \
        --crop 400,690,900,90 --delay 0.3

Env: DISPLAY, PORT, OUT.
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
OUT = os.environ.get("OUT", ".")


def drive(*args):
    p = subprocess.run(DRIVE + ["--display", DISPLAY, "--port", PORT] + [str(a) for a in args],
                       cwd=ROOT, capture_output=True, text=True)
    try:
        return json.loads(p.stdout)
    except Exception:
        return {}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--crop", required=True)
    ap.add_argument("--delay", type=float, default=0.3)
    a = ap.parse_args()

    controls = drive("ui", "--root", "all", "--text", "Export").get("controls", [])
    buttons = [c for c in controls if c.get("visible") and c.get("text") == "Export"
               and c.get("rect") and c["rect"][1] > 650 and c["rect"][0] < 1400]
    if not buttons:
        print(json.dumps({"ok": False, "error": "terrain Export button not found"}))
        return 1
    x, y, w, h = buttons[0]["rect"]
    cx, cy = int(x + w / 2), int(y + h / 2)
    drive("click", cx, cy)
    time.sleep(a.delay)
    drive("capture", os.path.join(OUT, a.out), "--crop", a.crop)
    print(json.dumps({"ok": True, "button": buttons[0]["rect"], "screen": [cx, cy],
                      "delay": a.delay}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
