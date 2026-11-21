#!/usr/bin/env python3
"""Pixelizer visibility sweep: with a Pixel* node selected, resize through the five
QA sizes and record the rail / strip / Pixel World panel rects."""
import json
import os
import subprocess
import sys
import time

BUILD = "/home/clintonnguyen/g/code_examples/rrr-wt/r4base"
OUT = "/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r5-ui-runtime"
TMP = "/tmp/opencode/r5t5"
sys.path[:0] = ["/home/clintonnguyen/g/code_examples/rrr/qa/scripts/r5-ui-runtime"]
import wave  # noqa: E402


def main():
    sizes = [(1920, 1080), (1600, 900), (1366, 768), (1280, 720), (1024, 600)]
    wave.drive("select", "--path", "PixelRenderer")
    time.sleep(5)
    for (w, h) in sizes:
        wave.resize(w, h)
        time.sleep(2)
        tag = f"{w}x{h}-70-pixelizer-sweep"
        cs = wave.dump_base(tag)["controls"]
        m = wave.measure(cs)
        # pixelizer-specific extras
        rail = [c for c in cs if c.get("name") == "PixelizerToolbarRail" and c.get("visible")]
        strip = [c for c in cs if c.get("name") == "PixelizerAttributes" and c.get("visible")]
        panel = [c for c in cs if c.get("class") == "MstSidePanel" and c.get("visible")]
        row = {
            "size": f"{w}x{h}",
            "view": m["view"],
            "rail": rail[0]["rect"] if rail else None,
            "strip": strip[0]["rect"] if strip else None,
            "strip_min": strip[0]["min"] if strip else None,
            "panel": panel[0]["rect"] if panel else None,
            "flow": strip and [c for c in cs if c.get("class") == "MstFlow"
                               and c["path"].startswith(strip[0]["path"])][0]["rect"],
        }
        print(json.dumps(row))
        with open(os.path.join(OUT, "pixelizer-sweep.jsonl"), "a") as f:
            f.write(json.dumps(row) + "\n")
        wave.capture(tag + ".png")
        wave.save_dump(tag, f"{w}x{h}")


if __name__ == "__main__":
    main()
