# r5-manage — Area 3 (Inspect/Chunks/Settings/Storage/Stamp/Gradient/Select/Tint) + Area 8 (deep undo)

Session `r5-t2`, display `:52`, port `8852`, build `/home/clintonnguyen/g/code_examples/rrr-wt/r4base`
(merged r4 tree), scene `res://demo/hex_island.tscn` (295 chunks, baseline digest `3064656926`),
editor 1600×900, editor scale 1.0, llvmpipe. 82 screenshots in `qa/screenshots/r5-manage/`
(`01`–`80`, `16b`). Helpers: `qa/scripts/r5-manage/{env.sh,showui.py,band.py,popick.py,wins.py,state.py,xtype.py}`,
log excerpt `qa/scripts/r5-manage/editor-log-excerpt.txt`.

**Headline.** All six Settings dropdowns that were broken or doubtful in r4/R2 — **Grid Type, Blend Mode,
Floor Slot, Wall Slot, Debug View, Storage Mode** — now write their property through the undo manager and
refresh their label. Grid Type now asks for confirmation before switching a grid that has chunks, Grass
Animation FPS is undoable, the Import dialog's minimum size is accurate, and the ambiguous chunk/tint undo
names are gone. Remaining defects in this area are UI/layout-level (clipped Settings row, clipped Chunks
hint, Grid-tab collision with the toolbar overlay, Export GLB without a path field, cramped Import dialog),
not data loss.

Undo was driven only with `key z --ctrl` / `key z --ctrl --shift` (never `drive.py undo`).
Because the OptionButton popups open as separate X windows, keyboard navigation inside a popup does not
reach it (Down/Enter are swallowed by the editor window); **every dropdown verdict below was driven with a
real mouse click on the popup item**, and `popups` was used to compute the item rect.

> **Harness note (not an addon defect):** `python3 qa/drive.py prop NAME` returns
> `{"error":"node not found"}` unless `--class MarchingSquaresTerrain` (or `--path`) is given, because the
> command resolves the node from `--path`/`--class` only. All values below were read with
> `prop NAME --class MarchingSquaresTerrain`.

---

## Findings

### 1. Settings → Grid clips its last row (Blend Mode) and scrolling hides the page tabs

- **Severity:** ui (a documented field is off-screen at the default 1600×900; discoverability)
- **Steps:** 1) Select the terrain. 2) Settings → **Grid**. 3) Look at the strip bottom / `ui` dump.
- **Expected:** all Grid rows visible, or a scroll area in which the page tabs stay pinned.
- **Actual:** the strip's scroll viewport is `ScrollContainer [410, 678, 903, 175]` (bottom pane 40 px)
  while its content `MstFlow` is **210 px** tall. The **Blend Mode** row — label `[579, 760, 100, 29]`,
  OptionButton `[719, 757, 163, 35]` — starts exactly at the viewport's bottom edge (757) and is fully
  clipped. The page tabs (`Grid | New Chunks | ...`, `[579, 678, 438, 33]`) live **inside** the scroll
  content: after scrolling down to reach Blend Mode the tab row scrolls out of view (measured: tabs at
  y=582 → Grid Type row moves to 585, tabs gone) so there is no way to switch page without scrolling back.
  With the bottom panel enlarged the numbers shift but the row still lands at/below the viewport edge
  (`[719,757,163,35]` vs viewport `…,175`).
- **Evidence:** `qa/screenshots/r5-manage/02-settings-grid.png` (last row cut off),
  `…/23-blendmode-popup.png`, `…/71-grid-page-retry.png`;
  `qa/screenshots/r5-manage/ui` dumps `/tmp/opencode/qa/ui-r5-set1.json` (rects above).
- **Undo:** n/a (layout). Blend Mode itself works once reachable (see Covered).

### 2. `click-text "Grid"` hits the toolbar Grid-overlay button instead of the Settings Grid tab

- **Severity:** ui (mis-click creates an undoable terrain change)
- **Steps:** 1) Terrain selected, Settings open. 2) Click the text "Grid" by name (`click-text "Grid"`,
  9 matches in the visible tree).
