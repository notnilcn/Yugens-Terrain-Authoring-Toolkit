#!/usr/bin/env python3
"""MST QA driver: talk to qa/harness/harness.gd and inject real X11 input.

Examples:
  python3 qa/drive.py --port 8791 ping
  python3 qa/drive.py --port 8791 info
  python3 qa/drive.py --port 8791 ui --filter Brush --save /tmp/ui.json
  python3 qa/drive.py --port 8791 click-text Brush
  python3 qa/drive.py --port 8791 drag 700 400 760 430
  python3 qa/drive.py --port 8791 key z --ctrl
  python3 qa/drive.py --port 8791 shot /tmp/shot.png
"""
from __future__ import annotations

import argparse
import json
import os
import socket
import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest


# --------------------------------------------------------------------- rpc

class Rpc:
    def __init__(self, port: int, host: str = "127.0.0.1", timeout: float = 30.0):
        self.port = port
        self.host = host
        self.timeout = timeout
        self.sock: socket.socket | None = None
        self.buf = b""

    def connect(self) -> None:
        deadline = time.time() + self.timeout
        last = None
        while time.time() < deadline:
            try:
                self.sock = socket.create_connection((self.host, self.port), timeout=10)
                self.sock.settimeout(120)
                return
            except OSError as exc:  # editor still booting
                last = exc
                time.sleep(0.25)
        raise SystemExit(f"cannot connect to harness on {self.host}:{self.port}: {last}")

    def call(self, req: dict) -> dict:
        if self.sock is None:
            self.connect()
        assert self.sock is not None
        self.sock.sendall((json.dumps(req) + "\n").encode("utf-8"))
        while b"\n" not in self.buf:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise SystemExit("harness closed the connection")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        return json.loads(line.decode("utf-8"))


# --------------------------------------------------------------------- X11

BUTTONS = {"left": 1, "middle": 2, "right": 3}


class X11:
    def __init__(self, name: str, match: str = "Godot Engine"):
        self.match = match
        self.d = display.Display(name)
        self.root = self.d.screen().root
        self.window = None
        self._origin = None
        self._find_window()

    def _wm_name(self, w) -> str:
        try:
            prop = w.get_full_property(self.d.intern_atom("_NET_WM_NAME"), self.d.intern_atom("UTF8_STRING"))
            if prop and prop.value:
                v = prop.value
                return v.decode("utf-8", "replace") if isinstance(v, bytes) else str(v)
        except Exception:
            pass
        try:
            return w.get_wm_name() or ""
        except Exception:
            return ""

    def _find_window(self):
        def walk(w, depth=0):
            if depth > 6:
                return None
            name = self._wm_name(w)
            if self.match in name:
                return w
            try:
                children = w.query_tree().children
            except Exception:
                return None
            for c in children:
                found = walk(c, depth + 1)
                if found is not None:
                    return found
            return None

        self.window = walk(self.root)
        if self.window is None:
            raise SystemExit("no Godot window on display")

    def origin(self) -> tuple[int, int]:
        """Screen coordinate of the Godot window's client-area (0,0)."""
        if self._origin is None:
            t = self.root.translate_coords(self.window, 0, 0)
            self._origin = (t.x, t.y)
        return self._origin

    def size(self) -> tuple[int, int]:
        g = self.window.get_geometry()
        return (g.width, g.height)

    def focus(self) -> None:
        try:
            self.window.set_input_focus(X.RevertToParent, X.CurrentTime)
            self.d.sync()
        except Exception:
            pass

    def move(self, x: int, y: int) -> None:
        xtest.fake_input(self.d, X.MotionNotify, x=x, y=y)
        self.d.sync()

    def button(self, b: int, press: bool) -> None:
        xtest.fake_input(self.d, X.ButtonPress if press else X.ButtonRelease, b)
        self.d.sync()

    def keycode(self, name: str) -> int:
        special = {
            "ctrl": "Control_L", "control": "Control_L", "shift": "Shift_L",
            "alt": "Alt_L", "esc": "Escape", "enter": "Return", "space": "space",
            "tab": "Tab", "delete": "Delete", "backspace": "BackSpace",
            "up": "Up", "down": "Down", "left": "Left", "right": "Right",
            "minus": "minus", "equal": "equal", "bracketleft": "bracketleft",
            "bracketright": "bracketright", "period": "period", "comma": "comma",
        }
        sym_name = special.get(name.lower(), name)
        if len(sym_name) == 1:
            sym_name = sym_name
        sym = XK.string_to_keysym(sym_name)
        if sym == 0:
            raise SystemExit(f"unknown key {name!r}")
        return self.d.keysym_to_keycode(sym)

    def key(self, name: str, press: bool) -> None:
        xtest.fake_input(self.d, X.KeyPress if press else X.KeyRelease, self.keycode(name))
        self.d.sync()


