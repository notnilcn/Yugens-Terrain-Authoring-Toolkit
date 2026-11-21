#!/usr/bin/env python3
"""Sea/void + plate scan for #38 (pure PIL, no numpy).

usage: scan.py IMG [--crop x,y,w,h]
"""
import sys
from PIL import Image, ImageStat, ImageFilter, ImageChops

path = sys.argv[1]
box = None
if "--crop" in sys.argv:
    i = sys.argv.index("--crop")
    box = tuple(int(v) for v in sys.argv[i + 1].split(","))

im = Image.open(path).convert("RGB")
if box:
    x, y, w, h = box
    im = im.crop((x, y, x + w, y + h))
w, h = im.size
print(f"{path}  {w}x{h}")
print("mean", tuple(round(v, 1) for v in ImageStat.Stat(im).mean))

bottom = im.crop((0, int(h * 0.85), w, h))
print("bottom15 mean", tuple(round(v, 1) for v in ImageStat.Stat(bottom).mean))

rows = []
for y in range(int(h * 0.75), h):
    st = ImageStat.Stat(im.crop((0, y, w, y + 1)))
    rows.append((y, st.stddev[0], st.mean[0]))
flat = [r for r in rows if r[1] < 2.0]
print(f"bottom-quarter rows: {len(rows)}, flat(stdev<2): {len(flat)}"
      + (f" (y {flat[0][0]}..{flat[-1][0]}, mean {flat[0][2]:.1f})" if flat else ""))

reg = im.crop((0, int(h * 0.6), w, h))
L = reg.convert("L")
edge = L.filter(ImageFilter.FIND_EDGES)
low = edge.point(lambda v: 255 if v < 8 else 0)
bright = L.point(lambda v: 255 if v > 140 else 0)
both = ImageChops.multiply(low, bright)
hist = both.histogram()
npx = int(hist[128:].__len__() and sum(hist[128:]))
print(f"flat-pale px (bright>140, low local grad) in bottom 40%: {npx} / {reg.size[0]*reg.size[1]}")


def transect(y):
    px = im.crop((0, y, w, y + 1)).convert("L").getdata()
    return list(px)


for frac in (0.55, 0.70, 0.85):
    y = int(h * frac)
    line = transect(y)
    k = 9
    s = [sum(line[max(0, i - k // 2):min(w, i + k // 2 + 1)]) / (min(w, i + k // 2 + 1) - max(0, i - k // 2)) for i in range(w)]
    edges = []
    for i in range(1, w):
        if abs(s[i] - s[i - 1]) > 6:
            if edges and i - edges[-1][-1] <= 12:
                edges[-1].append(i)
            else:
                edges.append([i])
    print(f"transect y={y} ({frac:.2f}h): mean={sum(line)/w:.1f}; edges>6 at x={[int(sum(g)/len(g)) for g in edges][:12]}")
