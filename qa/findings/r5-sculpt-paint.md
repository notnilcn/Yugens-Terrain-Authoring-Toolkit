# r5 — T1 Sculpt / Paint / Foliage / textures

Session `r5-t1` — display `:51`, port `8851`, build `/home/clintonnguyen/g/code_examples/rrr-wt/r4base`
(Godot 4.7.2, llvmpipe, 1600×900 unless stated). Real X11 input through `qa/drive.py`.
Scenes: `demo/hex_island.tscn` (hex rings) and `tutorial/02_island.tscn` (square grid).
Screenshots: `qa/screenshots/r5-sculpt-paint/` (101 PNG + 7 UI dumps in `ui/`).
Log: `/tmp/opencode/qa/sessions/r5-t1/editor.log` (excerpt copied to
`qa/scripts/r5-sculpt-paint/log-excerpt.txt`).

Verified **working** in this build and worth keeping: digit tool shortcuts (`1`–`9`), compact
toolbar at 40 px, disabled+tooltip Grid Aligned, distinct undo names for Terraced/Sloped, the
rewritten **Save Preset** dialog (Cancel/OK, live path, empty-name validation), the **Paint Walls**
hint/status, the **Foliage** panel (real Control panel, not a canvas overlay), the noise texture
previews, and the per-class undo/redo digest round-trips.

---

## Finding — Attribute strips are still uncapped: the Paint strip runs off the window at 1280×720 / 1024×600

- **Severity:** error (documented controls unreachable; painting is not possible below 1600×900)
- **Steps:** 1) Open `tutorial/02_island.tscn`, select `MarchingSquaresTerrain`, click **Paint**.
  2) `resize 1280 720`, wait 3 s, `ui --root all`. 3) `resize 1024 600`, repeat.
- **Expected:** the strip caps at ~3 rows (≈105 px) and scrolls inside itself, keeping the
  Material / Texture Name / Texture Preset / Paint Walls / Slope controls reachable.
- **Actual:** the strip is squeezed to **319 px wide** (120 px toolbar + 250 px panel + inspector)
  and wraps to **319×385**. At 1280×720 the strip starts at y=498 so its bottom is 163 px **below
  the window edge**; at 1024×600 it starts at y=454 and only the first row (Brush Type/Size/Aspect)
  is visible — Material, Texture Name, Texture Preset, Paint Walls, Slope Min/Max are all
  off-window with no reachable scrollbar. Viewport shrinks to 333×341 (1280) and 319×300 (1024).
- **Evidence:** `qa/screenshots/r5-sculpt-paint/94-1280-paint.png`,
  `95-1024-paint.png`; UI dumps `ui/06-paint-1280.json` (`MstFlow [410,498,319,385]`),
  `ui/05-paint-1024.json` (`MstFlow [302,454,319,385]`, panel `[638,154,250,399]`); at 1600 the
  same strip is 650×175 (`ui/07-paint-strip-1600.json`). No log line (layout only).
- **Undo:** n/a. This is r4 issue **7** (`strips have no height cap`) reproducing with a new
  manifestation — instead of collapsing the viewport to 39 px it now clips the strip below the
  window.

---

## Finding — Save Preset dialog: missing `save_preset_dialog_size` key logs an ERROR and the size is never remembered

- **Severity:** warning (console error on every open; the documented "remembers size" behaviour is broken)
- **Steps:** 1) Paint tool → **Export Texture Preset**. 2) Resize the dialog to 800×220
  (`window-resize 800 220 --title "Save Preset"`), Cancel. 3) Reopen the dialog.
- **Expected:** the dialog remembers its size; no console error.
- **Actual:** every open logs
  `ERROR: Couldn't find the given section "marching_squares_terrain" and key "save_preset_dialog_size", and no default was given.`
  (`at: get_value (core/io/config_file.cpp:60)`) and the dialog reopens at its default **420×179**,
  not 800×220.
