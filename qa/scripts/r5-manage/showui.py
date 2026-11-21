#!/usr/bin/env python3
"""Summarize a drive.py `ui --save` dump: visible, non-empty controls only.
Usage: showui.py FILE.json [substring] [--all] [--class Cls]"""
import json, sys

f = sys.argv[1]
sub = ""
args = sys.argv[2:]
cls = None
show_all = False
i = 0
while i < len(args):
    a = args[i]
    if a == "--class":
        cls = args[i + 1]; i += 2; continue
    if a == "--all":
        show_all = True; i += 1; continue
    sub = a; i += 1

d = json.load(open(f))
for c in d.get("controls", []):
    t = c.get("text", "") or ""
    tip = c.get("tooltip", "") or ""
    nm = c.get("name", "") or ""
    if sub and sub.lower() not in (t + " " + tip + " " + nm).lower():
        continue
    if cls and c.get("class") != cls:
        continue
    r = c.get("rect") or [0, 0, 0, 0]
    if not show_all and (not c.get("visible") or r[2] < 1 or r[3] < 1):
        continue
    flags = []
    if c.get("disabled"): flags.append("DIS")
    if c.get("focus"): flags.append("FOCUS")
    if c.get("toggle_mode"): flags.append("TOG" + ("!" if c.get("button_pressed") else ""))
    extra = ""
    if "value" in c and c["value"] is not None: extra += f" val={c['value']}"
    if c.get("items"): extra += f" items={c['items'][:8]}"
    print(f"{(t.replace(chr(10),'|'))[:46]:46} {c.get('class','')[:14]:14} {[round(x,1) for x in r]} {' '.join(flags)}{extra}  tip={tip[:30]}")
print("count", d.get("count"))
