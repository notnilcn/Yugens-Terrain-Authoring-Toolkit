# Area 8 — Deep undo/redo QA

Session `undodeep`, display `:35`, port `8815`, project copy
`/tmp/opencode/qa/sessions/undodeep/project`, editor 1600×900 (Godot 4.7.2, llvmpipe).
Scenes: `res://demo/hex_island.tscn` (Hex Rings, 295 chunks) and
`res://tutorial/02_island.tscn` (Square, 9 chunks). Terrain was re-selected after every
scene change. Undo/redo was always driven with the real shortcuts (`key z --ctrl`,
`key z --ctrl --shift`) unless a row says "menu"; `drive.py undo/redo` was used only to
isolate the LineEdit-focus failure. Nothing under `addons/` or the repo scenes was
modified — saves happened only in the session copy.

Log: the only addon lines are expected warnings (`new chunks must touch an existing
chunk`, `chunk_size is fixed once chunks exist`, `Gradient needs a start ...`) plus
`ERROR`s induced by this pass calling `get_chunk_grass_count` with the wrong argument
shape (harness typo, not an addon defect). No addon `SCRIPT ERROR`, no crash.

## Undo matrix

Digests are the `terrain` digest (heights/floors/walls/grass). `—` means the digest does
not cover the data (texture/tint/props), so the proof is the history version plus
`get_scatter_count`/screenshots.

