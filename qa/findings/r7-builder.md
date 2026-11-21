# Builder QA (r7 V2) — verification on the r7 merge build

Session `r7-v2` on display `:92`, port `8892`, Godot **4.7.2**, Linux, Mesa **llvmpipe**,
editor scale 1.0, Xvfb 1920×1080, window content 1600×900 (resized during the pass). Build:
`/home/clintonnguyen/g/code_examples/rrr-wt/r4base` (**HEAD `3474990`** — `qa/runtime.sh` import
fix on top of the r7 merge `4b6811e`). Scene `res://demo/hex_island.tscn`, target
`BuilderRuins/Ruin` (209 verts / 401 edges / 210 faces, 2 material slots) plus a `Box` created
during the pass (8/12/6, saved then reloaded). The session was started once and stopped at the
end; it never crashed or restarted (one `Godot Engine v` line, **79** log lines total, the same
as boot — every action in the pass added no log output). Editor log:
`/tmp/opencode/qa/sessions/r7-v2/editor.log`, copy
[`editor-log-final.log`](/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r7-builder/editor-log-final.log).

Helpers written for this pass (read-only drivers, in `qa/scripts/r7-builder/`): `d.sh` (session
`drive.py` wrapper), `m.py` (layout rects), `strip.py` (strip + section tabs + visible section
fields), `tabs.py`/`u.py`/`tree.py`/`focus.py`/`w.py` (reused from r6), `crop.py` (PIL crops
for HUD/dialog evidence).

Measurements below are window-content px from `ui --root all`; the 3D viewport is
`@Node3DEditorViewportContainer@10229`, the strip is `PixelBuilderStripRoot`, the palette is
`PixelBuilderPalette`, the rail is `PixelBuilderToolbarRail`.

---

## Finding — Edge mode offers an Extrude section that has no controls (35 px empty tab row)

- Severity: **ui**
- Steps: 1) 1600×900, select `Ruin`, Edge mode. 2) Click the **Extrude** section tab in the strip.
- Expected: either the section shows edge-extrusion fields, or the tab is hidden (as in Vertex
  mode, where Extrude is removed from the tab row).
- Actual: the tab toggles on and the strip collapses to **35 px** — the tab row and nothing else:
  `strip_root [376,818,902,35]`, no field in the strip. Vertex mode hides Extrude
  (`Extrude visible=False`), so the two modes disagree about the same section.
- Evidence: `strip.py` output at 1600×900 for Edge/Extrude
  (`strip_root [376,818,902,35]`, tab row only, no field rows) vs Vertex/Ops
  (`strip_root [376,758,902,95]`, `Ops / Move / Paint`, `Extrude visible=False`).
- Undo: n/a.

## Finding — `undo-state` harness readout is empty for every builder action

- Severity: **minor** (QA harness, not the addon; r6 could read `Pixel Builder: Duplicate` here)
- Steps: run `qa/drive.py undo-state` right after a builder duplicate / slot add / bevel / paint.
- Expected: `action` = the builder action name, `has_undo: true`.
- Actual: `{"action": "", "has_undo": false, "has_redo": false, "undo_count": 0, "version": 1}`
  for the whole pass, while the **History** dock shows `Pixel Builder: Duplicate`,
  `Pixel Builder: Bevel`, `Pixel Builder: Paint`, `Pixel Builder: Create Box` and the HUD
  counts follow undo/redo. Undo itself works through `Ctrl+Z` (see below); only the harness
  readout is dead, so it cannot be used to verify builder undo.
- Evidence: History panel crops
  [`19h-history.png`](/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r7-builder/19h-history.png),
  [`33h-history.png`](/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r7-builder/33h-history.png),
  [`43h-history.png`](/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r7-builder/43h-history.png);
  `undo-state` output quoted here.

---

## r7 verification

### N11/N32, R6-7/R6-8 — palette splitter / collapse / 220 minimum / 480 view → **PASS**