def shot(display_name: str, out: str) -> tuple[int, int]:
    from PIL import Image

    d = display.Display(display_name)
    s = d.screen()
    w, h = s.width_in_pixels, s.height_in_pixels
    raw = s.root.get_image(0, 0, w, h, X.ZPixmap, 0xFFFFFFFF)
    img = Image.frombytes("RGB", (w, h), raw.data, "raw", "BGRX")
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    img.save(out)
    return img.size


# --------------------------------------------------------------------- ui helpers

def find_controls(controls: list[dict], text: str = None, class_name: str = None,
                  exact: bool = False, visible_only: bool = True, path: str = None) -> list[dict]:
    out = []
    for c in controls:
        if visible_only and not c.get("visible"):
            continue
        if class_name and c.get("class") != class_name:
            continue
        blob = (str(c.get("text", "")) + "\n" + str(c.get("tooltip", "")) + "\n" + str(c.get("name", "")))
        if text is not None:
            if exact:
                if str(c.get("text", "")) != text:
                    continue
            elif text.lower() not in blob.lower():
                continue
        if path and path not in str(c.get("path", "")):
            continue
        out.append(c)
    return out


def center(control: dict, origin: tuple[int, int]) -> tuple[int, int]:
    x, y, w, h = control["rect"]
    return (origin[0] + int(x + w / 2), origin[1] + int(y + h / 2))


