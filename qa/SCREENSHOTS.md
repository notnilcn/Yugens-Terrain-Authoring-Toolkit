# QA screenshots and findings index

The evidence (649 PNGs, ~960 MB) for [`ISSUES.md`](../ISSUES.md) and
[`FEEDBACK.md`](../FEEDBACK.md). PNGs are
full editor/game captures (or crops named `*-crop*`), stored under `qa/screenshots/<area>/`;
`ui/` subfolders hold the matching `drive.py ui --save` JSON dumps (control rects, min sizes,
text, values) plus a few close-up captures. The tree is ~824 MB and is **git-ignored** (`qa/screenshots/`); commit selected
shots explicitly with `git add -f` if you want them versioned.

| Area | PNGs | Focus | Report |
|---|---|---|---|
| `sculpt/` | 93 | Brush (Drag and Raise/Lower), Level, Smooth, Bridge, modifiers, toolbar/grid, undo matrix | `findings/sculpt.md` |
| `paint/` | 62 | 16 texture slots, presets/export, Paint Walls, Foliage, grass, fill/clear | `findings/paint.md` |
| `manage/` | 73 | Inspect, Chunks, Settings pages, Storage save/bake/GLB, Stamp, Gradient, Select, Tint | `findings/manage.md` |
| `pixel/` | 108 | Renderer pages, Palette/LUT, Object/OutlineColors, Preview/Animate, Pixel World | `findings/pixel.md` |
| `effects/` | 112 | Sky, Day/Night, Weather, Water, fireflies/birds/smoke/fire/particles/lights/shafts, erosion, wild flowers, feature switches | `findings/effects.md` |
| `builder/` | 36 | Builder UI, modes, draw/extrude/paint/materials/UV/save, both layouts | `findings/builder.md` |
| `runtime/` | 52 | Six demo/tutorial scenes, three window sizes, F1/F3, orbit/zoom/WASD | `findings/runtime.md` |
| `uiresize/` | 51 | Every surface at 1600×900 and 1024×600 (plus 1920/1366/1280 for Brush), dialogs, popups | `findings/ui.md` |
| `undodeep/` | 62 | Deep undo/redo matrix and the remaining gaps | `findings/undodeep.md` |

Notable evidence:

- `sculpt/undo-matrix.txt` — the per-tool Ctrl+Z/Ctrl+Shift+Z matrix with digests.
- `builder/14-box-selected-face-mode.png` — the 345×39 px viewport caused by the 447 px face
  strip.
- `uiresize/04-brush-1024x600.png` — 39 px viewport and the 7-row strip at 1024×600.
- `uiresize/31-pixelizer-1024x600.png` — Pixel World panel only 17 px on screen.
- `uiresize/50-save-preset-1024x600.png` — the Save Preset dialog (OK only).
- `findings/ui-crash.txt` — excerpt of the 105,978-line editor log for the 1024×600
  Level/Smooth/Tint crash.
- `runtime/03-hex-1280-f3.png` and `35-hex-1024-f3.png` — F3 tuner at 720p vs 600p.

The per-area markdown files list the exact screenshot → finding mapping and the "Covered" /
"Not tested" checklists. Editor logs and temporary session copies live under
`$MST_QA_STATE/sessions/<name>/` (default `/tmp/opencode/qa`) and are disposable; the crash
excerpt above is the only log preserved in the repository.
