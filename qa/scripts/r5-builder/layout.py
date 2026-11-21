#!/usr/bin/env python3
"""Report the builder layout state: mode strip height, viewport, toolbar, palette.

Usage: python3 layout.py [--display :54 --port 8854] [--crop-hud out.png]
Prints a JSON one-liner plus reads the strip root height to infer the mode.
"""
import argparse
import json
import subprocess
import sys

BUILD = "/home/clintonnguyen/g/code_examples/rrr-wt/r4base"
TMP = "/tmp/opencode/qa/_r5_t4_ui_dump.json"
KEYS = ("PixelBuilderToolbarRail", "PixelBuilderToolbar", "PixelBuilderStripRoot",
        "PixelBuilderStrip", "PixelBuilderPalette", "@Node3DEditorViewportContainer@10229")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":54")
    ap.add_argument("--port", default="8854")
    ap.add_argument("--crop-hud")
    a = ap.parse_args()
    subprocess.run([sys.executable, "qa/drive.py", "--display", a.display, "--port",
                    a.port, "ui", "--root", "all", "--save", TMP],
                   cwd=BUILD, capture_output=True)
    d = json.load(open(TMP))
    out = {}
    for c in d["controls"]:
        if c["visible"] and c["name"] in KEYS:
            out[c["name"]] = [int(v) for v in c["rect"]]
    info = subprocess.run([sys.executable, "qa/drive.py", "--display", a.display, "--port",
                           a.port, "info"], cwd=BUILD, capture_output=True, text=True)
    try:
        win = json.loads(info.stdout).get("window_size")
    except Exception:
        win = None
    out["window"] = win
    out["selection"] = None
    if a.crop_hud:
        subprocess.run([sys.executable, "qa/drive.py", "--display", a.display, "--port",
                        a.port, "capture", a.crop_hud, "--crop", "268,120,620,120"],
                       cwd=BUILD, capture_output=True)
        out["hud_crop"] = a.crop_hud
    print(json.dumps(out, sort_keys=True))


if __name__ == "__main__":
    main()
