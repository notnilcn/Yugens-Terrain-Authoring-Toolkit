# Screenshot index

All evidence is local and git-ignored (`qa/screenshots/`). PNGs are in-engine editor captures
unless noted; `shot` images include the Xvfb desktop and are used for window-size evidence.
UI dumps (`ui/*.json`) are `qa/drive.py ui --root all --save` control trees with rects,
`min`, `clipped` and `outside_root`.

The addon was tested at 1600×900 by default; Round 3 adds 1920×1080, 1366×768, 1280×720 and
1024×600. The editor minimum is 1024×600; an 800×600 request is clamped.

## Round 3 (r4) — 2026-11-21

Nine area passes plus a fresh-session verification pass; 591 PNGs and 60 UI dumps. Default editor
1600×900; resizability matrix at 1920×1080, 1600×900, 1366×768, 1280×720 and 1024×600.

| Directory | PNG | UI dumps | Report | What it shows |
|---|---|---|---|---|
| `qa/screenshots/r4-sculpt/` | 50 | 7 | `qa/findings/r4-sculpt.md` | Brush/Level/Smooth/Bridge strips, selection modifiers, Raise/Lower, wheel, toolbar, undo/redo, scene-close panic |
| `qa/screenshots/r4-paint/` | 59 | 7 | `qa/findings/r4-paint.md` | Paint slots/presets/export, panel height, Paint Walls, foliage paint/fill/clear, preset undo |
| `qa/screenshots/r4-manage/` | 62 | 14 | `qa/findings/r4-manage.md` | Inspect, Chunks, all six Settings pages, Storage, Stamp, Gradient, Select, Tint, enum bugs |
| `qa/screenshots/r4-pixel/` | 82 | 8 | `qa/findings/r4-pixel.md` | 15 Pixelizer sections, 9 Renderer pages, Pixel World, presets, `on_float` panic, 1024 panel |
| `qa/screenshots/r4-effects/` | 89 | 13 | `qa/findings/r4-effects.md` | Sky, Day/Night, all weather types, water, fireflies, birds, smoke, particles, lights, shafts, erosion, wild flowers |
| `qa/screenshots/r4-builder/` | 43 | 3 | `qa/findings/r4-builder.md` | Builder modes, 39 px Face viewport, editor shortcut collisions, mesh undo |
| `qa/screenshots/r4-uiresize/` | 60 | 3 | `qa/findings/r4-uiresize.md` | Size × surface matrix, 39 px viewports, fixed 250 px panels, Pixelizer/Builder crashes, dialog sizing |
| `qa/screenshots/r4-undodeep/` | 72 | 5 | `qa/findings/r4-undodeep.md` | 42-row Ctrl+Z/Ctrl+Shift+Z matrix, 19-chunk stroke, context/focus cases |
| `qa/screenshots/r4-runtime/` | 71 | — | `qa/findings/r4-runtime.md` | Hex Island and tutorials 01–04, F1/F3, weather/time, orbit, live resize |
| `qa/screenshots/r4-verify/` | 3 | — | `ISSUES.md` | Fresh re-check: Floor Slot label-only, Debug View label-only, Pixelizer 1280 crash excerpt, Builder re-run |

Key round-3 evidence:

- Undo works: `r4-undodeep/` matrix + screenshots, `r4-sculpt/15..20`, `r4-paint/` scatter/fill
  pairs, `r4-manage/60-gridtype-undo.png`, `r4-builder/17-extrude-undo.png`.
- Undo/state failures: `r4-verify/01`, `r4-undodeep/49-floorslot.png` + `51-debugview-slope.png`,
  `r4-paint/54-preset-undo-stale-names.png`, `r4-pixel/11-dawn-undone.png`,
  `r4-undodeep/57-pixelsize-drag.png`.
- Crashes/layout: `r4-verify/03-pixelizer-crash-excerpt.txt`, `r4-uiresize/43-1366-pixelizer.png`,
  `r4-uiresize/52-1024-brush.png`, `r4-builder/04-mode-face-button.png`,
  `r4-uiresize/37-1600-builder-face.png`.
- Panels: `r4-paint/ui/02-paint-panel.json`, `r4-paint/52-panel-splitter-drag-left.png`,
  `r4-uiresize/19-1920-paint-panel-bottom.png`, `r4-pixel/73-resize-1024-renderer.png`.
