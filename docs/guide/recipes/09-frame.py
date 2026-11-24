#!/usr/bin/env python3
"""Put the editor's 3D camera at a known look-at point, distance, azimuth and pitch.

Usage (inside a recapture `shell` step, or by hand with DISPLAY/PORT set):

    python3 docs/guide/recipes/09-frame.py --target -14 13.5 -3 --dist 12
    python3 docs/guide/recipes/09-frame.py --target -14 13.5 -3 --dist 12 --azimuth 125

--target is the world point to look at (it is panned to the middle of the viewport).
--dist is the wanted camera distance in metres. --azimuth is the wanted compass angle of
the view direction in degrees, measured like atan2(forward.x, forward.z) (0 = looking
towards +Z, 90 = looking towards +X). --elev is the wanted downward pitch in degrees.
The script nudges the builder camera with middle-drags, shift+middle pan drags and wheel
steps until each value is within tolerance, then prints the final numbers.
"""
import argparse
import json
import math
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
DRIVE = [sys.executable, os.path.join(ROOT, "qa", "drive.py")]
DISPLAY = os.environ.get("DISPLAY", ":79")
PORT = os.environ.get("PORT") or os.environ.get("MST_QA_PORT") or "8879"
VIEW = (735, 470)
AZ_DEG_PER_PX = 0.25
ELEV_DEG_PER_PX = 0.2


def drive(*args):
    p = subprocess.run(DRIVE + ["--display", DISPLAY, "--port", PORT] + [str(a) for a in args],
                       cwd=ROOT, capture_output=True, text=True)
    try:
        return json.loads(p.stdout)
    except Exception:
        return {}


def view():
    d = drive("view3d")
    return d.get("camera_position"), d.get("camera_forward")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--target", nargs=3, type=float, required=True)
    ap.add_argument("--dist", type=float, required=True)
    ap.add_argument("--azimuth", type=float, default=None)
    ap.add_argument("--elev", type=float, default=None)
    ap.add_argument("--tolerance", type=float, default=0.05)
    a = ap.parse_args()
    tgt = a.target

    def metrics():
        pos, fwd = view()
        if not pos or not fwd:
            return None
        dx, dy, dz = pos[0] - tgt[0], pos[1] - tgt[1], pos[2] - tgt[2]
        dist = math.sqrt(dx * dx + dy * dy + dz * dz)
        az = math.degrees(math.atan2(fwd[0], fwd[2]))
        elev = math.degrees(math.asin(max(-1.0, min(1.0, -fwd[1]))))
        return dist, az, elev

    def az_error(want, got):
        d = want - got
        while d > 180:
            d -= 360
        while d < -180:
            d += 360
        return d

    m = metrics()
    if m is None:
        print("frame: no camera")
        return 2

    def set_azimuth():
        if a.azimuth is None:
            return
        for _ in range(6):
            d = az_error(a.azimuth, metrics()[1])
            if abs(d) < 1.5:
                break
            px = int(max(-380, min(380, -d / AZ_DEG_PER_PX)))
            drive("drag", 500, VIEW[1], 500 + px, VIEW[1], "--button", "middle", "--duration", "1.4")
            drive("move", VIEW[0], VIEW[1])

    def set_elevation():
        if a.elev is None:
            return
        for _ in range(6):
            d = a.elev - metrics()[2]
            if abs(d) < 1.5:
                break
            px = int(max(-200, min(200, d / ELEV_DEG_PER_PX)))
            drive("drag", VIEW[0], 420, VIEW[0], 420 + px, "--button", "middle", "--duration", "1.0")
            drive("move", VIEW[0], VIEW[1])

    def set_distance():
        drive("move", VIEW[0], VIEW[1])
        m0 = metrics()
        drive("wheel", VIEW[0], VIEW[1], "down", "--times", 2)
        m1 = metrics()
        ratio = 1.15
        if m0 and m1 and m1[0] > m0[0]:
            ratio = max(1.05, min(1.5, (m1[0] / m0[0]) ** 0.5))
        for _ in range(12):
            m = metrics()
            if abs(m[0] - a.dist) <= a.dist * a.tolerance:
                break
            n = int(round(math.log(a.dist / m[0]) / math.log(ratio)))
            if n == 0:
                n = 1 if a.dist > m[0] else -1
            n = max(-8, min(8, n))
            drive("wheel", VIEW[0], VIEW[1], "up" if n < 0 else "down", "--times", abs(n))

    def centre():
        for _ in range(4):
            p = drive("project", "--point", json.dumps(a.target))
            pts = p.get("points") or []
            if not pts:
                return
            w = pts[0]["window"]
            ex, ey = w[0] - VIEW[0], w[1] - VIEW[1]
            if abs(ex) < 14 and abs(ey) < 14:
                return
            dx = int(max(-500, min(500, -ex)))
            dy = int(max(-300, min(300, -ey)))
            if dx == 0 and dy == 0:
                return
            drive("drag", 500, 470, 500 + dx, 470 + dy, "--button", "middle",
                  "--duration", "0.8", "--mod", "shift")
            drive("move", VIEW[0], VIEW[1])

    set_azimuth()
    set_elevation()
    for _ in range(3):
        centre()
        set_distance()
    centre()

    drive("move", VIEW[0], VIEW[1])
    m = metrics()
    print("frame: dist %.2f az %.1f elev %.1f (want %.2f / %s / %s)" % (
        m[0], m[1], m[2], a.dist, a.azimuth, a.elev))
    return 0


if __name__ == "__main__":
    sys.exit(main())
