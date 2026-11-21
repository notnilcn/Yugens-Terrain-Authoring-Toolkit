#!/usr/bin/env python3
"""Walk every pixelizer rail section; record strip/header/tab geometry."""
import json, os, subprocess, sys, time

DRIVE = "python3"
outdir = sys.argv[1]
qs = os.environ.get("MST_PROJECT", "")
RAIL = [
    ("Renderer", 147), ("Sky", 192), ("DayNight", 228), ("Weather", 264),
    ("Water", 300), ("Fireflies", 336), ("ParticleField", 372), ("Birds", 408),
    ("SmokeFire", 444), ("LightVolumes", 480), ("PixelObject", 525),
    ("GridFog", 570), ("Erosion", 606), ("WildFlowers", 642), ("GrassFire", 678),
]


def drive(*args):
    return subprocess.run([DRIVE, "qa/drive.py", *args], capture_output=True, text=True)


def rect(d):
    r = d["rect"]
    return r


rows = []
for name, y in RAIL:
    drive("click", "304", str(y))
    time.sleep(1.5)
    path = os.path.join(outdir, f"rail-{name}.json")
    drive("ui", "--root", "all", "--save", path)
    data = json.load(open(path))
    cs = data["controls"]
    strip = next((c for c in cs if c["name"] == "PixelizerAttributes"), None)
    pages = next((c for c in cs if c["name"] == "SectionPages"), None)
    title = next((c for c in cs if c["name"] == "SectionTitle"), None)
    tabs = []
    if pages:
        for c in cs:
            if "SectionPages/" not in str(c["path"]):
                continue
            r = c["rect"]
            tabs.append((c["name"], c["text"], r[0], r[2], c["clipped"]))
        tabs.sort(key=lambda t: t[2])
    rows.append({
        "section": name,
        "strip": strip["rect"] if strip else None,
        "title": title["text"] if title else None,
        "pages": pages["rect"] if pages else None,
        "tabs": [[t[1], round(t[2]), round(t[3]), t[4]] for t in tabs],
    })
    print(json.dumps(rows[-1]))

json.dump(rows, open(os.path.join(outdir, "rail-sweep.json"), "w"), indent=1)
