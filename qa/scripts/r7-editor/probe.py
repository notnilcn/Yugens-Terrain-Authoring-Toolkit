#!/usr/bin/env python3
"""First-paint timing probe: persistent harness socket, cheap control_at polls.

usage: probe.py <seconds> [--select PATH] [--points x,y;x,y;...]
Prints, per sample: wall-clock offset, and for each probe point the hit control's
name + global rect. Used to catch the r6 '39x0 / 0x35 / 250x0' zero-layout frame
without paying the ~2.5 s of a full `ui --root all` dump.
"""
import os
import sys
import time

BUILD = "/home/clintonnguyen/g/code_examples/rrr-wt/r4base"
sys.path.insert(0, os.path.join(BUILD, "qa"))
from drive import Rpc  # noqa: E402

port = int(os.environ.get("MST_QA_PORT", "8891"))
rpc = Rpc(port)
rpc.connect()

args = sys.argv[1:]
dur = float(args[0]) if args and not args[0].startswith("-") else 5.0
select = None
points = [(1100, 300), (300, 400), (700, 700)]
i = 0
while i < len(args):
    if args[i] == "--select":
        select = args[i + 1]
    elif args[i] == "--points":
        points = [tuple(int(v) for v in p.split(",")) for p in args[i + 1].split(";") if p]
    i += 1

t0 = time.time()
if select:
    r = rpc.call({"cmd": "select", "path": select})
    print("select %s -> ok=%s at +%.3fs" % (select, r.get("ok"), time.time() - t0), flush=True)

while True:
    el = time.time() - t0
    if el > dur:
        break
    parts = []
    for (x, y) in points:
        r = rpc.call({"cmd": "control_at", "x": x, "y": y})
        c = r.get("control")
        if not c:
            parts.append("%d,%d=none" % (x, y))
            continue
        rc = c["rect"]
        nm = c["name"]
        # compress the editor path to the last two components
        path = c.get("path", "")
        tail = "/".join([p for p in path.split("/") if p][-2:])
        parts.append("%d,%d=%s[%gx%g@%g,%g]{%s}" % (
            x, y, nm, rc[2], rc[3], rc[0], rc[1], tail))
    print("+%.2f %s" % (el, " | ".join(parts)), flush=True)
    time.sleep(0.12)
