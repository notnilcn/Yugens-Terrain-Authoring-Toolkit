#!/usr/bin/env python3
"""drive.py windows -> only visible windows, compact."""
import json, subprocess, sys
out = subprocess.run(["python3", "qa/drive.py", "windows"], capture_output=True, text=True).stdout
d = json.loads(out)
for w in d.get("windows", []):
    if not w.get("visible"):
        continue
    print(f"{w.get('title','')!r:40} {w.get('name','')[:28]:28} pos={w.get('position')} size={w.get('size')} min={w.get('min')} resizable={w.get('resizable')} exclusive={w.get('exclusive')}")
print("total", len(d.get("windows", [])), "visible", sum(1 for w in d.get("windows", []) if w.get("visible")))
