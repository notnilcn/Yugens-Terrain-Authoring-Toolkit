#!/usr/bin/env python3
"""Grab a Roblox builder handle at its computed screen position and drag it.

The builder's Roblox handles (face balls, axis arrows, rings, corner cubes) are
drawn at positions derived from the part's cage and the camera. Centring drift of
a few pixels makes a hard-coded coordinate miss, so this helper reads the part's
AABB + position, projects the handle, and performs the drag.

Usage (DISPLAY/PORT set by the recapture runner):

    python3 docs/guide/recipes/09-grip.py --path Box --kind scale-ball --axis x --dx 80 --dy 4
    python3 docs/guide/recipes/09-grip.py --path Box --kind move-arrow --axis x --dx 80
    python3 docs/guide/recipes/09-grip.py --path Box --kind corner --corner max,max,min --dx 27 --dy 22
    python3 docs/guide/recipes/09-grip.py --path Box --kind rotate-ring --axis x --px 30

--mod ctrl is forwarded to a scale drag (symmetric). The press/move/release is
real X11 input (drive.py button/move), so the editor sees the same events as a
person. `--dry-run` only prints the handle screen position.
"""
import argparse
import json
import math
import os
import re
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
DRIVE = [sys.executable, os.path.join(ROOT, "qa", "drive.py")]
DISPLAY = os.environ.get("DISPLAY", ":79")
PORT = os.environ.get("PORT") or os.environ.get("MST_QA_PORT") or "8879"
BALL_GAP_PX = 14.0
ARROW_GAP_PX = 10.0
ARROW_LEN_PX = 46.0
RING_GAP_PX = 26.0


def drive(*args):
    p = subprocess.run(DRIVE + ["--display", DISPLAY, "--port", PORT] + [str(a) for a in args],
                       cwd=ROOT, capture_output=True, text=True)
    try:
        return json.loads(p.stdout)
    except Exception:
        return {}


_CAM = {}


def _norm(a):
    n = math.sqrt(sum(x * x for x in a)) or 1.0
    return [x / n for x in a]


def _cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def _dot(a, b):
    return sum(a[i] * b[i] for i in range(3))


def cam():
    """The editor 3D camera, read once; project() is then pure maths (a `drive
    project` call per ring sample made the handle search take ~40 s)."""
    if not _CAM:
        v = drive("view3d")
        f = _norm(v["camera_forward"])
        r = _norm(_cross(f, [0.0, 1.0, 0.0]))
        u = _cross(r, f)
        rect = v["rect"]
        tv = math.tan(math.radians(v["fov"]) / 2.0)
        _CAM.update(pos=v["camera_position"], f=f, r=r, u=u, rect=rect,
                    tv=tv, th=tv * rect[2] / rect[3])
    return _CAM


def project(world):
    c = cam()
    d = [world[i] - c["pos"][i] for i in range(3)]
    dep = _dot(d, c["f"])
    if dep <= 0.01:
        return None
    x = _dot(d, c["r"]) / (dep * c["th"])
    y = _dot(d, c["u"]) / (dep * c["tv"])
    return (c["rect"][0] + (x * 0.5 + 0.5) * c["rect"][2],
            c["rect"][1] + (0.5 - y * 0.5) * c["rect"][3])


def part_box(path):
    raw = drive("call", "get_aabb", "--path", path).get("value", "")
    m = re.search(r"P:\s*\(([^)]*)\),\s*S:\s*\(([^)]*)\)", raw)
    if not m:
        raise SystemExit("cannot parse aabb: %r" % raw)
    p = [float(v) for v in m.group(1).split(",")]
    s = [float(v) for v in m.group(2).split(",")]
    pos = drive("prop", "position", "--path", path).get("value")
    lo = [pos[i] + p[i] for i in range(3)]
    hi = [lo[i] + s[i] for i in range(3)]
    return lo, hi


def upp(centre):
    a = project(centre)
    b = project((centre[0] + 1.0, centre[1], centre[2]))
    if not a or not b:
        raise SystemExit("cannot project the part centre")
    return 1.0 / max(0.001, math.dist(a, b))


def add(a, d, k=1.0):
    return (a[0] + d[0] * k, a[1] + d[1] * k, a[2] + d[2] * k)


def drag(start, dx, dy, mod=None):
    drive("move", int(round(start[0])), int(round(start[1])))
    time.sleep(0.3)
    drive("button", "left", "press")
    time.sleep(0.3)
    steps = 4
    for i in range(1, steps + 1):
        drive("move", int(round(start[0] + dx * i / steps)), int(round(start[1] + dy * i / steps)))
        time.sleep(0.18)
    # keep the button down so the on-screen readout is visible long enough for the
    # clip's poster frame (the drive process start-up makes exact timings unreliable).
    time.sleep(1.5)
    drive("button", "left", "release")


