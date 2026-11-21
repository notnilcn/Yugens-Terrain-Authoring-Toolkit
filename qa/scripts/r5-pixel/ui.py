#!/usr/bin/env python3
"""Compact UI dump helper for the r5-t3 session.

Usage:
  ui.py <substr> [<substr2> ...] [--root base|all] [--all] [--json out.json] [--full]
Matches by control name OR text (case-insensitive substring).
Prints: class | name | rect | text | value | toggled | disabled | clipped | visible
"""
import argparse
import json
import os
import socket
import sys

PORT = int(os.environ.get("MST_QA_PORT", "8853"))
HOST = "127.0.0.1"


def rpc(req):
    s = socket.create_connection((HOST, PORT), timeout=20)
    s.settimeout(120)
    s.sendall((json.dumps(req) + "\n").encode())
    buf = b""
    while b"\n" not in buf:
        chunk = s.recv(65536)
        if not chunk:
            raise SystemExit("closed")
        buf += chunk
    return json.loads(buf.split(b"\n", 1)[0])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("needles", nargs="*")
    ap.add_argument("--root", default="base", choices=["base", "all", "scene"])
    ap.add_argument("--all", action="store_true", help="include invisible controls")
    ap.add_argument("--json", default=None)
    ap.add_argument("--full", action="store_true")
    ap.add_argument("--y", type=int, default=None, help="only controls whose rect starts below this y")
    ap.add_argument("--x", type=int, default=None, help="only controls whose rect starts right of this x")
    a = ap.parse_args()
    res = rpc({"cmd": "ui", "root": a.root})
    controls = res.get("controls", [])
    if a.json:
        with open(a.json, "w") as f:
            json.dump(res, f, indent=1)
    needles = [n.lower() for n in a.needles]
    rows = []
    for c in controls:
        if not a.all and not c.get("visible", True):
            continue
        hay = (str(c.get("name", "")) + " " + str(c.get("text", ""))).lower()
        if needles and not any(n in hay for n in needles):
            continue
        if a.y is not None and (c.get("rect") or [0, 0, 0, 0])[1] < a.y:
            continue
        if a.x is not None and (c.get("rect") or [0, 0, 0, 0])[0] < a.x:
            continue
        rows.append(c)
    if a.full:
        print(json.dumps(rows, indent=1))
        return
    for c in rows:
        r = c.get("rect") or [0, 0, 0, 0]
        bits = [
            f"{c.get('class','?'):22s}",
            f"{str(c.get('name',''))[:34]:34s}",
            f"[{r[0]:.0f},{r[1]:.0f} {r[2]:.0f}x{r[3]:.0f}]",
        ]
        t = str(c.get("text", ""))
        if t:
            bits.append(f"text={t[:52]!r}")
        if "value" in c:
            bits.append(f"val={c['value']}")
        if c.get("toggled"):
            bits.append("TOGGLED")
        if c.get("pressed"):
            bits.append("PRESSED")
        if c.get("disabled"):
            bits.append("DISABLED")
        if c.get("clipped"):
            bits.append("clipped")
        extra = ""
        if c.get("items"):
            extra = f" items={c['items']}"
        print(" | ".join(bits) + extra)


if __name__ == "__main__":
    main()
