#!/usr/bin/env python3
"""UI/resizability wave driver for the r5 UI/runtime pass.

Runs against the merged build's qa/drive.py and the `r5-t5` editor session.
Never edits the addon; only writes captures/dumps under the output paths.

  wave.py terrain 1920 1080
  wave.py pixel   1920 1080
  wave.py builder 1920 1080
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time

BUILD = "/home/clintonnguyen/g/code_examples/rrr-wt/r4base"
OUT = "/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r5-ui-runtime"
SCRIPTS = "/home/clintonnguyen/g/code_examples/rrr/qa/scripts/r5-ui-runtime"
TMP = "/tmp/opencode/r5t5"
CSV = os.path.join(OUT, "measured.csv")

os.makedirs(os.path.join(OUT, "ui"), exist_ok=True)
os.makedirs(TMP, exist_ok=True)
os.environ.setdefault("DISPLAY", ":55")
os.environ.setdefault("MST_QA_PORT", "8855")

# toolbar button identity differs between the expanded (text) and compact (name) rail
ALIAS = {
    "Brush": ("Brush",), "Level": ("Level",), "Smooth": ("Smooth",), "Bridge": ("Bridge",),
    "Paint": ("Paint", "Vertex Painting", "Vertex Paint"),
    "Foliage": ("Foliage",),
    "Inspect": ("Inspect", "Debug Brush"),
    "Chunks": ("Chunks", "Chunk Manager"),
    "Settings": ("Settings", "Terrain Settings"),
}
STRIP_TITLE = {
    "Brush": "Brush", "Level": "Level", "Smooth": "Smooth", "Bridge": "Bridge",
    "Paint": "Vertex Painting", "Foliage": "Foliage", "Inspect": "Debug Brush",
    "Chunks": "Chunk Manager", "Settings": "Terrain Settings",
}


def drive(*args, quiet=True):
    p = subprocess.run([sys.executable, "qa/drive.py", *map(str, args)],
                       cwd=BUILD, capture_output=True, text=True, timeout=300)
    if p.returncode != 0:
        sys.stderr.write(f"drive {' '.join(map(str, args))} failed: {p.stderr[-300:]}\n")
    return p.stdout


def drive_json(*args):
    out = drive(*args)
    i = out.find("{")
    if i < 0:
        return None
    try:
        return json.loads(out[i:])
    except json.JSONDecodeError:
        return None


def dump_base(tag):
    path = os.path.join(TMP, f"{tag}.json")
    drive("ui", "--root", "base", "--save", path)
    with open(path) as f:
        return json.load(f)


def click(x, y, settle=1.0):
    drive("click", int(x), int(y))
    time.sleep(settle)


def capture(name):
    drive("capture", os.path.join(OUT, name))


def rect(c):
    return c["rect"] if c else [0, 0, 0, 0]


def center(c):
    r = rect(c)
    return r[0] + r[2] / 2, r[1] + r[3] / 2


def inside(outer, inner):
    ox, oy, ow, oh = outer
    ix, iy, iw, ih = inner
    return ix >= ox - 1 and iy >= oy - 1 and ix + iw <= ox + ow + 2


# ------------------------------------------------------------------ rail

def rail(controls):
    rs = [c for c in controls if c.get("class") == "MstRail" and c.get("visible")]
    return rs[0] if rs else None


def compact_toggle(controls):
    r = rail(controls)
    if not r:
        return None
    rr = rect(r)
    best = None
    for c in controls:
        if c.get("class") != "Button" or not c.get("visible"):
            continue
        cr = rect(c)
        if cr[0] >= rr[0] - 2 and cr[0] < rr[0] + rr[2] + 2 and abs(cr[1] - rr[1]) < 8:
            if best is None:
                best = c
    return best


def set_expanded(controls, want_expanded=True):
    """Click the rail's collapse toggle until the rail is wide (or narrow)."""
    for _ in range(4):
        r = rail(controls)
        if not r:
            return controls
        w = rect(r)[2]
        if want_expanded and w >= 100:
            return controls
        if not want_expanded and w < 100:
            return controls
        b = compact_toggle(controls)
        if not b:
            return controls
        click(*center(b), settle=1.8)
        controls = dump_base("_toggle")["controls"]
    return controls


