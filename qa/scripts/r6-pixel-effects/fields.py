#!/usr/bin/env python3
"""Print only controls inside the pixelizer strip (y > 660) that are editable."""
import json, sys

path = sys.argv[1]
data = json.load(open(path))
for c in data.get("controls", []):
    r = c["rect"]
    cls = c["class"]
    interesting = (cls in ("SpinBox", "HSlider", "VSlider", "LineEdit", "OptionButton", "CheckBox", "Button",
                           "MstValueLabel", "MstNumberField", "MstSpinBox")
                   or c.get("value") is not None)
    if r[1] >= 660 and r[2] > 0 and interesting:
        print(f'{cls:<14} {str(c["name"]):<20} [{r[0]:.0f},{r[1]:.0f} {r[2]:.0f}x{r[3]:.0f}] '
              f'val={c.get("value")} min={c.get("min_value")} max={c.get("max_value")} text={c.get("text")!r}')
