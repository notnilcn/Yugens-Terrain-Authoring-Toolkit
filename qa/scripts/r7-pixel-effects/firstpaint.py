#!/usr/bin/env python3
"""Time from selecting a Pixel* node to the Pixelizer strip having non-zero rects.

usage: firstpaint.py <path> [--repeats N]
"""
import json
import subprocess
import sys
import time

drive = "qa/drive.py"
path = sys.argv[1] if len(sys.argv) > 1 else "PixelRenderer"


def ui():
    out = subprocess.run([sys.executable, drive, "ui", "--filter", "PixelizerToolbarRail",
                          "--root", "all"], capture_output=True, text=True)
    try:
        data = json.loads(out.stdout)
    except Exception:
        return None
    def flat(cs):
        for c in cs:
            yield c
            yield from flat(c.get("children") or [])
    for c in flat(data.get("controls") or []):
        if c.get("name") == "PixelizerToolbarRail":
            return c.get("rect")
    return None


t0 = time.time()
subprocess.run([sys.executable, drive, "select", "--path", path], capture_output=True, text=True)
t_sel = time.time() - t0
samples = []
while time.time() - t0 < 6.0:
    r = ui()
    samples.append((round(time.time() - t0, 2), r))
    if r and r[2] > 0:
        break
print(f"select returned after {t_sel:.2f}s")
for t, r in samples:
    print(f"  t={t:.2f}s rail={r}")