- **Expected:** the Settings **Grid** page tab activates.
- **Actual:** the toolbar **Grid** toggle (terrain grid overlay, `[287, 628, 116, 32]`) wins: the button
  becomes pressed, the page stays on whatever page was open (Display in my run), and the History gains a
  `terrain grid overlay` action. Reproduced twice (`70`, `71`); r4 issue 18 reported the same collision.
- **Evidence:** `qa/screenshots/r5-manage/70-settings-which-page.png` (toolbar Grid pressed, page = Display,
  History top = `terrain grid overlay`).
- **Undo:** works — one `Ctrl+Z` reverts `terrain grid overlay`.

### 3. Chunks status line is clipped at the viewport's right edge

- **Severity:** ui (guidance truncated exactly when it matters — the refusal message)
- **Steps:** Chunks tool → hover an empty chunk that cannot be added (e.g. far water at `(700,330)`).
- **Expected:** the whole status sentence is readable or wraps.
- **Actual:** the status is drawn as one line starting at x≈410; the refusal text
  `chunk (-4, -9) cannot be added: new chunks must touch an existing chunk` is cut at the viewport edge
  (`… must touch an e`). No wrap, no tooltip.
- **Evidence:** `qa/screenshots/r5-manage/11-invalid-add-refused.png`;
  cropped status strip in `/tmp/opencode/qa/hv-700-330-status.png` (derived from the run).
- **Undo:** n/a (no edit; the refused add correctly creates **no** undo step — see Covered).

### 4. Chunks add/remove glyphs are barely legible; three different wordings for the same thing

- **Severity:** ui / minor
- **Steps:** Chunks tool; look for the green `+` handles on empty neighbour chunks, then hover one.
- **Expected:** the `+` (add) and `X` (remove) glyphs are visible against water and against the bright
  pixel terrain, and the strip/tooltip use one wording.
- **Actual:** the non-hovered `+` discs are drawn at alpha ≈0.5 with the colour multiplied to ≈0.4 —
  on this scene they read as pale green blobs and I could only locate the **hovered** one (bright disc with
  a white `+`, `16b`); white chunk outlines are indistinguishable from the snow/foam pixels.
  The strip hint says `LMB +: add chunk · LMB chunk: remove · Ctrl+LMB: select`, the **Remove Chunk**
  tooltip still says `LMB a chunk (red X) removes it, LMB a green + adds one`, and the status line says
  `LMB: add chunk (x, y)` / `LMB: remove chunk (x, y)`. Also: `Ctrl+LMB` selection has **no** visible
  "N selected" feedback anywhere in the strip (the chunk dropdown just shows the last-selected chunk).
- **Evidence:** `qa/screenshots/r5-manage/05-chunks-open.png` (handles), `…/15-chunk-hover-remove-x.png`
  (red X + blue selected frame), `…/16b-chunk-empty-green-plus-hover.png` (hovered green +),
  `…/12-chunk-multiselect.png`.
- **Undo:** n/a (glyph legibility). Add/remove/merge all undo correctly (see Covered).

### 5. Tint: "Grid Size" stays enabled while "Grid Aligned" is off

- **Severity:** minor (phantom control)
- **Steps:** Tint tool → leave **Grid Aligned** unchecked → inspect the **Grid Size** slider.
- **Expected:** Grid Size is disabled until Grid Aligned is on (it only means something then).
- **Actual:** `HSlider [921, 722, 80, 35] disabled=false`, tooltip "Size of the grid-aligned stamp in cells",
  while `CheckBox [779, 727, 28, 25]` (Grid Aligned) is unchecked. Dragging it changes nothing.
  Same pattern carried over from the prior round (also seen on the Gradient/Brush strips).
- **Evidence:** `qa/screenshots/r5-manage/59-tint-strip.png`; UI dump `/tmp/opencode/qa/ui-tint.json`.
- **Undo:** n/a (tool setting).

### 6. Settings → Storage "Export GLB" has no path field; the target lives only in the Inspector

- **Severity:** ui (silent 32 MB write to a path the user cannot see in this panel)
- **Steps:** Settings → Storage → **Export** (`[1031, 688, 70, 33]`).
- **Expected:** the target file is visible/editable next to the button (the panel already shows
  Data Directory for the other storages).
