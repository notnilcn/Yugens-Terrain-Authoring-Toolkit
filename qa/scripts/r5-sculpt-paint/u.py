#!/usr/bin/env python3
"""Compact UI dump: prints class/name/rect/text(tooltip) for matching controls.
Usage: u.py <dump.json> [substr]  (substr filters class/name/text/tooltip)
"""
import json, sys

path = sys.argv[1]
filt = sys.argv[2].lower() if len(sys.argv) > 2 else None
d = json.load(open(path))
rows = 0
for c in d.get("controls", []):
    text = (c.get("text") or "")
    tip = (c.get("tooltip") or "")
    if len(text) > 60:
        text = text[:60] + "…"
    blob = f"{c['class']} {c['name']} {text} {tip}".lower()
    if filt and filt not in blob:
        continue
    rows += 1
    rect = "[" + ",".join(str(round(v)) for v in c["rect"]) + "]"
    extra = []
    if c.get("button_pressed") is not None:
        extra.append("pressed=%s" % c["button_pressed"])
    if c.get("disabled"):
        extra.append("DISABLED")
    if c.get("visible") is False:
        extra.append("hidden")
    if c.get("clipped"):
        extra.append("CLIPPED")
    if c.get("outside_root"):
        extra.append("OUTSIDE")
    if c.get("value") is not None:
        extra.append("value=%s" % c["value"])
    if c.get("items"):
        extra.append("items=%s" % ",".join(str(i) for i in c["items"][:8]))
    print(f"{c['class'][:20]:20s} {c['name'][:33]:33s} {rect:26s} {text!r:30s} {tip[:38]!r:40s} {' '.join(extra)}")
print(f"-- {rows} of {len(d.get('controls', []))} controls")