- **Evidence:** `qa/screenshots/r5-sculpt-paint/61-export-preset-dialog.png` (420×179, OK+Cancel,
  live "Saved as res://…/tutorial.tres"), `62-save-preset-narrow400.png` (400×150 request honoured
  as 400×179), `63-save-preset-wide800.png`, `65-preset-reopen.png` (back to 420×179);
  `editor.log` line 81; `qa/scripts/r5-sculpt-paint/log-excerpt.txt`.
- **Undo:** n/a. The rest of r4 issue **28** is fixed: Cancel exists, min size 340×140 (real, and
  honoured), it opens compact at 420×179 instead of 558×150, and the path it will write is shown.

---

## Finding — Foliage "Fill Terrain" records an undo step but paints nothing (Tree)

- **Severity:** warning (silent no-op; history entry for nothing)
- **Steps:** 1) `02_island`, Foliage tool, palette = **Tree**, Size 18.9, Density 0.250,
   Slope Min 0, Slope Max 90. 2) Hover the island and read the status (`862 props`).
   3) Click **Fill Terrain**, wait 6 s, re-read the status and the viewport.
- **Expected:** the island is filled with tree props (hundreds) and the prop count jumps.
- **Actual:** the status stays `862 props` (one run read 863→862), the viewport is unchanged, but
  `undo-state` reports `action: terrain foliage fill`, `version` +1. Repeating with Grass selected
  is correctly refused with the hint `Fill Terrain / Clear act on props: select one or more props
  (not only Grass) in the palette first`; Clear **does** work (Tree: 862→766 props, undo restores
  862).
- **Evidence:** `qa/screenshots/r5-sculpt-paint/80-foliage-tree-props-before.png` (863 props),
  `81-foliage-tree-fill-props-after.png` (862 props, no visual change),
  `77-foliage-fill-terrain.png` (no change), `82-foliage-tree-cleared.png` (766 props, Clear works),
  `84-foliage-clear-undone.png` (862 restored).
- **Undo:** broken for Fill (an action exists, but there is nothing to undo). Clear undo/redo works.

---

## Finding — Hint block never wraps; the first line is clipped mid-word at 1280 / 1024

- **Severity:** ui (guidance lost exactly where it is needed)
- **Steps:** 1) Paint tool, **Paint Walls** on, terrain selected. 2) `resize 1280 720` and
  `resize 1024 600`; read the hint block above the strip.
- **Expected:** each hint line fits, wraps, or shortens with width.
- **Actual:** the hint is a fixed 3-line block; line 1 ends mid-word at the viewport's right edge:
  `Paint Walls: LMB-drag paints the Material on cliff wa…` (no wrap, no ellipsis). At 1024 the same
  clip occurs at a narrower point.
- **Evidence:** `qa/screenshots/r5-sculpt-paint/94-1280-paint.png`, `95-1024-paint.png`;
  contrast `86-palette-flowers-hint.png` (fits at 1600). No log line.
- **Undo:** n/a. Related: r4 issue **43** (status/hint clipping). The Level-hint part of 43 is
  **fixed** — it now reads `LMB-drag: level to Height · Ctrl+LMB: pick the hovered height ·
  Shift+wheel: size` with no phantom strength control (`51-level-strip-hint-02island.png`).

---

## Finding — Status block grows to three lines and rests on the strip edge

- **Severity:** ui (readability)
- **Steps:** Brush → Drag, Alt-click one cell so the status carries cell + chunk + count, then read
  the bottom of the viewport at 1600×900.
- **Expected:** a status line that fits, or is padded away from the strip.
- **Actual:** the status wraps to three lines (`… · Shift: add` / `RMB/Esc: clear`) and the last line
  sits flush on the strip's top edge; it also repeats the modifier list already printed in the
  hint above it.
- **Evidence:** `qa/screenshots/r5-sculpt-paint/12-status-alt-crop.png`,
  `11-mod-alt-single.png`.
- **Undo:** n/a.

---

## Finding — Grid Aligned stays on but disabled after a hex → square scene switch

- **Severity:** minor / ui (an unavailable option cannot be turned off)
- **Steps:** 1) `demo/hex_island.tscn`, Brush Raise/Lower, Falloff off, click **Grid Aligned**
   (check goes on). 2) Open `tutorial/02_island.tscn` (square grid) and select the terrain.