- **Actual:** the panel shows `Storage Mode | Import… / Bake / Export` only. The target is
  `MarchingSquaresTerrain.export_path` (default `res://terrain_export.glb`), editable only in the
  Inspector under **Tools → Export Path**. Clicking Export wrote **32,449,384 bytes** to the property's
  value and printed one Output line (`… Export GLB to <path> -> 0`) — no dialog, no confirmation, no
  placement next to the scene or next to the data directory.
- **Evidence:** `qa/screenshots/r5-manage/36-storage-resource-mode.png` (no path row),
  `…/43-storage-export-glb.png`; `ls -la` of `res://qa_tmp/terrain_export.glb` = 32 MB;
  `qa/scripts/r5-manage/editor-log-excerpt.txt` line 83.
- **Undo:** not applicable (file write); Export creates no undo entry (expected).

### 7. Settings → Storage "Save Data" gives no UI feedback

- **Severity:** minor / ui
- **Steps:** Storage Mode `Runtime`/`Baked` → set Data Directory → click **Save**.
- **Expected:** a toast/status ("saved 295 chunk files") or a disabled button while nothing changed.
- **Actual:** only the Output line
  `MarchingSquaresTerrain: saved 295 chunk file(s) to res://qa_tmp/mstdata (Runtime mode): 0 -> 149834 bytes`.
  The button is always enabled even with no pending changes, so a second click is indistinguishable from
  the first.
- **Evidence:** `qa/screenshots/r5-manage/41-storage-save-data.png`; log line 82.
- **Undo:** not applicable (file write; correctly creates no undo entry).

### 8. The bottom panel cannot be shrunk back after expanding (splitter sticks at 136 px)

- **Severity:** ui / minor (loses ~100 px of 3D view for the rest of the session)
- **Steps:** Drag the bottom-pane splitter up to read the Output, then drag it back down to the bottom.
- **Expected:** it returns to the original 40 px.
- **Actual:** it stops at **136 px** (`EditorBottomPane [287, 760, 1026, 136]`); repeated drags from the
  handle (`y=758`) to y=892/853/626 leave it unchanged. Before the first drag the pane was
  `[287, 856, 1026, 40]`.
- **Evidence:** measured in the `ui` dumps of the run (`/tmp/opencode/qa/ui-r5-chk2.json` …
  `ui-r5-chk4.json`); screenshot `…/05-chunks-open.png` shows the reduced viewport.
- **Undo:** n/a.

### 9. Import dialog's minimum size (500×300) is accurate but cramped

- **Severity:** ui / minor
- **Steps:** Storage → **Import…** → request `400×200`.
- **Expected:** either refuse below a usable size or keep the layout readable at the minimum.
- **Actual:** the request clamps to exactly `min [500, 300]` — the r4 mismatch (reported min `[200,70]`,
  actual clamp `498×200`) is **fixed**. At the minimum the layout is poor: the file list is ~2 rows tall,
  "All Recognized" truncates to `All Recogniz…`, the Favorites/Recent panes are empty grey blocks and the
  `File:` row sits under the list.
- **Evidence:** `qa/screenshots/r5-manage/44-import-dialog.png` (1050×700),
  `…/45-import-dialog-narrow.png` (500×300).
- **Undo:** n/a (cancelled with Esc; no import performed).

### 10. LOD & Nature "Add" (Erosion) steals the selection; undoing the add leaves nothing selected

- **Severity:** ui / minor (workflow interruption)
- **Steps:** Settings → LOD & Nature → **Erosion: Add** → then `Ctrl+Z`.
- **Expected:** the terrain stays selected, or the add is undone with the tool still open.
- **Actual:** Add selects the new `Erosion` child (and closes the terrain strip); the undo removes the node
  and leaves **no** selection (`info.selection: []`), so the terrain tool must be re-opened from the Scene
  tree. `Add MarchingSquaresErosion` / undo both work correctly otherwise.
- **Evidence:** `qa/screenshots/r5-manage/51-erosion-added.png`; `nodes MarchingSquaresErosion` → `[]`
  after the undo; `info.selection` count 0.
- **Undo:** works (one step); the selection side effect is the defect.

### 11. Partial-output Inspect clicks (observed, not filed)