| Action | One step? | Ctrl+Z restores? | Ctrl+Shift+Z? | Action name | Notes |
|---|---|---|---|---|---|
| Level drag | yes | yes (2572476293→3064656926) | yes | `terrain level` | baseline sanity |
| Paint ground stroke (×15) | yes each | yes, exact digest/version | yes | `terrain vertex paint` | 15 strokes → v22; 15 undos → v7; 15 redos → v22 |
| Paint Walls (side view, vertical cliff) | yes | yes | yes | `terrain wall paint` | digest 2373799866↔1958813665 |
| Quick Paint (Full Void + Raise/Lower drag) | yes | yes | yes | `terrain brush with quick paint` | quick paint on/off is part of the stroke |
| Grass paint (Foliage, Grass) | yes | yes | yes | `terrain foliage paint grass` | digest 3331022133↔3809822909 |
| Grass erase (RMB) | yes | yes | yes | `terrain foliage erase grass` | same pair reversed |
| Props paint (Tree) | yes | yes | yes | `terrain foliage paint props` | 164→216 |
| Props erase (RMB) | yes | yes | yes | `terrain foliage erase props` | 216→184 |
| Erase All + RMB erase | yes | yes (both Tree and Flowers restored) | yes | `terrain foliage erase props` | one step removes every prop type |
| Fill Terrain (Tree, hex_island) | yes | yes (181→184) | yes | `terrain foliage fill` | digest unchanged (props not in digest) |
| Clear (Tree, hex_island) | yes | yes (0→181) | not repeated | `terrain foliage clear` | one step empties all trees |
| Stamp Mesa | yes | yes | yes | `terrain stamp` | click stamp |
| Stamp Valley | yes | yes | not repeated | `terrain stamp` | click stamp |
| Stamp Hill | yes | yes | not repeated | `terrain stamp` | click stamp |
| Gradient (Ctrl+LMB start, Ctrl+RMB end, LMB drag) | yes | yes (555010447→1409542798) | yes | `terrain gradient` | Shift/Ctrl+wheel before the stroke did not add steps |
| Tint paint (LMB) | yes | yes (history only) | yes | `terrain tint paint` | digest does not cover tint; screenshot evidence |
| Tint erase (RMB) | yes | yes | not repeated | `terrain tint paint` | erase uses the *paint* name (misleading) |
| Chunk add (green +) | yes | yes (10→9) | yes | `add chunk` | digest restored |
| Chunk remove (click chunk) | yes | yes (294→295) | yes | `remove chunk` | digest restored |
| Slot Pattern (Forest) | yes | data yes, **label stale** | yes | `terrain texture 1 pattern` | label still says Forest until the panel rebuilds |
| Slot Scale drag | yes | yes | yes | `terrain texture 1 magnify` | name says "magnify", UI says Scale |
| Slot Color picker (#ff3333) | yes | data yes, **swatch stale** | not repeated | `terrain texture 1 tint` | swatch stays red after undo |
| Slot Grass Sprite → Clear | yes | yes | yes | `terrain texture 1 grass_sprite` | picker menu → Clear |
| Slot name edit (commit with Enter) | yes | yes | yes | `terrain texture 1 display_name` | redo verified; see LineEdit finding |
| Texture Preset → MST Classic | yes | resource yes, **labels stale** | not repeated | `terrain texture preset` | `tex_assign` restored, labels still MST Classic |
| Settings Chunk Size 32→24 | creates step | Ctrl+Z removes the step, **no data change** | no change | `terrain setting chunk size` | field snaps back to 32; no-op step |
| Settings Blend Mode Smooth→Hard Squares | **no step** | n/a | n/a | — | label changes only; `blend_mode` stays 0 |
| Settings Floor Slot 1→2. Meadow | **no step** | n/a | n/a | — | label changes only; `default_floor_slot` stays 0 |
| Settings Grass Animation FPS 0→12 | **no step** | **no** | n/a | — | value stays 12; Ctrl+Z undoes the previous terrain action |
| PixelRenderer Pixel Size drag (integer) | **two steps** | first undo → 4357, second → 2 | yes (→4357) | `Set Renderer Pixel Size` ×2 | one drag = two entries |
| Scene-menu Undo / Redo (menu clicks) | n/a | yes | yes | — | restores digest |
| 15 strokes → 15 Ctrl+Z → 15 Ctrl+Shift+Z | yes | exact digest/version | exact | `terrain vertex paint` | ordering perfect |
| Level → Paint → Paint (interleaved) | yes | reverse order, 3 steps | not repeated | mixed | digest follows each undo exactly |
| Switch scene tab away and back, then Ctrl+Z | yes | yes | yes | `terrain brush with quick paint` | per-scene history preserved (hex v26, 02_island v2) |
| Select PixelWater, Ctrl+Z, reselect terrain, Ctrl+Shift+Z | yes | yes | yes | `terrain gradient` | history is scene-wide |
| Save (Ctrl+S) → Ctrl+Z → Ctrl+Shift+Z | yes | yes | yes | — | saving does not clear history |
| Save + close scene + reopen | n/a | history empty, Ctrl+Z does nothing | n/a | — | saved digest 1409542798 reopened; no stale edits |
| Ctrl+Z with the Texture Name LineEdit focused | n/a | text edited, terrain untouched | n/a | `terrain texture 1 display_name` (a **new** entry) | field keeps focus after Enter; its text undo commits a new action |

## Texture-slot panel does not refresh after undo/redo (stale labels)

- Severity: warning
- Steps: 1) Paint tool, Texture slot 1. 2) Change **Pattern** Grass→Forest. 3) `Ctrl+Z`.
  4) Compare the Pattern dropdown with `terrain texture 1 pattern` history state.
  5) Repeat for **Scale**, **Color**, **Grass Sprite**, and **Texture Preset → MST Classic**.
- Expected: the dropdowns, swatch, sprite picker, slot names and Material dropdown show the
  restored values (the data is restored correctly).
- Actual: the resource properties revert (verified by rebuilding the panel: Pattern reads
  Grass again after switching tools; `tex_assign` returns to the embedded preset), but the
  visible controls keep the edited values until a strip/page rebuild. After the preset
  undo the Material dropdown still lists `1. Texture 1`, the preset dropdown still says
  `MST Classic`, and the slot names stay `Texture 1…`; the Color swatch stays red after
  the tint action is undone. This is the same class as the previously reported stale
  Material labels, now reproduced for Pattern, Color, Sprite and Scale.
- Evidence: qa/screenshots/undodeep/07-pattern-undo.png,
  qa/screenshots/undodeep/10-color-undo.png,
  qa/screenshots/undodeep/30-preset-undo-stale.png,
  qa/screenshots/undodeep/ui/05-paint-strip.json,
  qa/screenshots/undodeep/ui/30-preset-undo-stale.json,
  qa/screenshots/undodeep/ui/07-pattern-after-rebuild.json (Pattern reads `Grass`
  after a strip rebuild, proving the data had reverted).
