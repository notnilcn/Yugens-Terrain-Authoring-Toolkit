#!/usr/bin/env python3
"""Print selected controls (by name / class / text substring) from a drive.py ui dump.

Usage: uiq.py DUMP.json [--names A,B] [--match SUBSTR] [--limits]
Prints: name, class, rect [x,y,w,h], min, visible, plus text/tooltip/items/value.
"""
import json
import sys


def rect(c):
    r = c.get("rect") or [0, 0, 0, 0]
    return "[%g,%g %gx%g]" % (r[0], r[1], r[2], r[3])


def main():
    path = sys.argv[1]
    names = None
    match = None
    args = sys.argv[2:]
    i = 0
    while i < len(args):
        if args[i] == "--names":
            names = [n for n in args[i + 1].split(",") if n]
            i += 2
        elif args[i] == "--match":
            match = args[i + 1]
            i += 2
        else:
            i += 1
    d = json.load(open(path))
    ctrls = d["controls"] if isinstance(d, dict) else d
    vis = 0
    rows = []
    for c in ctrls:
        if c.get("visible"):
            vis += 1
        n = c.get("name", "")
        txt = c.get("text", "") or ""
        tip = c.get("tooltip", "") or ""
        if names is not None:
            if n not in names:
                continue
        elif match:
            if match not in n and match not in txt and match not in tip:
                continue
        else:
            continue
        extra = []
        if txt:
            extra.append("text=%r" % txt)
        if tip:
            extra.append("tip=%r" % tip)
        if c.get("items"):
            extra.append("items=%s" % c["items"])
        if "value" in c:
            extra.append("value=%s" % c["value"])
        if c.get("scroll_v") is not None:
            extra.append("sv=%s/sh=%s" % (c.get("scroll_v"), c.get("scroll_h")))
        if c.get("disabled"):
            extra.append("DISABLED")
        if c.get("clipped"):
            extra.append("clipped")
        if not c.get("visible"):
            extra.append("HIDDEN")
        print("%-34s %-18s %-22s min=%s %s" % (
            n, c.get("class"), rect(c), c.get("min"), " ".join(extra)))
        rows.append(c)
    print("-- %d matched, %d/%d controls visible, count=%s" % (
        len(rows), vis, len(ctrls), d.get("count") if isinstance(d, dict) else "?"))


if __name__ == "__main__":
    main()