- **Expected:** the option is cleared (or stays usable) for a grid where it does not apply.
- **Actual:** the checkbox stays **checked** and becomes **disabled** (drawn dimmed with a tick),
   tooltip `Grid Aligned needs a Triangle/Hexagon/Hex Rings grid and Falloff off.` It cannot be
   cleared without returning to a hex/triangle scene. It appears inert (a single click still
   selected 93 cells), so the impact is state hygiene only.
- **Evidence:** `ui/04-grid-aligned-square-disabled.json` (`toggled: true, disabled: true`),
  `51-level-strip-hint-02island.png`, `27-grid-aligned-on.png` (hex, togglable).
  r4 issue **41** itself is **fixed** — the control is now disabled with an explanatory tooltip
  instead of silently refusing.
- **Undo:** n/a (tool setting, not history).

---

## Finding — Raise/Lower pick hint is ambiguous and plain MMB does nothing

- **Severity:** minor / ui
- **Steps:** Brush → Brush Mode `Raise / Lower`; read the hint; hover a cell at height 8.39
  (Ceiling pick), then MMB-click it, then Ctrl+MMB-click it.
- **Expected:** the hint names the real bindings.
- **Actual:** the hint reads `Ctrl+LMB / MMB / RMB: pick Ceiling / Height / Floor`, which reads as
  three separate buttons, but plain MMB does nothing (Height stays 4.9735) while **Ctrl+MMB** picks
  Height (8.3905). Ctrl+LMB → Ceiling and Ctrl+RMB → Floor both work.
- **Evidence:** `qa/screenshots/r5-sculpt-paint/28-rl-raise.png` (hint),
  `31-rl-hover-height.png`; strip dumps taken right after each pick (Height stays 4.9735 after
  plain MMB, 8.3905 after Ctrl+MMB) — helper scripts under
  `qa/scripts/r5-sculpt-paint/` reproduce the sequence.
- **Undo:** n/a (picks create no history).

---

## Finding — Value labels drift from the slider value after wheel edits

- **Severity:** minor (misleading numbers)
- **Steps:** Brush Raise/Lower, Size 15.0 / Strength 4.000. Over the 3D view: Shift+wheel ×3,
  Ctrl+wheel ×2; read the strip, then `ui`.
- **Expected:** the label shows the control's value.
- **Actual:** labels update immediately (r4 issue **22** is fixed — nothing stays at 15.0/4.000),
  but they disagree with the control: label `18.9` vs `HSlider.value 19.0`; label `4.844` vs
  `HSlider.value 4.7595`. Same class in the initial strip: `Aspect 1.000` vs `value 1.0085`,
  `Peak 0.000` vs `0.01`.
- **Evidence:** `qa/screenshots/r5-sculpt-paint/23-rl-strip-before-wheel.png`,
  `24-rl-strip-after-wheel.png`, `25-rl-after-altwheel.png` (Rotation 45 after Alt+wheel ×3).
- **Undo:** n/a (tool settings).

---

## Finding — New scene-load log noise (not from `mst` scripts)

- **Severity:** minor (log only; triage)
- **Steps:** open `demo/hex_island.tscn` in a fresh session; read `editor.log`.
- **Actual:** 16 ×
  `WARNING: Plane cannot be normalized, the distance and the normal should be finite.`
  + 16 × `WARNING: Vector3 cannot be normalized …`, and 2 ×
  `ERROR: Condition "!res" is true. Returning: false` at
  `create_frustum_points (servers/rendering/rendering_light_culler.cpp:682)`.
  No `ERROR` / `SCRIPT ERROR` from `mst`/`MarchingSquares` anywhere in the 83-line log except the
  Save Preset one above.
- **Evidence:** `qa/scripts/r5-sculpt-paint/log-excerpt.txt`, `editor.log` lines 11–77 (2, 28, 62),
  79 (Inspect output), 81.
- **Undo:** n/a.

---

## Covered