- Undo: data restored; UI labels stale.

## Ctrl+Z in the texture-name LineEdit does not reach the terrain, and its own text undo records a new action

- Severity: warning
- Steps: 1) Paint tool, click the slot **Texture Name** field, type a name (`undotest`),
  press Enter. 2) Press `Ctrl+Z` with the field still focused. 3) `undo-state`.
  4) Click the 3D viewport and press `Ctrl+Z` again. 5) `undo-state`.
  6) Repeat with uncommitted text (`abc`) and `Ctrl+Z`.
- Expected: `Ctrl+Z` while the field is focused edits the text field (or at least does not
  touch terrain history), and clicking the 3D view leaves the field so `Ctrl+Z` undoes the
  rename. (Sculpt's README protocol expects undo after clicking the 3D view to work.)
- Actual: with the field focused, `Ctrl+Z` does not undo the terrain action; the field's
  text undo changes the text back, but because the field applies on every change the
  restored text is committed through the undo manager as a **new**
  `terrain texture 1 display_name` action (history version 24→25, redo stack lost).
  Clicking the 3D viewport does **not** release the LineEdit focus, so `Ctrl+Z` remains
  stuck in the text field. Clicking a toolbar button (e.g. Brush) or `raw unfocus` frees
  it and the rename can then be undone normally.
- Evidence: qa/screenshots/undodeep/32-rename.png,
  qa/screenshots/undodeep/33-lineedit-ctrlz.png,
  qa/screenshots/undodeep/34-lineedit-raw-ctrlz.png; `undo-state` went 23→24→25 while
  the field stayed focused, then 25→24 after clicking Brush.
- Undo: rename is undoable from outside the field; text-field undo itself adds entries.

## Misleading History names

- Severity: minor
- Steps: 1) Look at the History dock after a Paint slot Scale drag, a Tint RMB erase, and
  any sculpt stroke with Quick Paint enabled (Level/Smooth/Bridge).
- Expected: an entry that names what happened (Scale, tint erase, level with quick paint).
- Actual: Scale is `terrain texture 1 magnify`; RMB tint erase is `terrain tint paint`;
  with Quick Paint set, Level/Smooth/Bridge strokes are all named
  `terrain brush with quick paint`, so the tool that changed the terrain is not in the
  name. The History dock order itself is correct (newest last, "The Beginning" first).
- Evidence: qa/screenshots/undodeep/53-history-dock.png,
  qa/screenshots/undodeep/ui/59-gradient-strip.json,
  qa/screenshots/undodeep/ui/41-stamp-strip.json.
- Undo: works; only the labels are wrong.

## Grass Animation FPS is still not undoable (re-confirmed)

- Severity: warning
- Steps: 1) Settings → Grass. 2) Animation FPS `0` → `12` (type + Enter). 3) `undo-state`.
  4) `Ctrl+Z`.
- Expected: `terrain grass animation fps` action restores 0 (code has the branch).
- Actual: no action is created; `Ctrl+Z` undoes the previous terrain action instead
  (history 8→7) and the field keeps 12.
- Evidence: qa/screenshots/undodeep/25-grass-fps.png; `undo-state` before/after identical.
- Undo: broken (setting not undoable).

## Settings enum / slot dropdowns still apply nothing (re-confirmed, ISSUES #1)

- Severity: error
- Steps: 1) Settings → Grid, change **Blend Mode** Smooth→Hard Squares. 2) Settings →
  New Chunks, change **Floor Slot** 1. Grass→2. Meadow. 3) Read `undo-state` and
  `prop blend_mode` / `prop default_floor_slot`.
- Expected: property write through the undo manager + rebuild.
- Actual: only the OptionButton text changes (`Hard Squares`, `2. Meadow`);
  `blend_mode` stays 0, `default_floor_slot` stays 0, no undo action. A later strip
  rebuild snaps the labels back.
- Evidence: qa/screenshots/undodeep/23b-blendmode-hardsquares.png,
  qa/screenshots/undodeep/28-floorslot-enum.png,
  qa/screenshots/undodeep/ui/23-blendmode.json (label Hard Squares),
  qa/screenshots/undodeep/ui/28-floorslot.json (label 2. Meadow); `prop` values 0.
