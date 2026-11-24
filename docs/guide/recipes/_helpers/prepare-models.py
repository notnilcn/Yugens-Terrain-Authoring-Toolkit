#!/usr/bin/env python3
"""Write the page-11 res://models fixture, rescan the project and normalise the window.

Copying files into the project of a running editor does not make Godot's FileSystem
dock see them in this headless session (no focus-driven rescan). `Project > Reload
Current Project` does a full scan, but it re-creates the editor window centred at its
default size and re-opens the restored tabs (a read-only glb tab raises a confirm
dialog). This helper does all of that and leaves the editor usable at 1920x1080.

    python3 docs/guide/recipes/_helpers/prepare-models.py --session recap-11-importing-models \
        [--open imported_models]

Run it in a `shell` step after the recipe's session has started. Env: DISPLAY, PORT.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))))
HERE = os.path.dirname(os.path.abspath(__file__))
DRIVE = [sys.executable, os.path.join(ROOT, "qa", "drive.py")]
DISPLAY = os.environ.get("DISPLAY", ":79")
PORT = os.environ.get("PORT") or os.environ.get("MST_QA_PORT") or "8879"

OBJ = """mtllib roblox_house.mtl
o RobloxHouse
v -1.5 0 -1.5
v 1.5 0 -1.5
v 1.5 2 -1.5
v -1.5 2 -1.5
v -1.5 0 1.5
v 1.5 0 1.5
v 1.5 2 1.5
v -1.5 2 1.5
usemtl Walls
f 1 2 3 4
f 5 8 7 6
f 1 4 8 5
f 2 6 7 3
"""
MTL = "newmtl Walls\nKd 0.42 0.30 0.20\n"


def drive(*args, timeout=None):
    cmd = DRIVE + ["--display", DISPLAY, "--port", PORT]
    if timeout:
        cmd += ["--timeout", str(timeout)]
    cmd += [str(a) for a in args]
    p = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    try:
        return json.loads(p.stdout), p.returncode
    except Exception:
        return {}, p.returncode


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--session", required=True)
    ap.add_argument("--open", default=None)
    a = ap.parse_args()

    state = os.environ.get("MST_QA_STATE", "/tmp/opencode/qa")
    proj = os.path.join(state, "sessions", a.session, "project")
    models = os.path.join(proj, "models")
    os.makedirs(models, exist_ok=True)
    shutil.copyfile(os.path.join(proj, "addons", "MarchingSquaresTerrain", "assets",
                                 "courtyard", "round_tree.glb"),
                    os.path.join(models, "exported_tree.glb"))
    with open(os.path.join(models, "roblox_house.obj"), "w", encoding="utf-8") as fh:
        fh.write(OBJ)
    with open(os.path.join(models, "roblox_house.mtl"), "w", encoding="utf-8") as fh:
        fh.write(MTL)

    # Reload re-creates the window and rescans res://.
    drive("click", 86, 11)
    time.sleep(0.5)
    drive("key", "r")
    time.sleep(0.3)
    drive("key", "enter")

    # Wait for the new harness to answer.
    deadline = time.time() + 90
    while time.time() < deadline:
        _, rc = drive("ping", timeout=3)
        if rc == 0:
            break
        time.sleep(1)
    else:
        print(json.dumps({"ok": False, "error": "harness did not come back after reload"}))
        return 1

    # Wait for the import files to appear (scan finished).
    imp = os.path.join(models, "roblox_house.obj.import")
    deadline = time.time() + 60
    while time.time() < deadline and not os.path.exists(imp):
        time.sleep(1)

    subprocess.run([sys.executable, os.path.join(HERE, "window-home.py")],
                   cwd=ROOT, capture_output=True, text=True)
    time.sleep(1)

    # The restored read-only glb tab raises a confirmation dialog.
    for _ in range(3):
        wins, _ = drive("windows")
        if any(w.get("visible") and "Please Confirm" in str(w.get("title", ""))
               for w in wins.get("windows", [])):
            subprocess.run([sys.executable, os.path.join(HERE, "click-window-button.py"),
                            "Please Confirm", "Cancel"], cwd=ROOT,
                           capture_output=True, text=True)
            time.sleep(0.5)
        else:
            break

    if a.open:
        drive("open-scene", a.open)
        time.sleep(2.5)
        # A restored glb tab can raise the dialog again after the tab switch.
        wins, _ = drive("windows")
        if any(w.get("visible") and "Please Confirm" in str(w.get("title", ""))
               for w in wins.get("windows", [])):
            subprocess.run([sys.executable, os.path.join(HERE, "click-window-button.py"),
                            "Please Confirm", "Cancel"], cwd=ROOT,
                           capture_output=True, text=True)
            time.sleep(0.5)

    info, _ = drive("info")
    print(json.dumps({"ok": True, "models": models, "imported": os.path.exists(imp),
                      "scene": info.get("scene"), "window": info.get("window_pos")}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