Sculpt (hex + square grids)
- Tool switching by click for Brush / Level / Smooth / Bridge, with strip title + row set captured
  (`01`–`04`).
- Brush Drag: LMB-drag paints a cell selection (`135 … 180 selected`), second drag on white cells
  raises → `terrain height draw`, digest 3064656926 → 2905084564.
- Modifiers: Shift adds (97→180), Ctrl removes (180→98), Alt replaces with one cell (1 selected),
  Esc clears and the HUD returns to `LMB: select cells` **immediately** (r4 issue 26 **not**
  reproduced — no stale "N selected"), RMB clears.
- Raise / Lower: LMB raise (→ `terrain raise/lower`), RMB lower, LMB+RMB flatten (all three change
  the digest and record one action each); Ctrl+LMB picks Ceiling, Ctrl+MMB picks Height, Ctrl+RMB
  picks Floor.
- Brush Type Round vs Square on the square grid: 93 vs 117 cells from one click, visibly square
  footprint (`45`, `49`); Hexagon/Hexagon30/Triangle/Triangle180 present in the popup.
- Size (Shift+wheel 15→19), Strength (Ctrl+wheel 4.008→4.76), Rotation (Alt+wheel 0→45) all change
  the tool and the labels refresh (r4 issue 22 fixed).
- Aspect (→ 5.0 / 1.31) and Rotation visibly change the footprint; Grid Size editable; Quick Paint
  dropdown present (`None` + list).
- Grid Aligned: disabled with tooltip while Falloff is on (`ui` dump), toggles once Falloff is off.
- Level: Ctrl+LMB picks height (4.9735), drag levels a plateau (`terrain level`, digest
  3064656926 → 3539599882).
- Smooth: Terraced (`terrain smooth terraced`) and Sloped (`terrain smooth sloped`), Strength 0.152,
  visibly different results.
- Bridge: drag A→B builds a ramp (`terrain bridge`).
- Undo/redo: one stroke → Ctrl+Z restores the exact digest → Ctrl+Shift+Z redoes; two strokes → two
  Ctrl+Z in reverse order → 2699816579 → 3658963193 → 3317084947 then two redos; undo **after
  switching tools**; undo **after clicking the 3D view** (Inspect active).
- Toolbar: Collapse → 40 px single-column rail (expanded 120 px, also single column) — r4 issue 17
  not reproduced at 1600; Grid overlay toggles (`show_grid_in_editor`) and hex cell outlines are
  visible at close zoom (`37`/`38`); tooltips present on every tool button; digits `1`,`2`,`3`,`4`,
  `6`,`9` switch Brush/Level/Smooth/Bridge/Paint/Settings (r4 issue 9 fixed).

Paint / textures (02_island)
- 16 material slots (Grass, Meadow, Forest Floor, Sand, Dirt, Cliff, Dark Rock, Snow, Gravel, Clay,
  Moss, Path, Stone Tiles, Ash, Ice, Void), previews render (noise textures are grainy, not black),
  panel collapse tab (250 → 24 px) and re-expand.
- Vertex paint stroke → `terrain vertex paint`, digest 1822109369 → 544613470, Ctrl+Z restores
  1822109369 exactly, Ctrl+Shift+Z returns 544613470.
- Ctrl+LMB pick: Material 1. Grass → 2. Meadow from the painted cell.
- Material dropdown **refreshes after undo** (2. Texture 2 → 2. Meadow); slot labels return to
  Grass/Meadow — r4 issue 13's paint class not reproduced.
- Slope Min/Max clamp at 0–90 (overshoot drags, including a drag 300 px past the widget, stop at
  90.0 — no 6702° overflow; r4 issue 10 not reproduced for slope).
- Texture preset: `MST Classic` applies (terrain and slot names change, Material → 2. Texture 2),
  action `terrain texture preset`, Ctrl+Z restores data + labels.
- Paint Walls: toggle is honoured, hint switches to `Paint Walls: …` and status to
  `Wall Painting | cell (…) chunk (…)`, action `terrain wall paint`. r4 issue 29 fixed.
