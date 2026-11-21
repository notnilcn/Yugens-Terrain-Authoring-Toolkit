#!/usr/bin/env python3
"""Early-select first-paint test.

Polls `info` every 0.3 s; the instant the edited scene appears (i.e. the user has
just watched the project open), fires `select PixelRenderer` and then samples a
cheap `control_at` probe inside the pixelizer side panel for N seconds. A hit
inside `PixelWorldPanel` means the first-paint layout exists.

usage: early_firstpaint.py [secs] [--shots]
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
secs = float(sys.argv[1]) if len(sys.argv) > 1 and not sys.argv[1].startswith("-") else 20.0
shots = "--shots" in sys.argv

rpc = Rpc(port)
rpc.connect()
t_boot = time.time()
scene = ""
for _ in range(400):
    r = rpc.call({"cmd": "info"})
    scene = r.get("scene") or ""
    if scene:
        break
    time.sleep(0.3)
print("scene appeared at +%.2fs fps=%.1f" % (time.time() - t_boot, r.get("fps", 0)), flush=True)

t0 = time.time()
r = rpc.call({"cmd": "select", "path": "PixelRenderer"})
print("select ok=%s returned +%.2fs" % (r.get("ok"), time.time() - t0), flush=True)

i = 0
while True:
    el = time.time() - t0
    if el > secs:
        break
    hit = rpc.call({"cmd": "control_at", "x": 1100, "y": 300}).get("control")
    fps = rpc.call({"cmd": "info"}).get("fps", 0)
    if hit:
        rc = hit["rect"]
        inside = "PixelWorldPanel" in hit.get("path", "")
        print("+%.2f fps=%.0f hit=%s %s[%gx%g@%g,%g] panel=%s" % (
            el, fps, hit["name"], "" if inside else "OUTSIDE", rc[2], rc[3], rc[0], rc[1],
            "YES" if inside else "NO"), flush=True)
    if shots and i in (1, 4, 8):
        subprocess.run([sys.executable, os.path.join(BUILD, "qa/drive.py"), "shot",
                        "%s/r61-early-t%d.png" % (OUT, i)],
                       check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print("  shot r61-early-t%d.png at +%.2fs" % (i, time.time() - t0), flush=True)
    i += 1
