#!/usr/bin/env python3
"""Set the editor 3D viewport's Perspective VFOV through its View menu.

Usage (inside a recapture `shell` step, or by hand with DISPLAY/PORT set):

    python3 docs/guide/recipes/09-vfov.py --fov 40

Opens the 3D view's View > Settings... dialog and types the value, then verifies
`view3d` reports it. The menu/dialog are located with the harness (popup position,
window position/size), so only the View button's screen position is fixed (the 3D
toolbar sits at y=96 with the Roblox ribbon on, 1600x900 session).
"""
import argparse
import json
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
DRIVE = [sys.executable, os.path.join(ROOT, "qa", "drive.py")]
DISPLAY = os.environ.get("DISPLAY", ":79")
PORT = os.environ.get("PORT") or os.environ.get("MST_QA_PORT") or "8879"
VIEW_BUTTON = (972, 96)


def drive(*args):
    p = subprocess.run(DRIVE + ["--display", DISPLAY, "--port", PORT] + [str(a) for a in args],
                       cwd=ROOT, capture_output=True, text=True)
    try:
        return json.loads(p.stdout)
    except Exception:
        return {}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fov", type=float, required=True)
    a = ap.parse_args()

    v = drive("view3d")
    if abs(v.get("fov", 0.0) - a.fov) < 0.2:
        print("vfov: already %.1f" % a.fov)
        return 0

    drive("click", *VIEW_BUTTON)
    time.sleep(0.8)
    target = None
    for pop in drive("popups").get("popups", []):
        if any(it.get("text") == "Settings..." for it in pop.get("items", [])):
            pos, size = pop.get("pos"), pop.get("size")
            target = (pos[0] + 100, pos[1] + size[1] - 14)
    if not target:
        print("vfov: View menu did not open")
        return 2
    drive("move", *target)
    time.sleep(0.3)
    drive("click", *target)
    time.sleep(1.0)

    dlg = None
    for _ in range(6):
        for win in drive("windows").get("windows", []):
            if win.get("visible") and win.get("title") == "Viewport Settings":
                dlg = win
                break
        if dlg:
            break
        time.sleep(0.5)
    if not dlg:
        print("vfov: Viewport Settings dialog did not open")
        return 3

    x, y = dlg["position"]
    w, h = dlg["size"]
    drive("click", int(x + 0.50 * w), int(y + 0.216 * h))
    time.sleep(0.3)
    drive("key", "a", "--ctrl")
    time.sleep(0.2)
    drive("type", "%g" % a.fov)
    time.sleep(0.3)
    drive("click", int(x + 0.728 * w), int(y + 0.902 * h))
    time.sleep(1.0)

    got = drive("view3d").get("fov")
    print("vfov: %.2f (want %.2f)" % (got or 0.0, a.fov))
    return 0 if got is not None and abs(got - a.fov) < 0.5 else 4


if __name__ == "__main__":
    sys.exit(main())