- Undo: not applicable — no action exists.

## Chunk Size creates a no-op undo step (re-confirmed)

- Severity: minor
- Steps: 1) 02_island (Square), Settings → Grid. 2) Chunk Size `32` → `24`, Enter.
  3) `undo-state`, field value, `Ctrl+Z`.
- Expected: disabled field or a visible explanation; if nothing changed, no undo entry.
- Actual: field snaps back to `32`, Output only has the `chunk_size is fixed once chunks
  exist` warning, but the history gains `terrain setting chunk size`. `Ctrl+Z` removes
  that step and changes nothing.
- Evidence: qa/screenshots/undodeep/49-chunksize-noop.png,
  qa/screenshots/undodeep/ui/49-chunksize.json; editor.log warning line.
- Undo: step exists but restores nothing.

## One integer spin-slider drag is two undo steps (re-confirmed)

- Severity: warning
- Steps: 1) Select `PixelRenderer` on 02_island. 2) Drag **Pixel Size** ~55 px over 1.2 s.
  3) `undo-state` (two `Set Renderer Pixel Size`), `Ctrl+Z` twice, `Ctrl+Shift+Z`.
- Expected: one drag = one action.
- Actual: value 2→39497 creates **two** actions; first undo goes to 4357 (mid-drag),
  second to 2; one redo goes back to 4357. Float spin sliders drag as one step, so this is
  specific to the integer path.
- Evidence: qa/screenshots/undodeep/51-pixelsize-drag.png;
  qa/screenshots/undodeep/ui/51-pixelsize-drag.json,
  qa/screenshots/undodeep/ui/51b-pixelsize-undo1.json,
  qa/screenshots/undodeep/ui/51c-pixelsize-undo2.json (values 39497 / 4357 / 2).
- Undo: broken granularity.

## "Vertex Handles" does not expose a draggable vertex handle; edge clicks add/remove chunks

- Severity: warning (UI affordance / risk of accidental chunk removal)
- Steps: 1) Chunk Manager, tick **Vertex Handles**. 2) On hex_island, drag the green
  handle/line at the selected chunk edge. 3) Repeat on 02_island, dragging the red
  circle handle at the chunk border. 4) Drag the green vertical line endpoint.
- Expected: a vertex handle can be dragged to change the chunk-corner height; the History
  entry names a vertex edit.
- Actual: no drag produced a vertex edit (`undo-state` unchanged). The drag at the hex
  chunk border removed the selected chunk (`remove chunk`, 295→294), and a drag at a red
  circle in 02_island added a chunk (`add chunk`, 9→10). Both are undoable, but the
  handles look like vertex grips and the hint only says "LMB +: add chunk · LMB on a
  chunk: remove it", so it is easy to remove a chunk by accident while trying to grab a
  vertex. Whether the true vertex grips exist for this grid was not proven.
- Evidence: qa/screenshots/undodeep/45-vertex-handles.png,
  qa/screenshots/undodeep/46-vertex-drag.png,
  qa/screenshots/undodeep/48-02-vertex-handles.png.
- Undo: chunk add/remove restore correctly (`add chunk`, `remove chunk`).

## Ctrl+Z is swallowed while an OptionButton popup is open

- Severity: minor (expected menu behavior, noted for the protocol)
- Steps: Change a Settings enum, then press `Ctrl+Z` without pressing `Esc` first.
- Expected/actual: the popup consumes the shortcut; `Esc` then `Ctrl+Z` works. No action
  is lost. Not counted as a defect, but QA scripts must close popups first.
- Evidence: qa/screenshots/undodeep/22-settings-grid.png (popup-open state).
- Undo: works once the popup is closed.

## Covered

- [x] Baseline Level stroke: one step, exact digest restore, redo exact.
- [x] Paint ground strokes ×15: 15 actions, 15 Ctrl+Z return to the exact pre-series
      digest/version, 15 Ctrl+Shift+Z return to the exact post-series digest/version.