- **Severity:** observation
- **Steps:** Inspect tool, four LMB clicks (island cell, water, sky, far water).
- **Actual:** three Output lines for four clicks (`sample (0,1) … height 5.000, floor 1 'Grass', wall 6
  'Cliff'`, `no sample at (-71,-30)`, `no sample at (97,-132)`); the water click produced nothing at all.
  No error, no spam. Most likely a dropped synthetic click; the three that landed behaved exactly as
  documented, so this is **not** filed as an addon defect.
- **Evidence:** log lines 85–87; `04-inspect-output.png`.

---

## Covered (what worked, with the exact proof)

### Inspect
- Cell click → `Marching Squares Terrain: global Vector3 { … } sample (0, 1) chunk IVec2 { 0, 0 }
  local IVec2 { 2, 3 }: height 5.000, floor 1 'Grass', wall 6 'Cliff', grass 255, smooth false`.
  Sky / off-terrain → `no sample at (x, y)`. **No error or warning from either path** (all new log lines
  are the Inspect prints; see Log scan).
- `03-inspect-tool.png`, `04-inspect-output.png`.

### Chunks (295 chunks, baseline digest `3064656926`)
| Action | How | Digest / chunks | Undo name | Ctrl+Z | Ctrl+Shift+Z |
|---|---|---|---|---|---|
| Remove | LMB on a chunk (`860,480`) | 294 / `2843403357` | `remove chunk` | exact digest back | re-applies removed chunk |
| Add | LMB on the emptied slot (green `+` hover) | 295 / `1165459442` | `add chunk` | back to 294 | re-applies |
| Remove (button) | **Remove Chunk** `[953,653,116,33]` | 294 / `1222991727` | `remove chunk` | back | — |
| Invalid add | LMB far water `(470,180)` | unchanged 295 / `3064656926` | **none** (no no-op step — the prior-round "refused change records a no-op undo step" did **not** reproduce) | — | — |
| Merge Mode | `Polyhedron → Cubic` | 295 / `3796190186` | `chunk merge mode` | exact digest back | — |
| Apply to all | **Apply Mode to All Chunks** | 295 / `1171934158` | **`apply chunk merge mode to all`** (distinct name now — r4 issue 27 fixed for chunks) | exact digest back | — |
- Ctrl+LMB selects (blue frame drawn around the selected chunk, red `X` on the hovered chunk); the strip
  never shows a selection count (see finding 4).
- Screenshots `05`–`17`.

### Settings — all six dropdowns write the property **and** record an undo step

