#!/usr/bin/env python3
"""Print rects for named controls from a `drive.py ui --save` dump (flat format)."""
import json, sys

path = sys.argv[1]
names = sys.argv[2:]
data = json.load(open(path))
controls = data.get("controls", data if isinstance(data, list) else [])


def flatten(cs):
    for c in cs:
        yield c
        if c.get("children"):
            yield from flatten(c["children"])


flat = list(flatten(controls))
if not names:
    for c in flat:
        r = c.get("rect")
        print(f'{c.get("name")}\t[{r[0]},{r[1]} {r[2]}x{r[3]}]\tvis={c.get("visible")}\tclip={c.get("clipped")}\ttext={c.get("text")!r}')
else:
    for n in names:
        hits = [c for c in flat if c.get("name") == n]
        if not hits:
            print(json.dumps({"name": n, "missing": True}))
            continue
        for c in hits:
            r = c["rect"]
            rec = {"name": n, "rect": f'[{r[0]},{r[1]} {r[2]}x{r[3]}]'}
            for k in ("min", "custom_min", "visible", "clipped", "text", "value",
                      "min_value", "max_value", "toggled", "outside_root", "items"):
                if k in c and c[k] not in (None, "", []):
                    rec[k] = c[k]
            print(json.dumps(rec))
