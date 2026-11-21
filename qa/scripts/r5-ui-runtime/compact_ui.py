#!/usr/bin/env python3
"""Extract an auditable, compact subset of a drive.py `ui --root base` dump.

Keeps every control that belongs to the 3D editor screen (toolbar rail, bottom strip,
side panel, viewport) plus every Mst* control. Writes {meta, controls[]} where each
control keeps rect/min/custom_min/clip_contents/size_flags/scroll/visible/disabled/text.

Usage: compact_ui.py IN.json OUT.json [--width W --height H --surface NAME]
"""
import json
import sys

KEEP_CLASSES = {
    "ScrollContainer", "HScrollBar", "VScrollBar", "Node3DEditorViewport",
    "ColorRect", "PanelContainer", "Panel", "TabBar", "Button", "OptionButton",
    "CheckBox", "CheckButton", "LineEdit", "HSplitContainer", "VSplitContainer",
    "Label", "SpinBox", "EditorSpinSlider",
}

FIELDS = ("class", "name", "path", "rect", "min", "custom_min", "size_flags_h", "size_flags_v",
          "clip_contents", "visible", "disabled", "clipped", "outside_root", "scroll_h",
          "scroll_v", "text", "focus")


def main():
    src, dst = sys.argv[1], sys.argv[2]
    meta = {}
    for i, a in enumerate(sys.argv[3:]):
        if a in ("--width", "--height", "--surface"):
            meta[a.lstrip("-")] = sys.argv[4 + i]
    d = json.load(open(src))
    out = []
    for c in d.get("controls", []):
        path = c.get("path", "")
        cls = c.get("class", "")
        in3d = "@Node3DEditor@" in path or "EditorMainScreen" in path
        mst = cls.startswith("Mst") or "MarchingSquares" in c.get("name", "") or \
            "Pixelizer" in c.get("name", "") or "PixelBuilder" in c.get("name", "")
        if not (in3d or mst):
            continue
        if not (mst or cls in KEEP_CLASSES):
            continue
        out.append({k: c[k] for k in FIELDS if k in c})
    json.dump({"meta": meta, "controls": out}, open(dst, "w"))
    print(f"{dst}: {len(out)} controls")


if __name__ == "__main__":
    main()