- Export Texture Preset: dialog 420×179 (min 340×140, resizable, Cancel + OK, live
  `Saved as res://addons/…/texture_presets/<name>.tres`), 400×150 request honoured as 400×179,
  800×220 accepted, empty name → inline `Enter a name to save the preset.` + OK disabled
  (r4 issue 30 fixed), saving writes `texture_presets/r5preset.tres`, `None`/`MST Classic`/`r5preset`
  listed in the Texture Preset dropdown.
- Foliage panel: 6 palette entries (Grass, Tree, Rock, + Pine, + Bush, + Flowers) + Edit / Remove /
  Add Mesh-Scene; selecting each updates the hint (`Painting Rock`, `Painting Flowers`, …).
- Foliage paint: Tree stroke adds a visible prop and `terrain foliage paint props`, Ctrl+Z removes
  it; Flowers stroke 862 → 1049 props; RMB erase 1049 → 963 (`terrain foliage erase props`).
- Clear (Tree) 862 → 766 props with `terrain foliage clear`, undo restores 862.
- Grass: `grass_enabled` false removes the grass sheet, true restores it (`89`–`91`).

---

## UI feedback (for FEEDBACK.md)

All rects are `qa/drive.py ui` window-content px at 1600×900 unless stated.

1. **Paint/Foliage side panel: fixed 250 px with 5,764 px of content (≈7.9 screens).**
   `MstSidePanel MarchingSquaresTextureSettings [1063,121,250,732]`,
   `MarginContainer Content [1063,121,236,5764]`; only ~1.7 of 16 slots are visible at once and
   Export Texture Preset sits at the top (y=160). The **collapse tab works** (panel → 24 px), but
   there is no splitter and no width memory. Recommend a 200–420 px splitter plus a 2-column slot
   grid; content is ~8 screens of wheel scrolling for a 16-slot palette.
2. **The attribute strip is uncapped and loses the width fight.** At 1600 the Paint strip is
   650×175; at 1280 it is 319×385 and its bottom is 163 px below the window; at 1024 (600 px tall)
   only the first row is on screen (`94`, `95`). Cap the strip at ~3 rows (105 px) with its own
   scrollbar, and stop the panel from stealing strip width (float the panel over the viewport or
   give the strip the full width).
3. **Hint block: 3 fixed lines, no wrapping, clipped at the right edge.** Line 1 is cut mid-word
   at 1280/1024 (`94`, `95`); the status line repeats the same modifier list that the hint already
   prints, so the block is 3 lines tall for ~1 line of information.
4. **Status line has no vertical padding and no width cap.** With a selection it wraps to 3 lines
   and the last line sits on the strip's top border (`12-status-alt-crop.png`).
5. **Numeric affordances disagree with each other.** `MstValueLabel` shows `18.9` while
   `HSlider.value` is `19.0`, and `4.844` vs `4.7595` for Strength; Aspect/Peak show `1.000`/`0.000`
   for `1.0085`/`0.01`. Two decimals of rounding + the extra digit would remove the doubt.