def drag_ctrl(start, dx, dy):
    # drive.py drag supports --mod for a held modifier; use it for Ctrl+scale.
    tgt = (start[0] + dx, start[1] + dy)
    drive("drag", int(round(start[0])), int(round(start[1])),
          int(round(tgt[0])), int(round(tgt[1])), "--duration", "1.6", "--mod", "ctrl")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--path", default="Box")
    ap.add_argument("--kind", required=True,
                    choices=["scale-ball", "move-arrow", "corner", "rotate-ring"])
    ap.add_argument("--axis", default="x", choices=["x", "y", "z"])
    ap.add_argument("--sign", type=int, default=1)
    ap.add_argument("--corner", default="max,max,min")
    ap.add_argument("--dx", type=float, default=0)
    ap.add_argument("--dy", type=float, default=0)
    ap.add_argument("--px", type=float, default=30)
    ap.add_argument("--near", default="")
    ap.add_argument("--mod", default="")
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()

    lo, hi = part_box(a.path)
    centre = tuple((lo[i] + hi[i]) * 0.5 for i in range(3))
    origin = tuple(drive("prop", "position", "--path", a.path).get("value") or centre)
    u = upp(centre)
    ai = "xyz".index(a.axis)
    tangent = (1.0, 0.0)
    axis = [0.0, 0.0, 0.0]
    axis[ai] = float(a.sign)

    if a.kind == "scale-ball":
        face = list(centre)
        face[ai] = hi[ai] if a.sign > 0 else lo[ai]
        world = add(tuple(face), axis, BALL_GAP_PX * u)
        start = project(world)
    elif a.kind == "move-arrow":
        half = (hi[ai] - lo[ai]) * 0.5
        base = add(centre, axis, half + ARROW_GAP_PX * u)
        tip = add(base, axis, ARROW_LEN_PX * u)
        start = project(add(base, tuple(tip[i] - base[i] for i in range(3)), 0.7))
    elif a.kind == "corner":
        sides = [s.strip() for s in a.corner.split(",")]
        world = tuple(hi[i] if sides[i] == "max" else lo[i] for i in range(3))
        start = project(world)
    else:  # rotate-ring
        # ring_frame(axis): for X the frame is (UP, -Z)
        if abs(axis[0]) < 0.9:
            r = (1.0, 0.0, 0.0)
        else:
            r = (0.0, 1.0, 0.0)
        dot = sum(r[i] * axis[i] for i in range(3))
        t = [r[i] - axis[i] * dot for i in range(3)]
        n = math.sqrt(sum(v * v for v in t)) or 1.0
        t = [v / n for v in t]
        b = (axis[1] * t[2] - axis[2] * t[1],
             axis[2] * t[0] - axis[0] * t[2],
             axis[0] * t[1] - axis[1] * t[0])
        radius = math.dist(lo, hi) * 0.5 + RING_GAP_PX * u
        near = None
        if a.near:
            near = tuple(float(v) for v in a.near.split(","))
        samples = []
        for k in range(72):
            ang = math.tau * k / 72.0
            w = tuple(origin[i] + (t[i] * math.cos(ang) + b[i] * math.sin(ang)) * radius
                      for i in range(3))
            s = project(w)
            if s:
                samples.append((k, s))
        if not samples:
            start = None
            tangent = None
        else:
            if near:
                k, start = min(samples, key=lambda kv: math.dist(kv[1], near))
            else:
                k, start = max(samples, key=lambda kv: kv[1][1])
            prev = project(tuple(origin[i] + (t[i] * math.cos(math.tau * (k - 1) / 72.0)
                                              + b[i] * math.sin(math.tau * (k - 1) / 72.0)) * radius
                                 for i in range(3)))
            nxt = project(tuple(origin[i] + (t[i] * math.cos(math.tau * (k + 1) / 72.0)
                                             + b[i] * math.sin(math.tau * (k + 1) / 72.0)) * radius
                                for i in range(3)))
            if prev and nxt:
                dx, dy = nxt[0] - prev[0], nxt[1] - prev[1]
                n = math.hypot(dx, dy) or 1.0
                tangent = (dx / n, dy / n)
            else:
                tangent = (1.0, 0.0)

    if not start:
        print("grip: handle is off-screen")
        return 2
    print("grip: %s %s at %.1f,%.1f (u=%.4f)" % (a.kind, a.axis, start[0], start[1], u))
    if a.dry_run:
        return 0

    if a.kind == "rotate-ring":
        drag(start, tangent[0] * a.px, tangent[1] * a.px)
    elif a.mod == "ctrl":
        drag_ctrl(start, a.dx, a.dy)
    else:
        drag(start, a.dx, a.dy)
    time.sleep(0.5)
    return 0


if __name__ == "__main__":
    sys.exit(main())
