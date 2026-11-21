#!/usr/bin/env python3
"""Dump the current terrain tool strip: ToolTitle text + all controls in the same MstFlow strip."""
import json, subprocess, sys

H = "/home/clintonnguyen/g/code_examples/rrr/qa/scripts/r5-sculpt-paint/h.sh"
out = sys.argv[1] if len(sys.argv) > 1 else "/tmp/opencode/r5-strip.json"
subprocess.run([H, "ui", "--root", "all", "--save", out], capture_output=True, check=True)
d = json.load(open(out))

title = next((c for c in d["controls"] if c["name"] == "ToolTitle"), None)
print("TITLE:", (title or {}).get("text"), "rect", [round(v) for v in (title or {}).get("rect", [])])
if not title:
    sys.exit()
flowpath = title["path"].rsplit("/", 1)[0]
flow = next((c for c in d["controls"] if c["path"] == flowpath), None)
x, y, w, h = flow["rect"]
print("STRIP", [round(v) for v in flow["rect"]], "min", [round(v) for v in flow.get("min", [])])
SKIP = {"PanelContainer", "VBoxContainer", "ScrollContainer", "PopupMenuItems", "LineEdit",
        "TextureRect", "HScrollBar", "VScrollBar", "MarginContainer", "HBoxContainer",
        "CenterContainer", "VSeparator", "HSeparator", "MstFlow"}
rows = {}
for c in d["controls"]:
    cx, cy, cw, ch = c["rect"]
    if c["class"] in SKIP or not c["visible"] or not c["path"].startswith(flowpath + "/"):
        continue
    label = c.get("text") or c.get("tooltip") or ""
    val = "" if c.get("value") is None else " value=%s" % c["value"]
    if len(label) > 44:
        label = label[:44] + "…"
    rows[(round(cy), round(cx))] = (
        f"  y={round(cy):4d} x={round(cx):4d} w={round(cw):3d} {c['class']:17s} "
        f"{c['name'][:24]:24s} {label!r:46s}{val}"
        + (" DISABLED" if c.get("disabled") else ""))
for k in sorted(rows):
    print(rows[k])
