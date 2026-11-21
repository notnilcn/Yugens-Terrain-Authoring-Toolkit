#!/usr/bin/env python3
"""List controls whose dump path contains a path fragment, with rect/clipped."""
import json, sys

path = sys.argv[1]
fragment = sys.argv[2] if len(sys.argv) > 2 else ""
data = json.load(open(path))
controls = data.get("controls", data if isinstance(data, list) else [])
for c in controls:
    if fragment and fragment not in str(c.get("path", "")) and fragment not in str(c.get("name", "")):
        continue
    r = c["rect"]
    print(f'{c["name"]:<24} [{r[0]:.0f},{r[1]:.0f} {r[2]:.0f}x{r[3]:.0f}] clipped={c.get("clipped")} vis={c.get("visible")} text={c.get("text")!r}')
