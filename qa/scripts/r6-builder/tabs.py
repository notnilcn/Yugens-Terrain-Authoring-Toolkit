#!/usr/bin/env python3
"""Print the builder strip page tabs (name, rect, toggled/pressed, visible)."""
import json, subprocess, tempfile, os

out = tempfile.mktemp(suffix=".json")
subprocess.run(["python3", "qa/drive.py", "ui", "--root", "all", "--filter", "", "--save", out],
               cwd="/home/clintonnguyen/g/code_examples/rrr-wt/r4base", check=True,
               stdout=subprocess.DEVNULL)
d = json.load(open(out)); os.unlink(out)
names = ("Extrude", "Ops", "Move", "Paint")
for c in d["controls"]:
    if c["name"] in names:
        r = ",".join(str(int(v)) for v in c["rect"])
        print(f"{c['name']:8s} [{r}] toggled={c.get('toggled')} visible={c.get('visible')} text={c.get('text')!r}")
