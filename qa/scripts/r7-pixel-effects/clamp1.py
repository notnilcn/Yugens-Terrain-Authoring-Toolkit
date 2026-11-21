#!/usr/bin/env python3
"""One field: clamps-one.py X Y FIELD [PAGE_TAB]
Clicks a rail section at (X,Y) (optionally a page tab by text), finds the EditorSpinSlider
whose name contains FIELD, drags it +80 px, prints before/after/undo/undo-restored.
"""
import json
import subprocess
import sys
import time

DRIVE = ["python3", "qa/drive.py"]
TMP = "/tmp/opencode/r7v3-clamp1.json"


def run(*a):
    r = subprocess.run(DRIVE + list(a), capture_output=True, text=True)
    try:
        return json.loads(r.stdout)
    except Exception:
        return {"raw": r.stdout}


def dump():
    run("ui", "--filter", "PixelizerAttributes", "--root", "all", "--save", TMP)
    d = json.load(open(TMP))

    def flat(cs):
        for c in cs:
            yield c
            yield from flat(c.get("children") or [])
    return [c for c in flat(d.get("controls") or []) if c.get("visible")]


x, y, field = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
tab = sys.argv[4] if len(sys.argv) > 4 else None
run("click", str(x), str(y))
time.sleep(2)
if tab:
    run("click-text", tab, "--exact")
    time.sleep(2)


def get():
    for c in dump():
        if c.get("class") == "EditorSpinSlider" and field in str(c.get("name")):
            return c
    return None


c = get()
if not c:
    print("field not found; visible fields:", [c2.get("name") for c2 in dump() if c2.get("class") == "EditorSpinSlider"])
    sys.exit(1)
r = c["rect"]
before = c.get("value")
u0 = run("undo-state")
cx, cy = int(r[0] + r[2] / 2), int(r[1] + r[3] / 2)
run("drag", str(cx), str(cy), str(cx + 80), str(cy), "--duration", "1.2")
time.sleep(1.5)
after = get()
u1 = run("undo-state")
run("key", "z", "--ctrl")
time.sleep(1.5)
back = get()
print(f"{field}: range {c.get('min_value')}-{c.get('max_value')} before={before} after={after.get('value') if after else None} "
      f"action={u1.get('action')!r} count {u0.get('undo_count')}->{u1.get('undo_count')} restored={back.get('value') if back else None}")
