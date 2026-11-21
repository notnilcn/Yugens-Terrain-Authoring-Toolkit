#!/usr/bin/env python3
"""List clickable controls (Buttons/OptionButtons/CheckBoxes) in the editor base tree,
grouped, to find toolbar tool names and settings page tabs."""
import json
import sys

d = json.load(open(sys.argv[1]))
rows = d.get("controls", [])


def txt(c):
    t = (c.get("text") or "").replace("\n", " ")[:38]
    return t


seen = {}
for c in rows:
    if not c.get("visible"):
        continue
    cls = c.get("class", "")
    if cls not in ("Button", "OptionButton", "CheckBox", "CheckButton", "MenuButton", "LinkButton", "MstTabs", "TabBar", "Button_Tab"):
        continue
    r = c["rect"]
    key = (cls, txt(c), int(r[0]), int(r[1]))
    if key in seen:
        continue
    seen[key] = 1
    print(f"{cls:12s} {txt(c):40s} {r[0]:6.0f},{r[1]:6.0f} {r[2]:5.0f}x{r[3]:4.0f} dis={int(c.get('disabled', False))} pressed={c.get('pressed','-')}")
