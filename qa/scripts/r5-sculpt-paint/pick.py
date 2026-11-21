#!/usr/bin/env python3
"""Open a popup by clicking (x,y), then click the item whose text matches (case-insens).
Usage: pick.py X Y ITEM_TEXT [--substr]
"""
import json, subprocess, sys, time

H = "/home/clintonnguyen/g/code_examples/rrr/qa/scripts/r5-sculpt-paint/h.sh"


def drive(*args):
    r = subprocess.run([H] + list(args), capture_output=True, text=True)
    try:
        return json.loads(r.stdout)
    except Exception:
        print("DRIVE FAIL", args, r.stdout[-800:], r.stderr[-400:])
        sys.exit(1)


x, y = int(sys.argv[1]), int(sys.argv[2])
want = sys.argv[3]
substr = "--substr" in sys.argv
drive("click", str(x), str(y))
time.sleep(1.2)
p = drive("popups")
pops = p.get("popups") or []
if not pops:
    print("NO POPUP after click", x, y)
    sys.exit(2)
pop = pops[-1]
px, py = pop["pos"]
pw, ph = pop["size"]
n = len(pop["items"])
idx = None
for it in pop["items"]:
    t = it["text"]
    if (want.lower() in t.lower()) if substr else (t.strip().lower() == want.strip().lower()):
        idx = it["index"]
        break
if idx is None:
    print("ITEM NOT FOUND", want, [i["text"] for i in pop["items"]])
    sys.exit(3)
cy = py + 8 + (idx + 0.5) * (ph - 16) / n
cx = px + pw / 2
drive("click", str(int(cx)), str(int(cy)))
time.sleep(1.2)
left = drive("popups").get("popups") or []
print(f"picked {want!r} at ({int(cx)},{int(cy)}) popup_left={len(left)}")