| Field (page) | Property | Before → after | Undo action | Ctrl+Z restores | Label refreshes after undo |
|---|---|---|---|---|---|
| **Grid Type** (Grid) | `grid_type` | `3` Hex Rings → `1` Triangle → `3` | `terrain setting grid type` | yes (295 chunks, digest `3064656926`) | **yes** (r4 #13 stale label fixed) |
| **Blend Mode** (Grid) | `blend_mode` | `0` Smooth → `1` Hard Squares → `0` | `terrain setting blend mode` | yes | yes |
| **Floor Slot** (New Chunks) | `default_floor_slot` | `0` → `1` (`2. Meadow`) → `0` | `terrain setting floor slot` | yes | yes |
| **Wall Slot** (New Chunks) | `default_wall_slot` | `5` → `6` (`7. Dark Rock`) → `5` | `terrain setting wall slot` | yes | yes |
| **Debug View** (Display) | `debug_view` | `0` Off → `1` Slope → `0` | `terrain setting debug view` | yes | yes; the viewport visibly switched to slope shading (`34`) |
| **Storage Mode** (Storage) | `storage_mode` | `0` Resource → `1` Baked → `2` Runtime → `0` | `terrain setting storage mode` | yes | yes |
| **Merge Mode** (Grid) | `default_merge_mode` | `1` Polyhedron → `0` Cubic → `1` | `terrain setting merge mode` | yes | yes |
| **Edge Style** (Grid) | `hex_edge_style` | `3` Hex Prism → `2` Slopes → `3` | `terrain setting edge style` | yes | yes |
| **Extra Collision Layer** (Grid) | `impact_stratum` | `1` (None) → `257` (= 1 ∥ bit 8, layer 9) → `1` | `terrain setting extra collision layer` | yes | yes |

Every dropdown on the six Settings pages was driven with a real popup click (9 total); **none is
label-only any more**. `81-extralayer-9.png`, `82-edgestyle-slopes.png`, `83-edgestyle-undo.png`.
- **Grid Type chunk loss now asks first** (`18`, `20`, `72`, `77`): `ConfirmationDialog`
  "Change Grid Type?" 440×233 at `(580,334)`, text "This terrain has 295 chunks on the Hex Rings grid. …
  The Hex Rings chunks are kept and come back when you switch back or press Ctrl+Z.", buttons
  **Cancel** / **Switch to <grid>**. **Cancel** → `grid_type` stays 3, no undo step, no chunks lost.
  **Switch to Triangle** → `grid_type` 1, 0 chunks; **one** Ctrl+Z restores 3 + 295 chunks + exact digest.
- **Dependent rows:** Storage Mode `Baked` → `Bake Grass`, `Bake Collision`, `Data Directory`, `Save Data`
  appear; `Runtime` → `Data Directory` + `Save Data` only; `Resource` → none of them (verified by rects in
  `38`, `39`, `36`). Grid Type `Square` (0 chunks) → `Chunk Size`, `Cell Size`, `Blend Mode`, `Merge Mode`,
  `Extra Collision Layer`; hex-only `Ring Radius` / `Wrap Chunks` / `Edge Style` / `Instanced` disappear
  (verified `72`→`75`).
- **Grass Animation FPS** (r4 issue 12): `0 → 12` records **`terrain grass animation fps`**; Ctrl+Z
  returns the field to `0` and leaves the terrain digest at `3064656926` (it no longer eats an unrelated
  terrain edit).
- **LOD Distance** `48 → 200 → 48` (`terrain setting lod distance`).
- Screenshots `18`–`50`.

### Storage
- **Data Directory** typed through the LineEdit (`res://qa_tmp/mstdata`, Enter applies):
  `data_directory` written, `terrain setting data directory`, undo restores the empty string.
- **Save Data** in `Runtime` mode: 295 `<data_directory>/hex_rings/chunk_X_Y/metadata.res` files
  (489 bytes each, 149,834 bytes total), no undo step.
- **Bake Meshes**: `Bake terrain static meshes`; scene `MeshInstance3D` count 32 → **327** → undo → 32
  (one step).
- **Export GLB**: `res://qa_tmp/terrain_export.glb`, 32,449,384 bytes, Output `… -> 0`.
- **Import…**: dialog 1050×700 at `(275,100)`, `min [500,300]`; `400×200` request clamps to exactly
  500×300; Esc cancels, nothing imported. Demo assets untouched; everything written under
  `res://qa_tmp/` in the **session** copy only.

### Stamp / Gradient / Select / Tint (default modes)
| Tool | Gesture | Digest before → after | Undo name | Ctrl+Z | redo |
|---|---|---|---|---|---|
| Stamp | Mountain, click `(860,470)` | `3064656926 → 3669064663` | `terrain stamp` | exact | exact |
| Gradient | Ctrl+LMB start + Ctrl+RMB end + LMB drag | `3064656926 → 2408883792` | `terrain gradient` | exact | exact |
| Select | drag region → `C` → `V` → `Z` (rotate) → LMB paste | `3064656926 → 1493043505` | `terrain paste` | exact | — |
| Tint | LMB drag (Red, opacity 0.5) | overlay only (digest excludes tint) | `terrain tint paint` | overlay removed | — |
| Tint | RMB drag | — | **`terrain tint erase`** (distinct — r4 issue 27 fixed for tint) | — | — |
- Screenshots `52`–`61`.

### Deep undo / redo (Area 8), all with the real shortcuts
| # | Check | Result |
|---|---|---|
| 1 | one gesture → `Ctrl+Z` → original digest → `Ctrl+Shift+Z` → edited digest (stamp, gradient, paste, brush, level, chunk add/remove) | **all pass** |
| 2 | two gestures → two `Ctrl+Z` in reverse order → two `Ctrl+Shift+Z` | **pass**: `C 731496498 → B 932403843 → A 8724844 → base 3064656926`, then 2 redos back to C |
| 3 | undo after switching tools (brush → **Level**, then `Ctrl+Z`) | **pass** (C undone, `932403843`) |
| 4 | undo after selecting another node (`select --path PixelSky`, then `Ctrl+Z`) | **pass** (B undone, `8724844`) |
| 5 | undo after clicking in the 3D view, then `Ctrl+Z` | **pass** (A undone, `3064656926`; the selection survived: `selection` count 1) |
| 6 | `Ctrl+Z` with a **LineEdit** focused (Scene filter: typed `abc`) | **pass**: text `abc → ab`, terrain digest and history untouched (`has_redo` still true) |
| 7 | new edit after undo clears the redo stack | **pass** (`redo False` immediately after the new `terrain level` action) |
| 8 | save + reload clears the history and does not restore stale state | **pass**: `Ctrl+S`, then Scene → **Reload Saved Scene** → `undo-state` `''`, `undo_count 0`, `has_undo/redo false`, terrain reloaded at the saved digest `649516483`, `Ctrl+Z` does nothing |
| 9 | multi-chunk stroke restoration | **pass**: the brush stroke spanned the island's water/grass border (≥6 chunks at this zoom) and `Ctrl+Z` restored the exact aggregate digest `3064656926`, which hashes every chunk |
| 10 | History dock names | sensible and distinct: `terrain level`, `terrain height draw`, `terrain stamp`, `terrain gradient`, `terrain paste`, `terrain tint paint`, `terrain tint erase`, `add chunk`, `remove chunk`, `chunk merge mode`, `apply chunk merge mode to all`, `terrain setting <label>`, `Bake terrain static meshes`, `Add MarchingSquaresErosion`; the dock count always matched `undo-state.undo_count` (7↔7 in `67`, 4↔4 in `80`) |
| 11 | slider drag = one action | not exercised for terrain properties (the strip uses SpinBox/LineEdit for numbers; the property sliders live in the Inspector). Tool sliders (Stamp Size, Aspect, Tint Opacity) are tool settings and correctly create no history |
| 12 | chunk-size refusal with chunks present (Square grid, 1 chunk, Chunk Size 32 → 16) | **pass**: `chunk_size` stays `32`, **no** undo step is recorded (top action still `add chunk`) — the prior-round "refused change records a no-op undo step" did not reproduce |

Screenshots `62`–`80`.

### Log scan
`qa/sessions/r5-t2/editor.log` = 88 lines: **2 `ERROR`** (`create_frustum_points`,
`servers/rendering/rendering_light_culler.cpp:682`, lines 28 and 62 — both produced at **boot**, before
any interaction), **0 `SCRIPT ERROR`**, 34 warnings of which all but the two known startup ones are the
repeating engine pair `Plane cannot be normalized` / `Vector3 cannot be normalized` (same culler, present
from line 12 on). No `upcast_ref`, no `bind_mut`, no `Message queue out of memory`, no panic.
Excerpt: `qa/scripts/r5-manage/editor-log-excerpt.txt`.

---

## UI feedback (for FEEDBACK.md) — measured rects at 1600×900
1. **Settings strip** — `ScrollContainer [410, 678, 903, 175]`, content `MstFlow [410, 678, 889, 210]`.
   Last Grid row: label `[579, 760, 100, 29]`, control `[719, 757, 163, 35]` → 35 px below the clip.
   Either grow the strip to fit 5 rows (210 px) or keep the **page tabs outside the scroll area** and pin
   them; today scrolling to Blend Mode removes the tabs.
2. **Rename the Settings "Grid" tab** (currently `Button [579, 678, 44, 33] "Grid"`) or the toolbar's
   `Grid` overlay button (`[287, 628, 116, 32]`, tooltip "Grid Overlay"); they are text-identical and the
   toolbar wins `click-text`. Suggested: tab "Grid & Cells", toolbar "Show Grid".
3. **Chunks status line** — one line starting at x≈410 in a 900 px viewport; the refusal message is cut
   (`…: new chunks must touch an e`). Use the same `MstFlow`/wrap as the hint row, or move the refusal to
   a toast.
4. **Chunks glyph contrast** — non-hovered `+` discs are `rgba(0.4·COLOR_ADD, 0.5)` on top of a bright
   pixel-art island; only the hovered disc (`a=0.75`, `+` at `a=1.0`) is findable. Suggest an outline ring
   or a darker disc under all addable slots, and an explicit `N selected` label in the strip for Ctrl+LMB.
5. **Strip wordings** for Chunks: hint `LMB +: add chunk · LMB chunk: remove`, tooltip `LMB a chunk (red X)
   removes it, LMB a green + adds one`, status `LMB: add chunk (x, y)`. Pick one.
6. **Tint "Grid Size"** `HSlider [921, 722, 80, 35]` while `Grid Aligned` `CheckBox [779, 727, 28, 25]` is
   off → disable it (same for Brush/Gradient/Stamp strips).
7. **Storage "Export"** `Button [1031, 688, 70, 33]`: no path field on the page; add an `Export Path` row
   (`TKind::Text`) next to Data Directory so the 32 MB write is predictable, and toast the result.
8. **Storage "Save Data"** `Button [814, 688, 70, 33]`: always enabled; toast the saved-file count and
   disable while nothing is dirty.
9. **Import dialog** at its min `500×300`: file list ≈2 rows, `All Recogniz…` truncation, empty
   Favorites/Recent blocks. Either raise the min to ~700×420 or hide those panes under 380 px height.
10. **Bottom panel splitter** sticks at 136 px after any expansion (`EditorBottomPane [287, 760, 1026, 136]`);
    let it collapse back to 40 px.
11. **LOD & Nature → Add** moves the selection to the new child and closes the terrain tool; after undoing
    the add nothing is selected. Prefer `edit_node` only on the explicit click, and restore the previous
    selection on undo.

---

## r4 ISSUES.md status

**Fixed in this build (re-driven on `demo/hex_island.tscn`):**
- **#5** Floor Slot / Storage Mode label-only → **fixed** for both (properties write, undo entries created,
  dependent Storage rows appear).
