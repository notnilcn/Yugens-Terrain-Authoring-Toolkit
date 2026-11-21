#!/usr/bin/env python3
"""Dump builder-relevant Control rects (name, rect, min, text) from the QA harness.

Usage: python3 rects.py [--display :54 --port 8854] [patterns...]
Default patterns: PixelBuilder, Node3DEditorViewportContainer, BuilderToolStrip, Materials,
MstBuilder, BuilderStrip.
Prints one line per match, sorted by rect y then x.
"""
import argparse
import json
import subprocess
import sys

BUILD = "/home/clintonnguyen/g/code_examples/rrr-wt/r4base"
DEFAULT_PATTERNS = [
    "pixelbuilder", "builder", "viewportcontainer", "materials", "attribute",
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", default=":54")
    ap.add_argument("--port", default="8854")
    ap.add_argument("--root", default="all")
    ap.add_argument("patterns", nargs="*", default=[])
    ap.add_argument("--save")
    args = ap.parse_args()
    pats = [p.lower() for p in (args.patterns or DEFAULT_PATTERNS)]
    tmp = "/tmp/opencode/qa/_r5_t4_ui_dump.json"
    cmd = [sys.executable, "qa/drive.py", "--display", args.display, "--port", args.port,
           "ui", "--root", args.root, "--save", tmp]
    res = subprocess.run(cmd, cwd=BUILD, capture_output=True, text=True)
    if res.returncode != 0:
        print(res.stderr, file=sys.stderr)
        sys.exit(res.returncode)
    # stdout is truncated to 200 controls; the --save file is complete.
    data = json.load(open(tmp))
    if args.save:
        import shutil
        shutil.copyfile(tmp, args.save)
    ctrls = data.get("controls", [])
    rows = []
    for c in ctrls:
        blob = ((c.get("name") or "") + " " + (c.get("path") or "") + " " +
                (c.get("text") or "") + " " + (c.get("tooltip") or "")).lower()
        if not any(p in blob for p in pats):
            continue
        r = c.get("rect") or [0, 0, 0, 0]
        rows.append((r[1], r[0], c, r))
    rows.sort(key=lambda t: (t[0], t[1]))
    for _, _, c, r in rows:
        text = (c.get("text") or "").replace("\n", " | ")
        if len(text) > 90:
            text = text[:87] + "..."
        print("%-38s %-22s rect=[%6.0f,%6.0f,%5.0f,%5.0f] min=[%4.0f,%4.0f] vis=%s %s" % (
            (c.get("name") or "")[:38], (c.get("class") or "")[:22],
            r[0], r[1], r[2], r[3],
            (c.get("min") or [0, 0])[0], (c.get("min") or [0, 0])[1],
            "Y" if c.get("visible") else "n", text))
    extra = {k: data.get(k) for k in ("window_size", "ok") if k in data}
    print("--", json.dumps(extra))


if __name__ == "__main__":
    main()
