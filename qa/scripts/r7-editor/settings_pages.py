#!/usr/bin/env python3
"""Settings page sweep: click each Settings page tab, dump, report strip + page size.

usage: settings_pages.py <tag>
Dumps to qa/screenshots/r7-editor/ui/<tag>-settings-<page>.json and prints a
one-line summary per page (strip height, page content, H scrollbar, overflow).
"""
import json
import os
import subprocess
import sys

BUILD = "/home/clintonnguyen/g/code_examples/rrr-wt/r4base"
OUT = "/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r7-editor/ui"
sys.path.insert(0, os.path.join(BUILD, "qa"))
from drive import Rpc  # noqa: E402

TABS = {
    "Layout": (601, 694),
    "NewChunks": (682, 694),
    "Grass": (759, 694),
    "Display": (819, 694),
    "LOD": (905, 694),
    "Storage": (992, 694),
}

tag = sys.argv[1] if len(sys.argv) > 1 else "cfg"
port = int(os.environ.get("MST_QA_PORT", "8891"))
rpc = Rpc(port)
rpc.connect()


def click(x, y):
    subprocess.run([sys.executable, os.path.join(BUILD, "qa/drive.py"), "click",
                    str(x), str(y)], check=False, stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL)


def vis(d, name):
    for c in d["controls"]:
        if c.get("name") == name and c.get("visible"):
            return c
    return None


for page, (x, y) in TABS.items():
    click(x, y)
    import time
    time.sleep(2.5)
    r = rpc.call({"cmd": "ui", "root": "all"})
    f = "%s/%s-settings-%s.json" % (OUT, tag, page)
    json.dump(r, open(f, "w"))
    d = r
    strip = vis(d, "MarchingSquaresToolAttributes")
    cp = vis(d, "SettingsPage")
    sx, sy, sw, sh = strip["rect"]
    hbar = [c for c in d["controls"] if c.get("visible") and c["class"] == "HScrollBar"
            and "MarchingSquaresToolAttributes" in c.get("path", "")]
    over = [c for c in d["controls"] if c.get("visible")
            and "MarchingSquaresToolAttributes" in c.get("path", "")
            and c["rect"][0] + c["rect"][2] > sx + sw + 0.5]
    flow = [c for c in d["controls"] if c.get("visible") and c["name"].startswith("@MstFlow")
            and "MarchingSquaresToolAttributes" in c.get("path", "")]
    print("%-10s strip=[%g,%g %gx%g] min=%s page=%s flow=%s hbar=%d over=%d" % (
        page, sx, sy, sw, sh, strip["min"],
        "[%gx%g]" % (cp["rect"][2], cp["rect"][3]) if cp else "-",
        "[%gx%g]" % (flow[0]["rect"][2], flow[0]["rect"][3]) if flow else "-",
        len(hbar), len(over)), flush=True)
