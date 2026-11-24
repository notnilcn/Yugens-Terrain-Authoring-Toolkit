#!/usr/bin/env python3
"""Build docs/guide/assets/search-index.js (window.GUIDE_INDEX) and check media references.

Usage: python3 -I docs/guide/tools/build_index.py
"""
import html, json, os, re, sys
from html.parser import HTMLParser

root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def slug(s):
    return re.sub(r"^-|-$", "", re.sub(r"[^a-z0-9]+", "-", s.lower()))


class P(HTMLParser):
    def __init__(self):
        super().__init__()
        self.title = ""; self.cur = None; self.buf = []; self.recs = []; self.text = []; self.in_title = False
        self.refs = []; self.skip = 0
    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        for k in ("src", "poster", "href"):
            v = a.get(k)
            if v and not re.match(r"^(https?:|#|mailto:|data:)", v):
                self.refs.append(v.split("#")[0])
        if "data-frames" in a:
            try:
                self.refs += [f["src"] for f in json.loads(a["data-frames"])]
            except Exception:
                self.refs.append("<bad data-frames JSON>")
        if tag in ("script", "style"): self.skip += 1
        if tag == "title": self.in_title = True
        if tag in ("h1", "h2", "h3"):
            self.flush(); self.cur = {"tag": tag, "id": a.get("id"), "h": []}
    def handle_endtag(self, tag):
        if tag in ("script", "style"): self.skip -= 1
        if tag == "title": self.in_title = False
        if tag in ("h1", "h2", "h3") and self.cur: self.cur["done"] = True
    def handle_data(self, d):
        if self.skip: return
        if self.in_title: self.title += d; return
        if self.cur is not None and not self.cur.get("done"): self.cur["h"].append(d)
        else: self.text.append(d)
    def flush(self):
        if self.cur:
            h = " ".join("".join(self.cur["h"]).split())
            self.recs.append({"heading": h, "id": self.cur["id"] or slug(h),
                              "text": " ".join(" ".join(self.text).split())[:600]})
        self.text = []


index, missing = [], []
for fn in sorted(os.listdir(root)):
    if not fn.endswith(".html"):
        continue
    p = P(); p.feed(open(os.path.join(root, fn), encoding="utf-8").read()); p.flush()
    title = p.title.split("–")[0].split(" - ")[0].strip() or fn
    for r in p.recs:
        index.append({"file": fn, "title": title, **r})
    for ref in p.refs:
        if ref.endswith(".html") or ref.startswith("assets/"):
            if not os.path.exists(os.path.join(root, ref)) and ref != "assets/search-index.js":
                missing.append((fn, ref))
        elif not os.path.exists(os.path.join(root, html.unescape(ref))):
            missing.append((fn, ref))

with open(os.path.join(root, "assets", "search-index.js"), "w", encoding="utf-8") as f:
    f.write("window.GUIDE_INDEX = " + json.dumps(index, ensure_ascii=False) + ";\n")
print(f"indexed {len(index)} sections")
for fn, ref in missing:
    print(f"MISSING {fn}: {ref}")
sys.exit(1 if missing else 0)