- **#6** Debug View label-only → **fixed** (`debug_view` 0→1→0, viewport switches to slope shading).
- **#12** Grass Animation FPS not undoable → **fixed** (`terrain grass animation fps`, Ctrl+Z restores 0
  without touching an older terrain action).
- **#15** Grid Type silently deletes every chunk → **fixed** by the "Change Grid Type?" confirmation
  (Cancel keeps everything; switch + one Ctrl+Z restores 295 chunks and the exact digest).
- **#13** (stale labels after undo) → **fixed for this area**: Grid Type, Blend Mode, Floor Slot, Wall Slot,
  Debug View and Storage Mode labels all refresh on undo.
- **#27** ambiguous undo names → **fixed for chunks and tint**: `apply chunk merge mode to all` and
  `terrain tint erase` are now distinct (Smooth Terraced/Sloped not re-tested here).
- **#40** file dialog minimum inaccurate → **fixed**: `min [500,300]` and a 400×200 request clamps to
  exactly 500×300.
- **#18** Settings tab row jumping between pages (Settings pages) → **mostly fixed**: the tab row stayed at
  y=582 across Grid/New Chunks/Grass/Display/LOD & Nature/Storage; the remaining problem is the new
  clipping/scroll behaviour (finding 1) and the Grid-tab collision (finding 2).