def tool_button(controls, tool):
    r = rail(controls)
    if not r:
        return None
    rr = rect(r)
    names = ALIAS.get(tool, (tool,))
    for c in controls:
        if c.get("class") != "Button" or not c.get("visible"):
            continue
        cr = rect(c)
        if not (cr[0] >= rr[0] - 2 and cr[0] < rr[0] + rr[2] + 2):
            continue
        txt = c.get("text") or ""
        nm = c.get("name") or ""
        tip = (c.get("tooltip") or "").splitlines()[0] if c.get("tooltip") else ""
        for cand in names:
            if txt == cand or nm == cand or tip.startswith(cand + " ") or tip.startswith(cand + " Tool"):
                return c
    return None


# ------------------------------------------------------------ measurement

def measure(controls):
    m = {}
    views = [c for c in controls if c.get("class") == "Node3DEditorViewport" and c.get("visible")]
    view = views[0] if views else None
    m["view"] = rect(view)[2:]
    m["view_xy"] = rect(view)[:2]

    strips = [c for c in controls if c.get("class") == "ScrollContainer"
              and (c.get("name") or "").startswith(("MarchingSquaresToolAttributes",
                                                    "PixelizerAttributes", "PixelBuilderStrip"))
              and c.get("visible")]
    strip = strips[0] if strips else None
    m["strip_name"] = strip["name"] if strip else ""
    m["strip"] = rect(strip)[2:]
    m["strip_xy"] = rect(strip)[:2]
    m["strip_min"] = list(strip.get("min") or [0, 0]) if strip else [0, 0]
    if strip:
        sp = strip["path"]
        flows = [c for c in controls if c.get("class") == "MstFlow"
                 and (c.get("path") or "").startswith(sp)]
        flow = flows[0] if flows else None
        m["flow"] = rect(flow)[2:]
        m["flow_min"] = list(flow.get("min") or [0, 0]) if flow else [0, 0]
        sr = rect(strip)
        sbs = [c for c in controls if c.get("class") == "VScrollBar" and c.get("visible")
               and inside(sr, c["rect"])]
        m["strip_vscroll"] = rect(sbs[0])[2:] if sbs else None
        hs = [c for c in controls if c.get("class") == "HScrollBar" and c.get("visible")
              and inside(sr, c["rect"])]
        m["strip_hscroll"] = rect(hs[0])[2:] if hs else None
        clipped = [c for c in controls if c.get("clipped") and c.get("visible")
                   and (c.get("path") or "").startswith(sp) and c.get("class") != "MstFlow"]
        m["strip_clipped"] = len(clipped)
        m["strip_clipped_names"] = ";".join((c.get("text") or c.get("name") or "")[:18]
                                            for c in clipped[:6])
        # strip title = the first disabled Button inside the strip
        titles = [c for c in controls if c.get("class") == "Button" and c.get("visible")
                  and inside(sr, c["rect"]) and c.get("disabled")]
        m["strip_title"] = (titles[0].get("text") or "") if titles else ""
        first = sorted([c for c in controls if c.get("class") in ("Button", "OptionButton", "CheckBox")
                        and c.get("visible") and inside(sr, c["rect"])
                        and (c.get("path") or "").startswith(sp)],
                       key=lambda c: (c["rect"][1], c["rect"][0]))
        m["strip_first_y"] = first[0]["rect"][1] if first else 0
        # the page-tab row: the buttons on the strip's first row
        if first:
            row_y = first[0]["rect"][1]
            tabs = [c for c in first if abs(c["rect"][1] - row_y) < 4]
            m["strip_row1"] = len(tabs)
    else:
        m.update(flow=None, flow_min=None, strip_vscroll=None, strip_hscroll=None,
                 strip_clipped=0, strip_clipped_names="", strip_title="", strip_first_y=0,
                 strip_row1=0, strip_xy=[0, 0])

    r = rail(controls)
    m["rail"] = rect(r)[2:] if r else None
    tbs = [c for c in controls if (c.get("name") or "") == "MarchingSquaresToolbar" and c.get("visible")]
    m["toolbar"] = rect(tbs[0])[2:] if tbs else None

    panels = [c for c in controls if c.get("class") == "MstSidePanel" and c.get("visible")]
    m["panel"] = rect(panels[0])[2:] if panels else None
    m["panel_name"] = panels[0]["name"] if panels else ""
    if panels:
        p = panels[0]
        pv = rect(p)
        sbs = [c for c in controls if c.get("class") == "VScrollBar" and c.get("visible")
               and inside(pv, c["rect"])]
        m["panel_vscroll"] = rect(sbs[0])[2:] if sbs else None
        content = [c for c in controls if (c.get("name") or "") == "Content" and c.get("visible")
                   and (c.get("path") or "").startswith(p["path"])]
        m["panel_content"] = rect(content[0])[2:] if content else None
        m["panel_xy"] = pv[:2]
    else:
        m.update(panel_vscroll=None, panel_content=None, panel_xy=None)
    return m