| Size / state | Palette | 3D view | Strip root | Evidence |
|---|---|---|---|---|
| 1600×900, first expand (stored default) | 250 | **684×611** | 85 | `01` |
| 1600×900, grip narrowed | **220** (clamped) | **714×611** | 85 | `03` |
| 1600×900, grip widened | **420** (clamped) | **514×582** | 114 | (`04` family) |
| 1600×900, grip clicked (collapse) | **32** | **902×638** | 58 | `02`, `64` |
| 1600×900, 420 → collapse (the exact claim) | 420 → **32** | **514 → 902** | — | `64` |
| 1600×900, split dragger far left | 420 (clamped, was 537) | 514 (was 397) | 114 | `04` |
| 1600×900, split dragger far right | 220 (clamped) | 714 | 85 | — |
| 1280×800, resize (auto) | 32 | **585** | 114 | `05` |
| 1280×800, explicit expand | 220 | 397 | 143 | `07`, `08` |
| 1280×800, collapse again | 32 | **585** | 114 | — |
| 1280×800, split dragger left while forced open | 220 (no change) | 397 (no change) | 143 | — |
| 1024×600, resize (auto) | 32 | **585** | 99 | `06` |
| 1024×600, explicit expand | 220 | 397 | 99 | — |
| 1366×768 | 220 | **480** (floor) | 114 | `60` |
| 1920×1080 | 220 | 1034 | 58 | `60` |

