#!/usr/bin/env python3
"""Clamp + undo sweep over the Pixelizer strip number fields (#38-adjacent, r6 Finding 3).

usage: clamps.py
Runs in the session env (DISPLAY/MST_QA_PORT set by env.sh). Prints one row per field:
field, widget min/max, before, after(+80px drag), undo action, undo_count delta, value after Ctrl+Z.
"""
import json
import subprocess
import sys
import time

DRIVE = ["python3", "qa/drive.py"]
TMP = "/tmp/opencode/r7v3-clamps.json"


def run(*args):
    r = subprocess.run(DRIVE + list(args), capture_output=True, text=True)
    try:
        return json.loads(r.stdout)
    except Exception:
        return {"raw": r.stdout, "err": r.stderr}


def dump():
    run("ui", "--filter", "PixelizerAttributes", "--root", "all", "--save", TMP)
    d = json.load(open(TMP))

    def flat(cs):
        for c in cs:
            yield c
            yield from flat(c.get("children") or [])
    return [c for c in flat(d.get("controls") or []) if c.get("visible")]


def click(x, y):
    run("click", str(x), str(y))


def find_rail(name):
    """rail buttons are at x 289..319; find the one whose name matches."""
    fields = dump()
    for c in fields:
        if c.get("class") == "Button" and name in str(c.get("name")):
            r = c["rect"]
            return int(r[0] + r[2] / 2), int(r[1] + r[3] / 2)
    return None


def fields():
    out = []
    for c in dump():
        if c.get("class") == "EditorSpinSlider":
            out.append(c)
    return out


def field_value(name):
    for c in fields():
        if name in str(c.get("name")):
            return c
    return None


def main():
    plan = [
        ("rail Renderer", "Renderer", "Pixel Size"),
        ("tab Outlines", "Outlines", "Darken"),
        ("tab Rays", "Rays", "Ray Alpha"),
        ("rail Sky", "Sky & Clouds", "Coverage"),
        ("rail Water", "Water", "Swell"),
        ("rail Fireflies", "Fireflies", "Count"),
    ]
    print(f"{'field':<16}{'range':<14}{'before':<9}{'after':<9}{'undo action':<32}{'dcount':<7}{'after ctrl+z'}")
    for label, target, fname in plan:
        if label.startswith("tab"):
            run("click-text", target, "--exact")
        else:
            pos = find_rail(target)
            if not pos:
                print(f"{fname}: rail {target!r} not found")
                continue
            click(*pos)
        time.sleep(2)
        c = field_value(fname)
        if not c:
            print(f"{fname}: field not found on page {target!r}")
            continue
        r = c["rect"]
        before = c.get("value")
        u0 = run("undo-state").get("undo_count")
        x, y = int(r[0] + r[2] / 2), int(r[1] + r[3] / 2)
        run("drag", str(x), str(y), str(x + 80), str(y), "--duration", "1.2")
        time.sleep(1.5)
        after = field_value(fname)
        u1 = run("undo-state")
        run("key", "z", "--ctrl")
        time.sleep(1.5)
        back = field_value(fname)
        print(f"{fname:<16}{str(c.get('min_value')) + '-' + str(c.get('max_value')):<14}"
              f"{str(before):<9}{str(after.get('value') if after else None):<9}"
              f"{str(u1.get('action')):<32}{str((u1.get('undo_count') or 0) - (u0 or 0)):<7}"
              f"{back.get('value') if back else None}")


if __name__ == "__main__":
    main()
