#!/usr/bin/env python3
"""First-paint screenshot timeline.

Waits for the scene to load, fires `select PixelRenderer`, then takes full-screen
`shot` captures as fast as possible for N seconds, printing the true elapsed time
of each capture. `shot` is X11-side (no editor walk), so it does not stall the
build like a `ui` dump does.
"""
import os
import subprocess
import sys
import time

BUILD = "/home/clintonnguyen/g/code_examples/rrr-wt/r4base"
OUT = "/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r7-editor"
sys.path.insert(0, os.path.join(BUILD, "qa"))
from drive import Rpc  # noqa: E402

port = int(os.environ.get("MST_QA_PORT", "8891"))
disp = os.environ.get("MST_QA_DISPLAY", ":91")
rpc = Rpc(port)
rpc.connect()

# wait for the edited scene
for _ in range(120):
    r = rpc.call({"cmd": "info"})
    if r.get("scene"):
        break
    time.sleep(0.5)
print("scene=%r" % r.get("scene"), flush=True)

tag = sys.argv[1] if len(sys.argv) > 1 else "r61"
n = int(sys.argv[2]) if len(sys.argv) > 2 else 7
gap = float(sys.argv[3]) if len(sys.argv) > 3 else 0.0

t0 = time.time()
r = rpc.call({"cmd": "select", "path": "PixelRenderer"})
print("select ok=%s after %.3fs" % (r.get("ok"), time.time() - t0), flush=True)

for i in range(n):
    el = time.time() - t0
    path = "%s/%s-t%02d.png" % (OUT, tag, i)
    subprocess.run([sys.executable, os.path.join(BUILD, "qa/drive.py"), "shot", path],
                   check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print("shot %s at +%.2fs" % (os.path.basename(path), el), flush=True)
    if gap:
        time.sleep(gap)