- Prior-round items now working: **Blend Mode** and **Wall Slot** write (property + undo), the refused
  **chunk size** change no longer records a no-op undo step, and an **invalid chunk add** creates no undo
  step.

**Still reproduces:**
- **#16** Side panels fixed at 250 px with no splitter — the Paint panel in `76-chunksize-refused.png` is
  still 250 px wide with a long scroll (the Textures panel shows 2 of 16 slots).
- **#18** (residual) `click-text "Grid"` hits the toolbar overlay instead of the Settings tab (finding 2).
- Prior-round item: **Tint Grid Size enabled while Grid Aligned is off** (finding 5).

**Not re-tested in this pass** (out of scope or timeboxed): #1/#2/#4/#8 (pixelizer/builder), #3 (scene
close/reopen error flood — I reloaded, not closed), #7 (strip height at 1024/1280), #9 (shortcut keys),
#10 (out-of-range sliders), #11/#14 (pixelizer undo), #17, #19–#26, #28–#39, #41–#43.

---

## New issues (not in r4 ISSUES.md)
| # | Sev | Issue |
|---|---|---|
| N1 | ui | Settings → Grid clips the Blend Mode row (35 px below the scroll viewport) and the page tabs live inside the scroll content, so reaching Blend Mode hides the tabs (finding 1) |
| N2 | ui | Chunks status line clipped at the viewport edge, including the "cannot be added" refusal (finding 3) |
| N3 | ui | Chunks add `+` glyphs are effectively invisible unless hovered; three different wordings for add/remove; no `N selected` feedback for Ctrl+LMB (finding 4) |
| N4 | ui | Storage "Export GLB" has no path field; a 32 MB write goes to an Inspector-only property (finding 6) |
| N5 | minor/ui | Storage "Save Data" has no UI feedback and is always enabled (finding 7) |
| N6 | ui/minor | The bottom-panel splitter cannot be shrunk back below 136 px after expanding (finding 8) |
| N7 | ui | Import dialog at its accurate minimum (500×300) is cramped: 2-row list, truncated "All Recognized", dead Favorites/Recent panes (finding 9) |
| N8 | ui/minor | LOD & Nature "Add" (Erosion) steals the selection; undoing the add leaves nothing selected and closes the terrain tool (finding 10) |