- [x] Paint slot edits: Pattern, Scale, Color (hex picker), Grass Sprite (Clear), rename;
      each is one action and the resource reverts; labels stale (finding).
- [x] Texture Preset → MST Classic: one action; resource/`tex_assign` reverts; labels
      stale (finding).
- [x] Paint Walls on a true vertical cliff face (side view): `terrain wall paint`, one
      step, digest restore/redo.
- [x] Quick Paint: Full Void + Raise/Lower stroke → `terrain brush with quick paint`, one
      step, digest restore/redo; `None`/`Base Grass Masked`/`Selected Material` listed.
- [x] Foliage: Grass paint and RMB erase (digest pair); Tree props paint (164→216), RMB
      erase (216→184), Erase All RMB (removes Tree+Flowers in one step), each one action.
- [x] Fill Terrain on hex_island (Tree 184→181) — one step, undo/redo restore the count.
- [x] Clear on hex_island (Tree 181→0) — one step, undo restores 181.
- [x] Stamps Mesa, Valley, Hill: each one `terrain stamp`, digest changed, undo restores.
- [x] Chunk add/remove: one step each, chunk count and digest restored, redo re-applied.
- [x] Gradient: Shift+wheel size and Ctrl+wheel strength then a start/end/drag stroke →
      one `terrain gradient` step, exact digest restore/redo. A plain LMB drag without
      endpoints is a no-op and prints the documented warning.
- [x] Tint: LMB paint and RMB erase are each one `terrain tint paint` step; undo/redo
      proven by history + screenshots (digest does not cover tint).
- [x] Cross-tool ordering: Level → Paint → Paint, three Ctrl+Z undo in reverse order with
      the expected digest at every step.
- [x] Undo after switching scene tabs (02_island → hex_island → 02_island): per-scene
      histories preserved and the right action undone.
- [x] Undo after selecting another node (PixelWater) then reselecting the terrain:
      gradient undone and redone.
- [x] Save (Ctrl+S) then Ctrl+Z / Ctrl+Shift+Z: history stays live.
- [x] Save + close + reopen 02_island: history empty (`undo_count 0`, Ctrl+Z does
      nothing), saved digest reopened, no stale edit resurrected.
- [x] Scene menu → Undo / Redo clicked with the mouse (not the shortcut): both work.
- [x] History dock: entries present in correct order with "The Beginning"; names noted
      (some misleading, finding).
- [x] Redo stack cleared by a new edit (Quick Paint, Paint, Tint, Gradient actions all
      cleared a pending redo).
- [x] Ctrl+Z with a LineEdit focused edits the text, but the follow-up behaviour is wrong
      (finding) and clicking the 3D view does not release the focus.
- [x] Settings known defects re-confirmed: enum/slot dropdowns no-op, Grass FPS not
      undoable, Chunk Size no-op step; Pixel Size integer drag = two steps.
- [x] Log scanned before/after: no addon `SCRIPT ERROR`; only the expected warnings plus
      two `get_chunk_grass_count` argument errors caused by this QA pass.

## Gaps still open

- **Chunk vertex-handle drag** was not conclusively exercised: the visible handles at the
  chunk edge trigger add/remove chunk, and no drag was accepted as a vertex edit. Needs a
  targeted pass that identifies the real grip (possibly only on square grids with a
  specific zoom/camera) before calling it broken.
- **Hex grid vs square grid** stamp/gradient behaviour was only spot-checked; the deep
  undo matrix ran mostly on 02_island for the square-only settings (Chunk Size) and on
  hex_island for the terrain tools.
- **Grass Fill Terrain** (the Foliage palette's Fill with Grass selected) was not run;
  Fill/Clear were run with Tree. Grass painting/erasing was run.
- **Undo of Import/Bake/Save Data** was out of scope here (covered by manage.md for Bake;
  Import round-trip still untested).
- **Multi-chunk stroke undo** was proven by the depth-15 series on hex_island (strokes at
  many positions) but not by hash-comparing every affected chunk; the digest covers all
  chunks, and exact digest equality is strong evidence.
- The texture-slot label staleness needs a code-level fix and a re-test of **all** slot
  controls (Pattern, Color, Sprite, Scale, Name) after one undo, not only Material names.