def emit(size, surface, m):
    row = {
        "size": size, "surface": surface,
        "view": f"{m['view'][0]:.0f}x{m['view'][1]:.0f}",
        "strip": f"{m['strip'][0]:.0f}x{m['strip'][1]:.0f}",
        "strip_title": m["strip_title"],
        "flow": f"{m['flow'][0]:.0f}x{m['flow'][1]:.0f}" if m.get("flow") else "-",
        "flow_min": f"{m['flow_min'][0]:.0f}x{m['flow_min'][1]:.0f}" if m.get("flow_min") else "-",
        "strip_vscroll": "yes" if m.get("strip_vscroll") else "no",
        "strip_hscroll": "yes" if m.get("strip_hscroll") else "no",
        "strip_clipped": m["strip_clipped"],
        "clipped_names": m["strip_clipped_names"],
        "rail": f"{m['rail'][0]:.0f}x{m['rail'][1]:.0f}" if m.get("rail") else "-",
        "toolbar": f"{m['toolbar'][0]:.0f}x{m['toolbar'][1]:.0f}" if m.get("toolbar") else "-",
        "panel": f"{m['panel'][0]:.0f}x{m['panel'][1]:.0f}" if m.get("panel") else "-",
        "panel_name": m.get("panel_name", ""),
        "panel_scroll": "yes" if m.get("panel_vscroll") else "no",
        "panel_content": f"{m['panel_content'][0]:.0f}x{m['panel_content'][1]:.0f}" if m.get("panel_content") else "-",
    }
    newfile = not os.path.exists(CSV)
    with open(CSV, "a") as f:
        if newfile:
            f.write(",".join(row.keys()) + "\n")
        f.write(",".join(str(v) for v in row.values()) + "\n")
    print("  " + " | ".join(f"{k}={v}" for k, v in row.items() if k not in ("size", "surface")))
    return row


def save_dump(tag, size):
    src = os.path.join(TMP, f"{tag}.json")
    dst = os.path.join(OUT, "ui", f"{tag}.json")
    subprocess.run([sys.executable, os.path.join(SCRIPTS, "compact_ui.py"), src, dst,
                    "--width", size.split("x")[0], "--height", size.split("x")[1],
                    "--surface", tag], check=False, capture_output=True)
    return dst


def snap(size, tag, surface):
    d = dump_base(tag)
    m = measure(d["controls"])
    capture(tag + ".png")
    save_dump(tag, size)
    emit(size, surface, m)
    return d["controls"], m


def resize(w, h):
    drive("resize", w, h)
    time.sleep(3.0)
    info = drive_json("info")
    print(f"resize {w}x{h} -> {info['window_size'] if info else '?'}")


# ------------------------------------------------------------------ waves

def terrain_surfaces(size):
    cs = dump_base(f"{size}-00-toolbar")["controls"]
    cs = set_expanded(cs, True)
    b = tool_button(cs, "Brush")
    if b:
        click(*center(b), settle=2.0)
    snap(size, f"{size}-00-toolbar", "toolbar-expanded")

    steps = ["Brush", "Level", "Smooth", "Bridge", "Paint", "Foliage",
             "Inspect", "Chunks", "Settings"]
    cs = dump_base("_x")["controls"]
    for i, tool in enumerate(steps, start=1):
        b = tool_button(cs, tool)
        if not b:
            print(f"  !! no toolbar button {tool}")
            continue
        click(*center(b), settle=2.2)
        tag = f"{size}-{i:02d}-{tool.lower()}"
        cs, m = snap(size, tag, tool)
        want = STRIP_TITLE[tool]
        if m["strip_title"] != want:
            print(f"    (strip title reads {m['strip_title']!r}, expected {want!r})")
        if tool == "Settings":
            settings_pages(size, cs)
            b = tool_button(cs, "Brush")
            if b:
                click(*center(b), settle=2.0)
            cs = dump_base("_x")["controls"]

    # compact rail
    cs = dump_base(f"{size}-90-pre-collapse")["controls"]
    b = compact_toggle(cs)
    if b:
        click(*center(b), settle=2.2)
        cs, m = snap(size, f"{size}-90-compact", "toolbar-compact")
        cs = set_expanded(cs, True)


