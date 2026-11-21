#!/usr/bin/env python3
"""Print rects of visible controls whose class matches (substring), for quick layout math."""
import json
import os
import socket
import sys

PORT = int(os.environ.get("MST_QA_PORT", "8853"))


def rpc(req):
    s = socket.create_connection(("127.0.0.1", PORT), timeout=20)
    s.settimeout(120)
    s.sendall((json.dumps(req) + "\n").encode())
    buf = b""
    while b"\n" not in buf:
        c = s.recv(65536)
        if not c:
            raise SystemExit("closed")
        buf += c
    return json.loads(buf.split(b"\n", 1)[0])


def main():
    root = os.environ.get("UI_ROOT", "base")
    pattern = sys.argv[1].lower() if len(sys.argv) > 1 else "node3deditorviewport"
    res = rpc({"cmd": "ui", "root": root})
    for c in res.get("controls", []):
        if pattern in str(c.get("class", "")).lower() and c.get("visible"):
            r = c.get("rect") or [0, 0, 0, 0]
            print(f"{c['class']:32s} {c.get('name',''):36s} x={r[0]:.0f} y={r[1]:.0f} w={r[2]:.0f} h={r[3]:.0f} min={c.get('min')}")


if __name__ == "__main__":
    main()
