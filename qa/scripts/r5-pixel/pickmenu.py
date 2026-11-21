#!/usr/bin/env python3
"""pickmenu.py INDEX [--popup N]: click item INDEX of the open popup (no rect in dump,
so item rows are computed from pos/size/count like the README formula)."""
import json
import os
import socket
import sys

PORT = int(os.environ.get("MST_QA_PORT", "8853"))


def rpc(req):
    s = socket.create_connection(("127.0.0.1", PORT), timeout=20)
    s.settimeout(60)
    s.sendall((json.dumps(req) + "\n").encode())
    buf = b""
    while b"\n" not in buf:
        c = s.recv(65536)
        if not c:
            raise SystemExit("closed")
        buf += c
    return json.loads(buf.split(b"\n", 1)[0])


def main():
    idx = int(sys.argv[1])
    which = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    d = rpc({"cmd": "popups"})
    p = d["popups"][which]
    px, py = p["pos"]
    pw, ph = p["size"]
    n = len(p["items"])
    row = (ph - 16) / n
    cy = py + 8 + (idx + 0.5) * row
    cx = px + pw / 2
    print(json.dumps(p["items"][idx].get("text")))
    print(json.dumps({"click": [int(cx), int(cy)]}))
    rpc({"cmd": "click", "x": int(cx), "y": int(cy)})


if __name__ == "__main__":
    main()
