#!/usr/bin/env python3
"""Expand the material's Albedo section and open its Color picker.

The material sections are drawn as EditorInspectorSection Controls (no text). In the
expanded StandardMaterial3D the order is Transparency, Shading, Vertex Color, Albedo,
so this helper clicks the fourth visible section, then the EditorPropertyColor that
appears below it.

    python3 docs/guide/recipes/_helpers/expand-albedo.py

Env: DISPLAY, PORT.
"""
import json
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))))
DRIVE = [sys.executable, os.path.join(ROOT, "qa", "drive.py")]
DISPLAY = os.environ.get("DISPLAY", ":79")
PORT = os.environ.get("PORT") or os.environ.get("MST_QA_PORT") or "8879"


def drive(*args):
    p = subprocess.run(DRIVE + ["--display", DISPLAY, "--port", PORT] + [str(a) for a in args],
                       cwd=ROOT, capture_output=True, text=True)
    try:
        return json.loads(p.stdout)
    except Exception:
        return {}


def main():
    sections = [c for c in drive("ui", "--root", "all", "--class-name", "EditorInspectorSection")
                .get("controls", [])
                if c.get("visible") and c["rect"][0] > 1640 and c["rect"][1] > 500
                and c["rect"][1] < 900]
    sections.sort(key=lambda c: c["rect"][1])
    # Material sections are indented at x~1658; keep those.
    mats = [c for c in sections if c["rect"][0] > 1650]
    if len(mats) < 4:
        print(json.dumps({"ok": False, "error": f"material sections not found ({len(mats)})"}))
        return 1
    albedo = mats[3]
    x, y, w, h = albedo["rect"]
    drive("click", int(x + w / 2), int(y + h / 2))
    time.sleep(1.2)

    colors = [c for c in drive("ui", "--root", "all", "--class-name", "EditorPropertyColor")
              .get("controls", [])
              if c.get("visible") and c["rect"][0] > 1640
              and y <= c["rect"][1] <= y + 160]
    if not colors:
        print(json.dumps({"ok": False, "error": "Albedo colour property not found",
                          "albedo": albedo["rect"]}))
        return 1
    colors.sort(key=lambda c: c["rect"][1])
    col = colors[0]
    cx, cy = int(col["rect"][0] + col["rect"][2] * 0.8), int(col["rect"][1] + col["rect"][3] / 2)
    drive("click", cx, cy)
    time.sleep(1.2)
    print(json.dumps({"ok": True, "albedo": albedo["rect"], "color": col["rect"],
                      "clicked": [cx, cy]}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
