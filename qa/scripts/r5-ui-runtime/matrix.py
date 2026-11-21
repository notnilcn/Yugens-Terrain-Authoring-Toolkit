#!/usr/bin/env python3
"""Build the auditable measured matrix from the saved compact ui dumps.

Reads qa/screenshots/r5-ui-runtime/ui/*.json, prints a Markdown table plus a CSV.
"""
import glob
import json
import os

UI = "/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r5-ui-runtime/ui"


def rect(c):
    return c["rect"] if c else None


def pick(cs, name=None, cls=None, visible=True):
    for c in cs:
        if visible and not c.get("visible"):
            continue
        if name is not None and (c.get("name") or "") != name:
            continue
        if cls is not None and c.get("class") != cls:
            continue
        return c
    return None


def fmt(r):
    if not r:
        return "-"
    if isinstance(r, dict):
        r = r["rect"]
    if isinstance(r, (list, tuple)) and len(r) == 2:
        return f"{r[0]:.0f}x{r[1]:.0f}"
    return f"{r[2]:.0f}x{r[3]:.0f}"


def main():
    rows = []
    for path in sorted(glob.glob(os.path.join(UI, "*.json"))):
        d = json.load(open(path))
        meta = d.get("meta", {})
        cs = d["controls"]
        size = f"{meta.get('width')}x{meta.get('height')}"
        surface = meta.get("surface", os.path.basename(path))
        view = pick(cs, cls="Node3DEditorViewport")
        strip = None
        for c in cs:
            if c.get("class") == "ScrollContainer" and c.get("visible") and \
                    (c.get("name") or "").startswith(("MarchingSquaresToolAttributes",
                                                      "PixelizerAttributes", "PixelBuilderStripRoot")):
                strip = c
                break
        flow = None
        if strip:
            for c in cs:
                if c.get("class") == "MstFlow" and (c.get("path") or "").startswith(strip["path"]):
                    flow = c
                    break
        vscroll = any(c.get("class") == "VScrollBar" and c.get("visible")
                      and strip and c["rect"][0] >= strip["rect"][0]
                      and c["rect"][0] + c["rect"][2] <= strip["rect"][0] + strip["rect"][2]
                      and c["rect"][1] >= strip["rect"][1] - 1
                      and c["rect"][1] + c["rect"][3] <= strip["rect"][1] + strip["rect"][3] + 1
                      for c in cs) if strip else False
        hscroll = any(c.get("class") == "HScrollBar" and c.get("visible")
                      and strip and c["rect"][1] >= strip["rect"][1]
                      and c["rect"][1] <= strip["rect"][1] + strip["rect"][3]
                      for c in cs) if strip else False
        clipped = [c for c in cs if c.get("clipped") and c.get("visible") and strip
                   and (c.get("path") or "").startswith(strip["path"]) and c.get("class") != "MstFlow"]
        rail = pick(cs, cls="MstRail")
        brail = pick(cs, name="PixelBuilderToolbarRail")
        palette = pick(cs, name="PixelBuilderPalette")
        panel = pick(cs, cls="MstSidePanel")
        content = None
        if panel:
            for c in cs:
                if (c.get("name") or "") == "Content" and (c.get("path") or "").startswith(panel["path"]):
                    content = c
                    break
        prow = {
            "size": size, "surface": surface,
            "view": fmt(rect(view)),
            "strip": fmt(rect(strip)),
            "strip_min": fmt({"rect": strip["min"]} if strip else None),
            "flow": fmt(rect(flow)),
            "vscroll": "yes" if vscroll else "no",
            "hscroll": "yes" if hscroll else "no",
            "clipped": len(clipped),
            "rail": fmt(rect(rail)),
            "builder_rail": fmt(rect(brail)),
            "palette": fmt(rect(palette)),
            "panel": fmt(rect(panel)),
            "panel_name": (panel.get("name") if panel else ""),
            "panel_content": fmt(rect(content)),
        }
        rows.append(prow)
    order = {"1920x1080": 0, "1600x900": 1, "1366x768": 2, "1280x720": 3, "1024x600": 4}
    rows.sort(key=lambda r: (order.get(r["size"], 9), r["surface"]))
    keys = list(rows[0].keys()) if rows else []
    out = "/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r5-ui-runtime/matrix.csv"
    with open(out, "w") as f:
        f.write(",".join(keys) + "\n")
        for r in rows:
            f.write(",".join(str(r[k]) for k in keys) + "\n")
    print("| " + " | ".join(keys) + " |")
    print("|" + "---|" * len(keys))
    for r in rows:
        print("| " + " | ".join(str(r[k]) for k in keys) + " |")
    print(f"\n{len(rows)} rows -> {out}")


if __name__ == "__main__":
    main()