def settings_pages(size, controls):
    pages = ["Grid", "New Chunks", "Grass", "Display", "LOD & Nature", "Storage"]
    for j, page in enumerate(pages):
        cands = [c for c in controls if c.get("class") == "Button"
                 and (c.get("text") or "") == page and c.get("visible")]
        if not cands:
            print(f"    !! no settings tab {page}")
            continue
        tab = sorted(cands, key=lambda c: -c["rect"][1])[0]
        click(*center(tab), settle=1.8)
        tag = f"{size}-1{j}-settings-{page.split(' ')[0].lower()}"
        controls, _ = snap(size, tag, "Settings:" + page)
        # the tab row y after the switch (row-1 stability check)
        tabs = [c for c in controls if c.get("class") == "Button" and c.get("visible")
                and (c.get("text") or "") in pages]
        if tabs:
            y = min(c["rect"][1] for c in tabs)
            print(f"    tab-row y={y:.0f} for {page}")


def pixel_surfaces(size):
    drive("select", "--path", "PixelRenderer")
    time.sleep(3.5)
    tag = f"{size}-50-pixel-renderer"
    cs, m = snap(size, tag, "Pixelizer:Renderer")
    pages = ["Renderer", "Color", "Sky", "Outlines", "Rays", "Sun", "Clouds", "Weather"]
    for j, page in enumerate(pages):
        cands = [c for c in cs if c.get("class") == "Button"
                 and (c.get("text") or "") == page and c.get("visible")]
        if not cands:
            print(f"    !! no pixelizer tab {page}")
            continue
        tab = sorted(cands, key=lambda c: -c["rect"][1])[0]
        click(*center(tab), settle=2.0)
        tag = f"{size}-5{j+1}-pixelizer-{page.lower()}"
        cs, _ = snap(size, tag, "Pixelizer:" + page)


def pixel_world(size):
    cs = dump_base(f"{size}-55-pre-pixelworld")["controls"]
    btn = [c for c in cs if c.get("class") == "MenuButton"
           and (c.get("text") or "") == "Pixel World" and c.get("visible")]
    if not btn:
        print("  !! no Pixel World menu")
        return
    click(*center(btn[0]), settle=2.2)
    tag = f"{size}-56-pixel-world"
    cs, m = snap(size, tag, "PixelWorld")
    p = drive("popups")
    open(os.path.join(OUT, "ui", tag + "-popups.txt"), "w").write(p)
    drive("key", "esc")
    time.sleep(1.5)


def builder_nodes():
    out = drive("nodes", "PixelBuilderMesh")
    i = out.find("{")
    if i < 0:
        return []
    try:
        d = json.loads(out[i:])
    except json.JSONDecodeError:
        return []
    return [x if isinstance(x, str) else x.get("path", "") for x in d.get("nodes", [])]


def builder_surfaces(size):
    nodes = builder_nodes()
    if not nodes:
        print("  !! no PixelBuilderMesh nodes")
        return
    print(f"  builder nodes: {nodes}")
    drive("select", "--path", nodes[0])
    time.sleep(3.5)
    cs, _ = snap(size, f"{size}-60-builder", "Builder:Object")
    for j, mode in enumerate(["Face", "Edge", "Vertex"]):
        cands = [c for c in cs if c.get("class") == "Button" and c.get("visible")
                 and (c.get("name") or "") == f"Mode{mode}"]
        if not cands:
            print(f"    !! no builder mode {mode}")
            continue
        click(*center(cands[0]), settle=2.2)
        cs, _ = snap(size, f"{size}-6{j+1}-builder-{mode.lower()}", "Builder:" + mode)


def main():
    what = sys.argv[1]
    w, h = int(sys.argv[2]), int(sys.argv[3])
    size = f"{w}x{h}"
    resize(w, h)
    if what == "terrain":
        terrain_surfaces(size)
    elif what == "pixel":
        pixel_surfaces(size)
        pixel_world(size)
    elif what == "builder":
        builder_surfaces(size)
    else:
        raise SystemExit(f"unknown wave {what}")


if __name__ == "__main__":
    main()
