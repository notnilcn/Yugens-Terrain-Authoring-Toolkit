#!/usr/bin/env python3
"""Summarize a drive.py `ui --save` dump into the rows I care about.

Usage: summarize.py DUMP.json [--all]
Prints one line per control of interest: class | name | rect | min | visible | clipped | clip_contents
"""
import json
import sys

WANT_CLASSES = {
    "MstFlow", "MstSidePanel", "ScrollContainer", "HScrollBar", "VScrollBar",
    "Node3DEditorViewport", "PixelPreviewOverlay", "MstToolbar", "MstTabs",
}
WANT_NAMES = {
    "MarchingSquaresToolAttributes", "PixelizerAttributes", "PixelBuilderStrip",
    "PixelPreviewOverlay", "MarchingSquaresToolbar", "PixelizerToolbar",
    "PixelBuilderToolbar", "MstFlow", "PixelizerPanel", "PixelBuilderMaterials",
}
# name substrings that indicate the addon surfaces
SUBSTR = ("MarchingSquares", "Pixelizer", "PixelBuilder", "Mst", "PixelPreview")


def main():
    path = sys.argv[1]
    show_all = "--all" in sys.argv
    d = json.load(open(path))
    rows = d.get("controls", d if isinstance(d, list) else [])
    for c in rows:
        name = c.get("name", "")
        cls = c.get("class", "")
        keep = (
            show_all
            or cls in WANT_CLASSES
            or name in WANT_NAMES
            or any(s in name for s in SUBSTR)
        )
        if not keep:
            continue
        if not show_all and not c.get("visible", False):
            continue
        r = c.get("rect") or [0, 0, 0, 0]
        m = c.get("min") or [0, 0]
        cm = c.get("custom_min") or [0, 0]
        flags = []
        if c.get("clipped"):
            flags.append("CLIPPED")
        if c.get("outside_root"):
            flags.append("OUTSIDE")
        print(
            f"{cls:22s} {name:38s} rect={r[0]:6.0f},{r[1]:6.0f} {r[2]:6.0f}x{r[3]:5.0f} "
            f"min={m[0]:5.0f}x{m[1]:5.0f} cmin={cm[0]:4.0f}x{cm[1]:4.0f} "
            f"sf={c.get('size_flags_h')}/{c.get('size_flags_v')} "
            f"sh={c.get('scroll_h', '-')} sv={c.get('scroll_v', '-')} "
            f"{' '.join(flags)}"
        )


if __name__ == "__main__":
    main()