6. Smaller, measured: dialogs are now sane — Save Preset 420×179, min 340×140, resizable, Cancel
   present (r4's 558×150 no-Cancel dialog is gone) — but it does not remember the 800×220 it was
   resized to. The toolbar is a single 120 px column expanded / 40 px collapsed at 1600 (r4's
   236/116 px 2–3 column behaviour is gone).

---

## New issues (not in r4 ISSUES.md)

1. **Save Preset size persistence throws a `ConfigFile` ERROR and never remembers the size** —
   `ERROR: Couldn't find the given section "marching_squares_terrain" and key
   "save_preset_dialog_size", and no default was given.` on every dialog open; reopen returns to
   420×179. Repro: Paint → Export Texture Preset → resize → Cancel → reopen. (warning)
2. **Non-wrapping hint lines are clipped at 1280/1024** — first line ends `…on cliff wa`. (ui)
3. **Grid Aligned stays toggled-but-disabled across a hex→square scene switch** and cannot be
   cleared without reopening a hex scene. (minor)
4. **Raise/Lower pick hint is ambiguous** (`Ctrl+LMB / MMB / RMB`); plain MMB does nothing, Ctrl+MMB
   picks Height. (minor)
5. **Foliage Fill Terrain records an undo step but paints nothing for Tree** (0 props added; the
   prior-round note reconfirmed). (warning)
6. **Status block grows to 3 lines and rests on the strip edge** at 1600 with a selection. (ui)
7. **Scene-load log noise:** 16 plane-normalize + 16 vector3-normalize warnings and 2
   `create_frustum_points` ERRORs when `demo/hex_island.tscn` loads (no `mst` frames in the stack;
   triage as renderer/light data). (minor, log)

### r4 issues re-tested — status

| r4 # | What it said | r5 verdict |
|---|---|---|
| 7 | Strips uncapped (39 px viewport) | **Still reproduces** — new form: strip clipped below the window at 1280/1024 (F1 above) |
| 9 | Sculpt `1`–`9` dead | **Fixed** — `1`,`2`,`3`,`4`,`6`,`9` all switch tools |
| 10 | Sliders accept absurd values (slope 6702°) | **Fixed for Slope Min/Max** (clamps at 90); other sliders in that issue not re-tested |
| 13 | Undo leaves UI stale (paint class) | **Not reproduced** — Material dropdown + slot labels refresh on undo |
| 16 | 250 px panels, 5,767 px content, no collapse | **Partly fixed** — collapse tab exists; still 250 px fixed, content 5,764 px, no splitter |
| 17 | Toolbar column count jumps; compact 116 px | **Not reproduced** at 1600 (120 px expanded, 40 px compact, 1 column) |
| 22 | Wheel changes leave Size/Strength labels stale | **Fixed** (labels refresh); residual label-vs-value drift (F8) |
| 26 | Esc leaves stale "N selected" HUD | **Not reproduced** — HUD resets immediately |
| 27 | Ambiguous undo names (Smooth) | **Fixed** — `terrain smooth terraced` / `terrain smooth sloped` |
| 28 | Save Preset: no Cancel, 558 px, forgets size | **Mostly fixed** — Cancel, 420×179, real min 340×140; still forgets size + logs an ERROR |
| 29 | Paint Walls hint says "Vertex Painting" | **Fixed** — `Paint Walls: …` hint, `Wall Painting | cell …` status |
| 30 | Empty preset name closes silently | **Fixed** — inline error, OK disabled, nothing written |
| 31 | `mst_noise.png` preview black | **Fixed** — grainy noise thumbs |
| 41 | Grid Aligned silently refuses with Falloff on | **Fixed** — disabled + explanatory tooltip |
| 43 | Status/hint clipped; Level hint phantom strength | **Partly fixed** — Level hint is correct now; clipping persists at 1280/1024 (F4) |

Also re-tested and clean: `Ctrl+Z` after a 3D-view click, after a tool switch, and no undo entry
from a pick or a resize.

---

## Not tested / limits

- Painting was not done on a **cliff face at close range** (wall paint records `terrain wall paint`
  and changes the wall texture set, but at the overview camera the 1–2 cell rim was not legible);
  no visual before/after of a repainted cliff.
- Quick Paint modes beyond confirming the dropdown; hexagon/triangle footprints (only Round/Square);
  Peak/Flatten values only confirmed writable, not visually compared.
- 16 slots: only Grass/Meadow/Texture-N and the MST Classic set were exercised; per-slot
  pattern/scale/color/sprite pickers untouched; overwrite-confirmation of an existing preset name
  not tested.
- Foliage: Rock/Pine/Bush/Flowers were selected and their hints confirmed, but only Tree and Flowers
  were painted; "Erase All" checkbox not exercised; Fill Terrain not retried for Rock/Bush.
- The resizability measurements are 1600/1280/1024 only (1280×720, 1024×600).
- Erosion undo was not exercised (no `MarchingSquaresErosion` child in either scene); digits `5`,
  `7`, `8` were not pressed (only `1`,`2`,`3`,`4`,`6`,`9`).
- `03_pixel_look`, `04_world_mood`, and the runtime game were out of scope for T1.