- **Collapse gives the space back**: 1600 514→**902**, 1280 397→**585** (the r6 "collapse keeps
  277 px" is gone; the `‹` tab is 32 px wide).
- **220 px minimum applies and is saved**: grip drag right stops at 220 and the view grows to
  714; `mst_editor_state.cfg` records `panel_PixelBuilderPalette_width=220.0`.
- **Splitter obeys the panel limits**: no 537 px palette / 397 px view is reachable any more.
  At 1280/1366 the view floor holds (480 at 1366; the splitter cannot widen the panel past 220
  when the view has no budget).
- **View 585 at 1280×800 and 1024×600 (r6: 340)**: reproduced, but only in the auto-collapsed
  state (see UI feedback 1/2).
- Collapsed state persists: `panel_PixelBuilderPalette_collapsed=true` in the cfg.

### N12/R6-9 — Ctrl+D → **PASS** (including the History-dock focus case)

| Case (1600×900 unless noted) | Result | Evidence |
|---|---|---|
| Face mode, 1 face picked, 3D view focused, `Ctrl+D` | **pass** — 209/401/210 → **213/405/211**, History `Pixel Builder: Duplicate`, selection kept (1 face), no `Ruin2` | `16`, `17` |
| `Ctrl+Z` | **pass** — back to 209/401/210 (`selected: 0 faces`) | `18` |
| Face mode, 1 face picked, **History dock focused** (`@TabBar@35`), `Ctrl+D` | **pass (fixed)** — duplicated faces 213/405/211, History `Pixel Builder: Duplicate`, **no `Duplicate Node(s)`**, no `Ruin2` | `19`, `19h` |
| `Ctrl+Z` with the History dock still focused | **pass** — 209/401/210 | `20` |
| Face mode, **no** face, History dock focused | **pass** — HUD hint `Duplicate: select faces (Face mode)`, no node duplicate | `65` |
| Vertex mode, 3D view focused | **pass** — same hint, no node duplicate | `63` |

Node list stayed `Box`, `Ruin` throughout; no `Ruin2` was ever created. Note the r6 wording
"shows the builder hint" is the *no-selection* case; with a face picked the key now duplicates
faces even from the History dock, which is strictly better.

### R6-10 — HUD → **PASS**

- **Status line clears on mode change**: Face + `Ctrl+D` (no face) → line 3
  `Duplicate: select faces (Face mode)`; `4` (Vertex) → two lines only (`21` → `22`).
- **Clears on undo**: `+ Slot` (3 rows) with the hint on screen → `Ctrl+Z` → hint gone, palette
  back to 2 rows (`23`, `24`). Bevel undo and paint undo also cleared the line (`34`, `43`).
- **Clears on mesh change**: hint on screen, select the `Box` node → HUD switches to
  `8 verts 12 edges 6 faces` with no hint (`62`).
- **Line 2 fits**: one line at view 902 / 714 / **585** (`25`, `26`, `27`); wraps to three lines
  (nothing clipped) at view **480** (1366×768) and **397** (1280 forced expand) — `61`, `07`.

### R6-11 — strip stuck at the 175 px cap at 1280 → **PASS**

At 1280×800 after visiting the tall Extrude section:

| Mode / section | Strip root | Content |
|---|---|---|
| Face / Extrude | 175 | 223 (scrolls) — tall section, legitimate cap |
| Face / Ops | **118** (r6: 175) | 118 |
| Edge / Extrude (after the Extrude visit) | **89** (r6: 175) | 89 |
| Vertex / Ops (Extrude hidden) | **151** (r6: 175) | 151 |

At 1600×900 the same sections are 99 / 62 / 66 / 62 px. No pinned-175 remnant.

### N15 — builder file dialogs → **PASS**

`Builder ▸ Export OBJ…` → `Export builder mesh (OBJ)` **1050×700**, `min [500,300]`, resizable,
exclusive. `window-resize 400 200` → actual **[500,300]** and the body still fits (Path,
Favorites/Recent/Directories, File name, Cancel, Save). Evidence `50`, `51`.

### Section tabs → **PASS**

One section at a time at 1600: Extrude (99 px, Dist/Inset/Profile/Inset by/Trim at/Trim walls),
Ops (62 px, Subdiv/Grow/To new object), Move (66 px, Grid/Axes), Paint (62 px,
Slot/Color/swatches) — each shows only its own fields (`strip.py` dumps at each step).
Empty tab hidden: Vertex shows `Ops / Move / Paint` only (`Extrude visible=False`). Chosen tab
persists across modes (Paint kept Face→Edge; Extrude chosen in Edge falls back to Ops in Vertex).
Only disagreement: Edge offers Extrude with no fields — see the Finding above.

### Crash watch → **PASS**

`Ruin` selected, pointer parked over the toolbar rail (`320,300`), 2 s wait per size:
1024×600, 1280×720, 1366×768, 1600×900, 1920×1080 — all survived (`60-*`). No `bind_mut`, no
layout-loop error, no `SCRIPT ERROR`, no `draw_rect`, no auto-restart. Log stayed at 79 lines
with only the known boot noise (2× `ERROR … create_frustum_points`, 32× plane/vector normalize
warnings, IM/V-Sync warnings).

### Regression sweep → **PASS**

- **Draw**: `C`, footprint drag + move/click → `Box` **8/12/6**, HUD `Created Box`, History
  `Pixel Builder: Create Box` (`35`, `36`).
- **Extrude**: Face + 1 face + `E` → **217/417/218**; `Ctrl+Z` → 209/401/210 (`29`, `30`).
- **Inset**: `I` → **217/417/218**; `Ctrl+Z` → 209/401/210 (`31`).
- **Bevel**: Edge + `Ctrl+A` + `Ctrl+B` → live **824/1614/808**, modal `Adjust Bevel` with
  `Bevel · 1 segments · Profile 0.50 · Offset 0.101 · Clamped to 0.004 at 72 corners` → `Done`
  → History `Pixel Builder: Bevel` → `Ctrl+Z` → 209/401/210 (`32`, `33`, `34`).
- **Paint**: Face + Paint tool (`B`) + slot 0 current + click a Box face → History
  `Pixel Builder: Paint`; `Ctrl+Z` greys it, HUD unchanged (`38`, `42`, `43`).
- **Material**: Face + face picked + click the palette slot 1 button (assigns slot 1 to the
  selection) → History `Pixel Builder: Paint`; `Ctrl+Z` (`44`).
- **Undo**: one step per press for every action above; counts and palette rows follow both ways
  (slot add → 3 rows → undo → 2 rows).
- **Both layouts**: Builder menu → `Modeler layout` → toolbar in the menu-bar row, full-width
  view **1026**, HUD intact (`52`); toggled off → back to MST layout, palette 220 / view 714
  (`53`).
- **Save-reload**: `Ctrl+S`, Scene ▸ `Reload Saved Scene` → `Ruin` 209/401/210 with 2 slots and
  the created `Box` survive; the builder panel reopens (`45`, `47`, `48`).

---

## Covered

- [x] Session started once (`:92`/`8892`), `env.sh` sourced, stopped at the end; no crash, one
      boot line, log unchanged at 79 lines through the whole pass.
- [x] Palette: default 250 (view 684), 220 min (714), 420 max (514), collapse (902), collapse
      after 420 (902), splitter clamps both ways, splitter blocked below the 480 view floor,
      collapse/expand round trip at 1280 (585 ⇄ 397 ⇄ 585), widths/collapse saved in the cfg.
- [x] Ctrl+D: face duplicate + undo, History-dock focus (with and without a face), Vertex hint,
      no `Ruin2` and no `Duplicate Node(s)` in any case.
- [x] HUD: status clears on mode / mesh / undo; line 2 fits at 902/714/585 and wraps (no clip)
      at 480/397; strip no longer pinned at 175 (118/89/151 at 1280).
- [x] Section tabs: one at a time, per-section heights, Vertex hides Extrude, choice persists
      across modes.
- [x] N15 dialog 1050×700 / min 500×300 / 400×200 → 500×300 with the body intact.
- [x] Crash watch at five sizes with the pointer over the rail.
- [x] Regression: draw, extrude, inset, bevel, paint, material, undo, Modeler layout, MST
      layout, save-reload.
- [x] Log scan: no `ERROR`/`SCRIPT ERROR`/`bind_mut`/`draw_rect` beyond the known
      `create_frustum_points` boot pair.

## New issues (summary)

1. **Edge mode keeps an empty Extrude section tab** (ui) — selecting it leaves a 35 px tab-only
   strip; Vertex hides the same tab. Make the section list per mode consistent.
2. **`undo-state` harness readout is empty** (minor, harness) — `action ""`, `undo_count 0`,
   `version 1` for every builder action; History and the HUD show the actions exist. QA had to
   use the History panel + HUD counts.
3. **Expanding the palette at ≤1280 leaves the view at 397 px** (ui/behaviour, from the fix) —
   the auto-collapsed default (585) is lost for the rest of the session after one explicit
   expand; the 32 px `‹` tab has no label, so "Materials" is undiscoverable at 1024/1280.

## UI feedback for FEEDBACK.md (measured)

| Size / state | 3D view | Strip root / content | Palette | Rail root / content |
|---|---|---|---|---|
| 1600×900 default | 684×611 | 85 / 85 | 250 | 86×699 / 72×1076 |
| 1600×900 palette 220 | 714×611 | 85 / 85 (Object) | 220 | 86×699 / 72×1076 |
| 1600×900 Face/Extrude (collapsed palette) | 902 (w) | 99 / 99 | 32 | 86×699 / 72×1364 |
| 1600×900 Face/Ops (collapsed palette) | 902 (w) | 62 / 62 | 32 | 86×699 / 72×1364 |
| 1600×900 palette 420 | 514×582 | 114 | 420 | 86×699 |
| 1600×900 collapsed | **902×638** | 58 | **32** | 86×699 / 72×1364 |
| 1280×800 auto-collapsed | **585×482** | 114 | **32** | 86×599 / 72×1364 |
| 1280×800 forced open | 397 | 143 | 220 | 86×599 / 72×1364 |
| 1366×768 open | **480** | 114 | 220 | 86×567 |
| 1024×600 auto-collapsed | **585** | 99 | **32** | 86×399 / 72×1364 |
| 1920×1080 | 1034 | 58 | 220 | 86×912 |

1. **The 1280/1024 default is now material-less.** With the space returned, the palette sits at
   32 px and only shows `‹`. Give the tab a vertical "Materials" label (or a chevron + glyph) so
   the hidden panel is discoverable; today a new user at 1280 sees no materials at all.
2. **Decide what "expanded at 1280" means.** Expanding gives 220 px and a 397 px view (under the
   documented 480 floor) until the user collapses again. Consider expanding as an overlay/popup
   at ≤1280, or remembering the 585 px view and letting the panel be 220 only in Modeler layout.
3. **Make the section tab list per mode.** Edge + Extrude is a 35 px empty strip; Vertex already
   hides Extrude. Either hide empty sections everywhere or show the fields Edge extrude would use.
4. **The collapse chevron is drawn 10 px from the left edge with no hover affordance** (no
   tooltip, no highlight); a 10 px strip is a small target. Widen the hit area or add a tooltip.
5. **HUD wrap is good, but the wrap threshold is ~480–585 px**; at 1366×768 (view 480) the
   natural single line already wraps into three lines and takes 45 px of a 450 px view. Consider
   dropping `0 edges 0 verts` from line 2 below 520 px.

## Files

- Screenshots (absolute): `/home/clintonnguyen/g/code_examples/rrr/qa/screenshots/r7-builder/`
  (`01`–`66`, `19h`/`33h`/`43h`/`60-*`/`editor-log-final.log`).
- Scripts: `/home/clintonnguyen/g/code_examples/rrr/qa/scripts/r7-builder/`
  (`d.sh`, `m.py`, `strip.py`, `crop.py`, plus the reused `u.py`, `tree.py`, `focus.py`,
  `tabs.py`, `w.py`).