- Runtime: `r4-runtime/37-hex-noon-normal.png`, `70-hex1024-f3-default.png`,
  `54-t02-zoom-out.png`, `02-hex-1280-f1.png`.

## Round 2 (2026-11-21)

| Directory | PNG | UI dumps | Report | What it shows |
|---|---|---|---|---|
| `qa/screenshots/r2-sculpt/` | 60 | 12 | `qa/findings/r2-sculpt.md` | Tool strips, Brush selection/modifiers, Raise/Lower clipping, Level/Smooth/Bridge strips, undo/redo, key shortcuts |
| `qa/screenshots/r2-paint/` | 53 | 16 | `qa/findings/r2-paint.md` | Paint panel, 16 slots, paint/wall strokes, preset dialog/export, panel scrolling |
| `qa/screenshots/r2b-paint/` | 34 | 6 | `qa/findings/r2b-paint.md` | Clean paint pass: slot selection, two-stroke undo, Ctrl+LMB pick, slope bug, Save Preset, label truncation |
| `qa/screenshots/r2c-manage/` | 52 | 33 | `qa/findings/r2c-manage.md` | Inspect, Settings pages, Grid Type + undo, Debug View bug, Stamp + undo, tab-row movement |
| `qa/screenshots/r3-pixel/` | 18 | 5 | `qa/findings/r3-pixel.md` | Pixel Preview/Animate, Pixel World, Dawn preset + undo, disabled Add rows |
| `qa/screenshots/r2-runtime/` | 93 | — | `qa/findings/r2-runtime.md` | Hex Island 1280/1024, F1/F3, Rain, orbit/zoom, tutorial 01 |

Key round-2 evidence:

- Undo: `r2b-paint/04-paint-undo.png`, `05-two-strokes.png`, `r2-sculpt/15..20`
  (digests are in the reports), `r2c-manage/28-stamp-undo.png`.
- Undo failures: `r2-paint/ui/preset-*.json` (texture preset),
  `r2c-manage/22-debugview-popup-open.png` + `23-debugview-slope-result.png`,
  `r3-pixel/12-dawn-undone.png`.
- Layout: `r2-sculpt/32-brush-raiselower-strip.png`, `r2-paint/17-paint-reopen.png`,
  `r2b-paint/19-panel-export-bottom.png`, `r2c-manage/07-settings-grid.png` vs
  `18-settings-grass-reopen.png`.
- Runtime: `r2-runtime/19-hex-zoom-out.png`, `13-hex-f1-reshown.png`,
  `35-hex-1024-wait.png`.

## Prior full pass (2026-11-20)

| Directory | PNG | Report |
|---|---|---|
| `qa/screenshots/sculpt/` | 93 | `qa/findings/sculpt.md` |
| `qa/screenshots/paint/` | 62 | `qa/findings/paint.md` |
| `qa/screenshots/manage/` | 73 | `qa/findings/manage.md` |
| `qa/screenshots/pixel/` | 101 | `qa/findings/pixel.md` |
| `qa/screenshots/effects/` | 82 | `qa/findings/effects.md` |
| `qa/screenshots/builder/` | 36 | `qa/findings/builder.md` |
| `qa/screenshots/uiresize/` | 51 | `qa/findings/ui.md` |
| `qa/screenshots/undodeep/` | 62 | `qa/findings/undodeep.md` |
| `qa/screenshots/runtime/` | 52 | `qa/findings/runtime.md` |

Prior key evidence still cited by `ISSUES.md`: `qa/findings/ui-crash.txt` (1024×600 container
loop log), `uiresize/01..04` (viewport/strip matrix), `builder/14..15` (Face mode 39 px
viewport), `undodeep/` (15-stroke undo matrix), `effects/` (weather/erosion/wildflowers).

## Taking new evidence

```bash
qa/session.sh start myarea --display :31 --port 8811
source /tmp/opencode/qa/sessions/myarea/env.sh
python3 qa/drive.py capture qa/screenshots/myarea/01-surface.png
python3 qa/drive.py ui --root all --save qa/screenshots/myarea/ui/01-surface.json
qa/session.sh stop myarea
```

Use `capture` for clean UI evidence and `shot` for window-size/resizability evidence. The
harness is enabled only in the disposable session copy; the repository's `project.godot` is
never modified.