---

## Not tested / gaps
- Stamp: only the **Mountain** stamp in **Add** mode, one click (plus `Ctrl+Z`/`Ctrl+Shift+Z`); stamp
  drag (size+rotation pull), the Mode dropdown (Max/Min/Replace/Lift/Carve/Level/Broad), Size/Aspect/
  Height/Rotation/Snap/Contrast/Limits and the Shift/Ctrl/Alt+wheel modifiers were **not** exercised.
- Gradient: default Round brush, default strength/falloff; Gradient **modes** and the Strength slider were
  not exercised (the Ctrl+LMB start / Ctrl+RMB end + LMB-drag path was).
- Select: drag → `C` → `V` → `Z` → paste only. **`M` mirror** and paste-relative-height were not driven.
- Tint: default **Red** at opacity 0.5, LMB paint + RMB erase only; colour/opacity changes, Clear and
  Grid Aligned were not driven.
- Chunks: Ctrl+LMB multi-select was performed but the *effect* (multi-chunk merge / remove) was not
  distinguished from single selection; **Vertex Handles** was not enabled or dragged; the 16-item Floor
  Slot popup was opened at y=514…887, i.e. it uses almost the whole screen height (minor UI note).
- Settings: the Grass page's `Ledge Cutoff` slider was not driven; `Noise Hmap` resource picker was not
  used; `Data Directory` was typed, not picked with the folder button.
- Undo: slider-drag merging for Inspector property sliders; Smooth Terraced/Sloped naming; close/reopen
  (only "Reload Saved Scene" was tested).
- Window sizes: everything was measured at 1600×900 only; 1024/1280/1366 layouts were not revisited.
- The History dock's `get_history_count` matched the dock in every controlled comparison; one transition
  (tint undo → brush tool switch) showed one fewer entry than the arithmetic predicts. Could not reproduce
  it under control, so it is **not** filed as a defect.

## Undo matrix (condensed)
| Action | Undo name | Digest restored | Redo |
|---|---|---|---|
| Chunk add | `add chunk` | yes | yes |
| Chunk remove (LMB / button) | `remove chunk` | yes | yes |
| Chunk merge mode | `chunk merge mode` | yes | — |
| Apply mode to all | `apply chunk merge mode to all` | yes | — |
| Invalid add / refused chunk size | *(none recorded)* | no change | — |
| Grid Type (with confirm) | `terrain setting grid type` | yes (295 chunks) | — |
| Blend Mode / Floor Slot / Wall Slot / Debug View / Storage Mode | `terrain setting <label>` | yes | — |
| Merge Mode / Edge Style / Extra Collision Layer | `terrain setting merge mode` / `… edge style` / `… extra collision layer` | yes | — |
| Grass Animation FPS | `terrain grass animation fps` | field restored | — |
| Data Directory | `terrain setting data directory` | yes | — |
| LOD Distance | `terrain setting lod distance` | yes | — |
| Erosion Add | `Add MarchingSquaresErosion` | node removed | — |
| Bake Meshes | `Bake terrain static meshes` | 327 → 32 meshes | — |
| Stamp / Gradient / Paste | `terrain stamp` / `terrain gradient` / `terrain paste` | exact | stamp yes |
| Tint paint / erase | `terrain tint paint` / `terrain tint erase` | overlay | — |
| Brush height draw | `terrain height draw` | exact | yes |
| Level | `terrain level` | exact | — |
