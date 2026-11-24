#!/usr/bin/env python3
"""Capture a region of a visible editor dialog window.

    python3 docs/guide/recipes/_helpers/shot-window.py "Project Settings" \
        --out 31-main-scene-setting.png --crop 0,0,1190,460 --scale 1 \
        --redbox 210,86,1159,146

The dialog is an embedded Window; `capture` grabs the whole editor and this helper
crops at the dialog's live position plus the given local rect. --scale resamples with
NEAREST, --redbox draws a 2 px red outline in the final image.

Env: DISPLAY, PORT, OUT.
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile

from PIL import Image, ImageDraw

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))))
DRIVE = [sys.executable, os.path.join(ROOT, "qa", "drive.py")]
DISPLAY = os.environ.get("DISPLAY", ":79")
PORT = os.environ.get("PORT") or os.environ.get("MST_QA_PORT") or "8879"
OUT = os.environ.get("OUT", ".")
FFMPEG = os.environ.get("MST_FFMPEG") or "/tmp/opencode/qa/tools/ffmpeg"


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
    ap.add_argument("--out", required=True)
    ap.add_argument("--crop", required=True)
    ap.add_argument("--scale", type=float, default=1.0)
    ap.add_argument("--redbox", default=None)
    a = ap.parse_args()

    win = None
    for w in drive("windows").get("windows", []):
        if w.get("visible") and a.title.lower() in str(w.get("title", "")).lower():
            win = w
            break
    if win is None:
        print(json.dumps({"ok": False, "error": f"no visible window matching {a.title!r}"}))
        return 1
    ox, oy = win["position"]
    l, t, w, h = [int(v) for v in a.crop.split(",")]
    tmp = os.path.join(tempfile.gettempdir(), "mst-shot-window.png")
    drive("capture", tmp)
    im = Image.open(tmp).convert("RGB")
    box = (ox + l, oy + t, ox + l + w, oy + t + h)
    im = im.crop(box)
    if a.scale != 1.0:
        im = im.resize((int(im.width * a.scale), int(im.height * a.scale)), Image.NEAREST)
    if a.redbox:
        x0, y0, x1, y1 = [int(v) for v in a.redbox.split(",")]
        d = ImageDraw.Draw(im)
        d.rectangle([x0, y0, x1, y1], outline=(255, 59, 48), width=2)
    os.makedirs(os.path.dirname(os.path.abspath(os.path.join(OUT, a.out))), exist_ok=True)
    im.save(os.path.join(OUT, a.out))
    try:
        os.unlink(tmp)
    except OSError:
        pass
    print(json.dumps({"ok": True, "window": [ox, oy], "size": im.size, "out": a.out}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
