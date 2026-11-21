#!/usr/bin/env python3
"""Print only the *visible* editor windows/dialogs (title, size, min, resizable)."""
import json, subprocess

p = subprocess.run(["python3", "qa/drive.py", "windows"],
                   cwd="/home/clintonnguyen/g/code_examples/rrr-wt/r4base",
                   capture_output=True, text=True, check=True)
d = json.loads(p.stdout)
any_ = False
for w in d["windows"]:
    if w.get("visible"):
        any_ = True
        print(f"{w['title']!r:42s} size={w['size']} min={w['min']} resizable={w['resizable']} name={w['name']}")
if not any_:
    print("(no visible windows)")