# --------------------------------------------------------------------- main

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", type=int, default=int(os.environ.get("MST_QA_PORT", "8791")))
    ap.add_argument("--display", default=os.environ.get("DISPLAY", ":11"))
    ap.add_argument("--timeout", type=float, default=120.0)
    ap.add_argument("--window-match", default=os.environ.get("MST_WINDOW_MATCH", "Godot Engine"),
                    help="substring of the X11 window title to drive (game windows differ)")
    sub = ap.add_subparsers(dest="cmd", required=True)

    sub.add_parser("ping")
    sub.add_parser("info")
    sub.add_parser("undo-state")
    sub.add_parser("undo")
    sub.add_parser("redo")

    p = sub.add_parser("raw")
    p.add_argument("json")

    p = sub.add_parser("select")
    p.add_argument("--path")
    p.add_argument("--class", dest="klass")

    p = sub.add_parser("open-scene")
    p.add_argument("path")

    p = sub.add_parser("main-screen")
    p.add_argument("name", nargs="?", default="3D")

    p = sub.add_parser("nodes")
    p.add_argument("class_name")

    p = sub.add_parser("prop")
    p.add_argument("prop")
    p.add_argument("--path")
    p.add_argument("--class", dest="klass")

    p = sub.add_parser("set-prop")
    p.add_argument("prop")
    p.add_argument("value", help="JSON value")
    p.add_argument("--path")
    p.add_argument("--class", dest="klass")

    p = sub.add_parser("call")
    p.add_argument("method")
    p.add_argument("args", nargs="*", default=[])
    p.add_argument("--path")
    p.add_argument("--class", dest="klass")

    p = sub.add_parser("terrain")
    p.add_argument("--path")
    p.add_argument("--class", dest="klass")
    p.add_argument("--chunk-x", type=int, default=0)
    p.add_argument("--chunk-y", type=int, default=0)
    p.add_argument("--verbose", action="store_true")

    p = sub.add_parser("ui")
    p.add_argument("--filter", default=None)
    p.add_argument("--text", default=None)
    p.add_argument("--class-name", default=None, dest="class_name")
    p.add_argument("--exact", action="store_true")
    p.add_argument("--all", action="store_true", help="include hidden controls")
    p.add_argument("--save", default=None)
    p.add_argument("--path", default=None)
    p.add_argument("--root", default="base", choices=["base", "all", "scene"])

    sub.add_parser("windows")
    sub.add_parser("popups")

    p = sub.add_parser("ensure-visible")
    p.add_argument("text", nargs="?")
    p.add_argument("--path", default=None)

    p = sub.add_parser("click")
    p.add_argument("x", type=int)
    p.add_argument("y", type=int)
    p.add_argument("--button", default="left", choices=BUTTONS)
    p.add_argument("--mod", action="append", default=[], choices=["ctrl", "shift", "alt"])
    p.add_argument("--hold", type=float, default=0.08)

    p = sub.add_parser("click-text")
    p.add_argument("text")
    p.add_argument("--class-name", default=None, dest="class_name")
    p.add_argument("--exact", action="store_true")
    p.add_argument("--path", default=None)
    p.add_argument("--index", type=int, default=0)
    p.add_argument("--mod", action="append", default=[], choices=["ctrl", "shift", "alt"])
    p.add_argument("--hold", type=float, default=0.08)
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--no-scroll", action="store_true")

    p = sub.add_parser("move")
    p.add_argument("x", type=int)
    p.add_argument("y", type=int)

    p = sub.add_parser("button")
    p.add_argument("which", choices=["left", "middle", "right"])
    p.add_argument("action", choices=["press", "release", "click"])

    p = sub.add_parser("wheel")
    p.add_argument("x", type=int)
    p.add_argument("y", type=int)
    p.add_argument("direction", choices=["up", "down"])
    p.add_argument("--times", type=int, default=1)
    p.add_argument("--mod", action="append", default=[], choices=["ctrl", "shift", "alt"])
    p.add_argument("--delay", type=float, default=0.06)

    p = sub.add_parser("drag")
    p.add_argument("x0", type=int)
    p.add_argument("y0", type=int)
    p.add_argument("x1", type=int)
    p.add_argument("y1", type=int)
    p.add_argument("--button", default="left", choices=BUTTONS)
    p.add_argument("--mod", action="append", default=[], choices=["ctrl", "shift", "alt"])
    p.add_argument("--duration", type=float, default=0.45)
    p.add_argument("--steps", type=int, default=12)

    p = sub.add_parser("key")
    p.add_argument("key")
    p.add_argument("--ctrl", action="store_true")
    p.add_argument("--shift", action="store_true")
    p.add_argument("--alt", action="store_true")
    p.add_argument("--times", type=int, default=1)

    p = sub.add_parser("type")
    p.add_argument("text")

    p = sub.add_parser("shot")
    p.add_argument("out")
    p.add_argument("--crop", default=None, help="x,y,w,h in window content coords")

    p = sub.add_parser("resize")
    p.add_argument("width", type=int)
    p.add_argument("height", type=int)

    p = sub.add_parser("capture")
    p.add_argument("out")
    p.add_argument("--crop", default=None, help="x,y,w,h in window content coords")

    a = ap.parse_args()

    if a.cmd == "shot":
        try:
            x = X11(a.display, a.window_match)
            origin = x.origin()
        except SystemExit:
            origin = (0, 0)
        path = a.out
        size = shot(a.display, path)
        if a.crop:
            from PIL import Image
            cx, cy, cw, ch = [int(v) for v in a.crop.split(",")]
            img = Image.open(path)
            img.crop((origin[0] + cx, origin[1] + cy, origin[0] + cx + cw, origin[1] + cy + ch)).save(path)
        print(json.dumps({"ok": True, "path": os.path.abspath(path), "screen": size, "origin": origin}))
        return 0

    rpc = Rpc(a.port)
    rpc.timeout = a.timeout

    def out(obj):
        print(json.dumps(obj, indent=2))
        return 0

    if a.cmd == "ping":
        return out(rpc.call({"cmd": "ping"}))
    if a.cmd == "info":
        return out(rpc.call({"cmd": "info"}))
    if a.cmd == "raw":
        return out(rpc.call(json.loads(a.json)))
    if a.cmd == "select":
        req = {"cmd": "select"}
        if a.path:
            req["path"] = a.path
        if getattr(a, "klass", None):
            req["class"] = a.klass
        return out(rpc.call(req))
    if a.cmd == "open-scene":
        return out(rpc.call({"cmd": "open_scene", "path": a.path}))
    if a.cmd == "main-screen":
        return out(rpc.call({"cmd": "main_screen", "name": a.name}))
    if a.cmd == "nodes":
        return out(rpc.call({"cmd": "nodes", "class": a.class_name}))
    if a.cmd == "undo-state":
        return out(rpc.call({"cmd": "undo_state"}))
    if a.cmd == "undo":
        return out(rpc.call({"cmd": "undo"}))
    if a.cmd == "redo":
        return out(rpc.call({"cmd": "redo"}))
    if a.cmd == "prop":
        req = {"cmd": "prop", "prop": a.prop}
        if a.path:
            req["path"] = a.path
        if getattr(a, "klass", None):
            req["class"] = a.klass
        return out(rpc.call(req))
    if a.cmd == "set-prop":
        req = {"cmd": "set_prop", "prop": a.prop, "value": json.loads(a.value)}
        if a.path:
            req["path"] = a.path
        if getattr(a, "klass", None):
            req["class"] = a.klass
        return out(rpc.call(req))
    if a.cmd == "call":
        args = [json.loads(v) for v in a.args]
        req = {"cmd": "call", "method": a.method, "args": args}
        if a.path:
            req["path"] = a.path
        if getattr(a, "klass", None):
            req["class"] = a.klass
        return out(rpc.call(req))
    if a.cmd == "terrain":
        req = {"cmd": "terrain", "chunk_x": a.chunk_x, "chunk_y": a.chunk_y, "verbose": a.verbose}
        if a.path:
            req["path"] = a.path
        if a.klass:
            req["class"] = a.klass
        return out(rpc.call(req))
    if a.cmd == "resize":
        return out(rpc.call({"cmd": "resize", "width": a.width, "height": a.height}))
    if a.cmd == "capture":
        x = X11(a.display)
        path = os.path.abspath(a.out)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        req = {"cmd": "capture", "path": path}
        if a.crop:
            req["crop"] = [int(v) for v in a.crop.split(",")]
        return out(rpc.call(req))

    if a.cmd == "windows":
        return out(rpc.call({"cmd": "windows"}))
    if a.cmd == "popups":
        return out(rpc.call({"cmd": "popups"}))
    if a.cmd == "ensure-visible":
        req = {"cmd": "ensure_visible"}
        if a.path:
            req["path"] = a.path
        if a.text:
            req["text"] = a.text
        return out(rpc.call(req))
    if a.cmd == "ui":
        req = {"cmd": "ui", "root": a.root}
        if a.path:
            req["root"] = "path"
            req["path"] = a.path
        res = rpc.call(req)
        controls = res.get("controls", [])
        if a.filter:
            controls = [c for c in controls if a.filter.lower() in json.dumps(c).lower()]
        if a.text is not None:
            controls = find_controls(controls, text=a.text, class_name=a.class_name,
                                     exact=a.exact, visible_only=not a.all)
        elif a.class_name:
            controls = find_controls(controls, class_name=a.class_name, visible_only=not a.all)
        res["controls"] = controls
        res["count"] = len(controls)
        if a.save:
            os.makedirs(os.path.dirname(os.path.abspath(a.save)), exist_ok=True)
            with open(a.save, "w", encoding="utf-8") as fh:
                json.dump(res, fh, indent=1)
        if len(controls) > 200:
            res["controls"] = controls[:200]
            res["truncated"] = True
        return out(res)

    x = X11(a.display, a.window_match)
    origin = x.origin()
    x.focus()

    if a.cmd == "move":
        x.move(origin[0] + a.x, origin[1] + a.y)
        return out({"ok": True, "screen": [origin[0] + a.x, origin[1] + a.y]})

    if a.cmd == "click":
        for m in a.mod:
            x.key(m, True)
        x.move(origin[0] + a.x, origin[1] + a.y)
        time.sleep(0.03)
        x.button(BUTTONS[a.button], True)
        time.sleep(a.hold)
        x.button(BUTTONS[a.button], False)
        for m in reversed(a.mod):
            x.key(m, False)
        return out({"ok": True, "screen": [origin[0] + a.x, origin[1] + a.y]})

    if a.cmd == "click-text":
        res = rpc.call({"cmd": "ui"})
        matches = find_controls(res.get("controls", []), text=a.text, class_name=a.class_name,
                                exact=a.exact, path=a.path)

        def score(c):
            t = str(c.get("text", ""))
            if t == a.text:
                s = 0
            elif t.lower() == a.text.lower():
                s = 1
            elif a.text.lower() in t.lower():
                s = 2
            else:
                s = 3
            cls = c.get("class")
            pref = 0 if cls in ("OptionButton", "MenuButton") else 1 if cls in ("Button", "CheckBox") else 2
            return (s, pref)

        matches.sort(key=score)
        if not matches:
            return out({"ok": False, "error": f"no visible control matching {a.text!r}", "origin": origin})
        if a.index >= len(matches):
            return out({"ok": False, "error": f"index {a.index} out of range ({len(matches)} matches)"})
        target = matches[a.index]
        if not a.no_scroll:
            ev = rpc.call({"cmd": "ensure_visible", "path": target["path"]})
            if ev.get("ok") and ev.get("rect"):
                x_, y_, w_, h_ = ev["rect"]
                target["rect"] = [x_, y_, w_, h_]
        sx, sy = center(target, origin)
        info = {"ok": True, "match": target, "screen": [sx, sy], "matches": len(matches)}
        if a.dry_run:
            return out(info)
        for m in a.mod:
            x.key(m, True)
        x.move(sx, sy)
        time.sleep(0.03)
        x.button(BUTTONS["left"], True)
        time.sleep(a.hold)
        x.button(BUTTONS["left"], False)
        for m in reversed(a.mod):
            x.key(m, False)
        return out(info)

    if a.cmd == "button":
        btn = BUTTONS[a.which]
        if a.action in ("press", "click"):
            x.button(btn, True)
        if a.action in ("release", "click"):
            x.button(btn, False)
        return out({"ok": True})

    if a.cmd == "wheel":
        for m in a.mod:
            x.key(m, True)
        x.move(origin[0] + a.x, origin[1] + a.y)
        time.sleep(0.03)
        btn = 4 if a.direction == "up" else 5
        for _ in range(a.times):
            x.button(btn, True)
            x.button(btn, False)
            time.sleep(a.delay)
        for m in reversed(a.mod):
            x.key(m, False)
        return out({"ok": True, "times": a.times})

    if a.cmd == "drag":
        for m in a.mod:
            x.key(m, True)
        x.move(origin[0] + a.x0, origin[1] + a.y0)
        time.sleep(0.03)
        x.button(BUTTONS[a.button], True)
        steps = max(a.steps, 1)
        for i in range(1, steps + 1):
            t = i / steps
            px = int(a.x0 + (a.x1 - a.x0) * t)
            py = int(a.y0 + (a.y1 - a.y0) * t)
            time.sleep(a.duration / steps)
            x.move(origin[0] + px, origin[1] + py)
        time.sleep(0.05)
        x.button(BUTTONS[a.button], False)
        for m in reversed(a.mod):
            x.key(m, False)
        return out({"ok": True})

    if a.cmd == "key":
        mods = [m for m, on in (("ctrl", a.ctrl), ("shift", a.shift), ("alt", a.alt)) if on]
        for i in range(a.times):
            for m in mods:
                x.key(m, True)
            x.key(a.key, True)
            time.sleep(0.05)
            x.key(a.key, False)
            for m in reversed(mods):
                x.key(m, False)
            time.sleep(0.08)
        return out({"ok": True, "key": a.key, "mods": mods, "times": a.times})

    if a.cmd == "type":
        for ch in a.text:
            keyname = ch if ch != " " else "space"
            x.key(keyname, True)
            x.key(keyname, False)
            time.sleep(0.02)
        return out({"ok": True})

    raise SystemExit(f"unhandled command {a.cmd}")


if __name__ == "__main__":
    sys.exit(main())
