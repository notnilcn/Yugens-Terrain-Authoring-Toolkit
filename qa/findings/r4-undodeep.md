# R4 — Deep undo/redo matrix (MarchingSquaresTerrain)

Session `r4-undodeep`, display `:38`, port `8818`, project copy
`/tmp/opencode/qa/sessions/r4-undodeep/project`, Godot 4.7.2 + llvmpipe, 1600×900.
Scene `res://demo/hex_island.tscn` (Hex Rings, 294→295 chunks), reopened several times.
Every undo/redo below was driven with the real shortcuts (`key z --ctrl`,
`key z --ctrl --shift`); `drive.py undo`/`redo` were never used in this pass.
Session log: `/tmp/opencode/qa/sessions/r4-undodeep/editor.log`.
Addon and repo `project.godot` untouched; saves only in the session copy.

Digest = `terrain` digest (heights/floors/walls/grass). Paint/tint/props actions are proven by
history version + `get_grass_count`/`get_scatter_count` and screenshots where the digest does
not cover that data.

## Action × undo matrix

| # | Action | Digest / data pair | Undo name (exact) | One step? | Ctrl+Z exact? | Ctrl+Shift+Z? | Context | Verdict |
|---|---|---|---|---|---|---|---|---|
| 1 | Brush → Drag, paint selection | digest unchanged | — | n/a | n/a | n/a | 3D view | works (selection is not an undo step) |
| 2 | Brush → Drag, move selection (height draw) | 3064656926 → 2553258098 | `terrain height draw` | yes | yes | yes | 3D view | works |
| 3 | Brush → Drag, 2nd stroke | 2553258098 → 3383117721 | `terrain height draw` | yes | yes ×2 in reverse order → 2553258098 → 3064656926 | yes ×2 → 3383117721 | 3D view | works; stroke touched **19 chunks**, all restored (per-chunk hashes) |
| 4 | Brush Raise/Lower → LMB raise | 3383117721 → 3270352743 | `terrain raise/lower` | yes | yes | yes | 3D view | works |
| 5 | Brush Raise/Lower → RMB lower | 3270352743 → 539162292 | `terrain raise/lower` | yes | yes | yes | 3D view | works |
| 6 | Brush Raise/Lower → LMB+RMB flatten | 539162292 → 4159945644 | `terrain raise/lower` | yes | yes | yes | 3D view | works |
| 7 | Level drag | 4159945644 → 2043180336 | `terrain level` | yes | yes | yes | after tool switch | works |
| 8 | Smooth, Terraced | 2043180336 → 2856533356 | `terrain smooth` | yes | yes | yes | 3D view | works |
| 9 | Smooth, Sloped | 2856533356 → 2410141702 | `terrain smooth` | yes | yes | yes | 3D view | works (same name as Terraced) |
| 10 | Bridge drag | 2410141702 → 1180396382 | `terrain bridge` | yes | yes | yes | 3D view | works |
| 11 | Paint vertex stroke | 1180396382 → 3985264679 | `terrain vertex paint` | yes | yes | yes | Paint panel open | works |
| 12 | Paint Wall (Paint Walls on) | 3985264679 → 828037534 | `terrain wall paint` | yes | yes | yes | Paint panel open | works |
| 13 | Foliage grass erase (RMB) | grass 4463 → 4159 | `terrain foliage erase grass` | yes | yes (4463) | yes (4159) | Foliage panel | works |
| 14 | Foliage grass scatter | grass 4159 → 4463 | `terrain foliage paint grass` | yes | yes (4159) | yes (4463) | Foliage panel | works |
| 15 | Foliage Clear (Tree selected) | props 2247 → 2074 | `terrain foliage clear` | yes | yes (2247) | yes (2074) | Foliage panel | works |
| 16 | Foliage Fill Terrain (Tree) | props 2074 → 2254 | `terrain foliage fill` | yes | yes (2074) | yes (2254) | Foliage panel | works |
| 17 | Stamp click (Mountain) | 828037534 → 415255086 | `terrain stamp` | yes | yes | yes | 3D view | works |
| 18 | Stamp drag (size/rotation pull) | 415255086 → 2001077784 | `terrain stamp` | yes | yes | yes | 3D view | works |
| 19 | Gradient (Ctrl+LMB start, Ctrl+RMB end, drag) | 2001077784 → 3872050656 | `terrain gradient` | yes | yes | yes | 3D view | works |
| 20 | Select paste | 3872050656 → 870770980 | `terrain paste` | yes | yes | yes | Select tool | works |
| 21 | Select paste, rotated (`Z`) | 870770980 → 3614475897 | `terrain paste` | yes | yes | yes | Select tool | works; different digest proves rotation |
| 22 | Select paste, mirrored (`M`) | 3614475897 → 3056827004 | `terrain paste` | yes | yes | yes | Select tool | works; different digest proves mirror |
| 23 | Tint paint (LMB) | digest n/a (tint data) | `terrain tint paint` | yes (version 22→23) | yes | yes | Tint strip | works (screenshot) |
| 24 | Tint erase (RMB) | digest n/a | `terrain tint paint` | yes (23→24) | yes | not repeated | Tint strip | works; misleading name |
| 25 | Chunk remove (Remove Chunk) | 295 → 294 chunks, digest restored | `remove chunk` | yes | yes (295) | yes (294) | Chunks strip | works |
| 26 | Chunk add (green + at chunk corner) | 293 → 294 chunks, 55253223 → 3508008992 | `add chunk` | yes | yes (293) | yes (294) | Chunks strip | works |
| 27 | Chunk merge mode (selected chunk) | 278382899 → 642044331 | `chunk merge mode` | yes | yes | yes | Chunks strip | works |
| 28 | Apply Mode to All Chunks | 642044331 → 1329477683 | `chunk merge mode` | yes | yes | yes | Chunks strip | works; **same name as #27** |
| 29 | Settings Cell Size 2.0 → 2.5125 | digest unchanged (property) | `terrain setting cell size` | yes | yes (2.0) | yes (2.5125) | Settings/Grid | works |
| 30 | Settings Grid Type Hex Rings → Square | 294 → 0 chunks, digest 0 | `terrain setting grid type` | yes | yes (294 chunks / 1329477683) | yes (0 chunks) | Settings/Grid | works, but wipes all chunks with no confirmation; strip label stale after undo |
| 31 | Settings Noise Amp 0.0 → 0.5 | digest unchanged | `terrain setting noise amp` | yes | yes | yes | Settings/New Chunks | works |
| 32 | Settings Floor Slot 1. Grass → 2. Meadow | **no data change** (`default_floor_slot` stays 0) | — | **no** | n/a | n/a | Settings/New Chunks | **broken** (label-only) |
| 33 | Settings Debug View Off → Slope | **no data change** (`debug_view` stays 0) | — | **no** | n/a | n/a | Settings/Display | **broken** (label-only; this pass did not toggle the grid) |
| 34 | Settings Grass Animation FPS 0 → 12 | **no action**, field stays 12 | — | **no** | Ctrl+Z undid the previous terrain action instead | n/a | Settings/Grass | **broken** |
| 35 | Settings Grass on/off toggle | property true→false | `terrain setting grass` | yes | yes (true) | not repeated | Settings/Grass | works |
| 36 | Settings Display Pixel Size 1 → 2 | property | `terrain setting pixel size` | yes | yes (1) | yes (2) | Settings/Display | works |
| 37 | Settings LOD Distance 48 → 64 | property | `terrain setting lod distance` | yes | yes (48) | yes (64) | Settings/LOD | works |
| 38 | Storage → Bake | digest unchanged | `Bake terrain static meshes` | yes | yes | yes | Settings/Storage | works |
| 39 | Storage → Import | dialog only | — | n/a | n/a | n/a | Settings/Storage | **not tested** (dialog opened and cancelled) |
| 40 | Texture Preset → MST Classic | `tex_assign` scene resource → embedded copy | `terrain texture preset` | yes | yes (resource path restored) | not repeated | Paint panel | works; labels stale |
| 41 | Texture slot rename (Enter) | slot display name | `terrain texture 1 display_name` | yes | yes | not repeated | Paint panel | works; labels stale after undo |
| 42 | PixelRenderer Pixel Size integer spin drag | value 3 → 27313 | `Set Renderer Pixel Size` ×2 | **no — two steps** | undo1 → 148, undo2 → 3 | yes ×2 → 27313 | PixelRenderer selected | **broken granularity** |

Context / protocol checks:

| Check | Result |
|---|---|
| Two gestures → two Ctrl+Z in reverse order | works: digests 3064656926 / 2553258098 / 3383117721 exactly at each step; two redos restore |
| Stroke touching multiple chunks | works: 19 chunks differed after two strokes; after two undos every per-chunk hash equals the baseline snapshot and the digest is exact |
| Undo after switching tools | works (stamp → brush, level → 3D view etc.; all undos landed on the intended action) |
| Undo after selecting another node (PixelSky) | works: level undone 2657929337 → 1329477683 with PixelSky selected; redo after reselecting terrain restored 2657929337 |
| Undo after clicking in the 3D view | works: bake undone after a sky click in the viewport |
| Ctrl+Z with Texture Name LineEdit focused (uncommitted text) | correct: typed `abc`, Ctrl+Z removed the text and left terrain history at version 31 |
| LineEdit after Enter | focus is released in this build; Ctrl+Z afterwards undoes the rename action (prior “field keeps focus” bug did **not** reproduce) |
| Ctrl+Z with an EditorSpinSlider value editor focused | correct: typed `5`, Ctrl+Z reverted the field to 0.0, terrain history unchanged |
| Clicking the 3D view after editing a LineEdit | releases the text focus; the next Ctrl+Z reaches the editor (prior finding did **not** reproduce) |
| Ctrl+Z while an OptionButton popup is open | not retested; every undo below was issued after the popup was closed |
| Save (Ctrl+S) then Ctrl+Z / Ctrl+Shift+Z | works: history stays live, level undone and redone |
| Close scene + reopen | history empty (`version 1`, `undo_count 0`), Ctrl+Z leaves the reopened digest 1226327139 untouched; **but scene close/reopen bursts the freed-instance ERROR (finding 1)** |
| Redo stack cleared by a new edit | works: after undo `has_redo:true`; a new Level stroke produced version 32 with `has_redo:false` |
| History dock | shows `terrain stamp`, `terrain level`, `The Beginning`; distinct and human-readable for terrain tools; Scene + Global both ticked |
| Slider drags on tool settings (Paint Size, Foliage Density) | no undo entries at all (tool settings, expected) — no per-value history spam |
| Destructive action audit | Fill/Clear (foliage) undoable; texture preset undoable (labels stale); grass toggle undoable; Grid Type undoable but silently wipes chunks; Bake undoable; **Grass Animation FPS not recorded**; Floor Slot/Debug View no-ops; Import not tested; wild flowers/erosion not tested |

## Scene close/reopen bursts `Node3D::upcast_ref … after it has been freed` errors

- Severity: error
- Steps: 1) Open `demo/hex_island.tscn`, select `MarchingSquaresTerrain`, use the terrain tools
  (paint/stamp/level, then `Ctrl+S`). 2) Scene menu → **Close Scene**. 3) Wait ~5 s.
  4) `grep -c "upcast_ref: access to instance" editor.log`.
- Expected: closing the scene releases the plugin cleanly; no errors.
- Actual: the log gains a burst of identical Rust panics:
  `ERROR: [panic … class_runtime.rs:379]  Node3D::upcast_ref: access to instance with ID
  1653126276585 after it has been freed` (73 → 202 on the first close, 202 → 287 after the
  reopen/close of the next scene). The errors stop when a terrain is selected again, but the
  closed scene’s terrain instance is referenced after free. Re-opening the scene recovers and the
  undo history is correctly empty.
- Evidence: `qa/screenshots/r4-undodeep/58-history-dock-pixel.png` (Output panel with the first
  ERROR), `61-history-dock.png`; editor.log 287 × `upcast_ref … after it has been freed`.
- Undo: not applicable (scene lifecycle; history is cleared as expected).

## Settings enum / slot dropdowns still apply nothing (re-confirmed)

- Severity: error
- Steps: 1) Settings → New Chunks → **Floor Slot** `1. Grass` → `2. Meadow`. 2) `undo-state`,
  `prop default_floor_slot`. 3) Settings → Display → **Debug View** `Off` → `Slope`.
  4) `prop debug_view`, `undo-state`.
- Expected: the property is written through the undo manager and an action is created.
- Actual: the dropdown label changes (`2. Meadow`, `Slope`) but `default_floor_slot` stays `0`,
  `debug_view` stays `0`, no action is created, and the next `undo-state` still names the previous
  action (`terrain setting noise amp` / `terrain setting pixel size`). This pass did not reproduce
  the r2c “Debug View toggles the grid overlay” side effect — here it is label-only.
- Evidence: `qa/screenshots/r4-undodeep/49-floorslot.png` (label 2. Meadow),
  `qa/screenshots/r4-undodeep/ui/49-floorslot.json`, `51-debugview-slope.png`.
- Undo: broken (nothing recorded).

## Grass Animation FPS is not undoable; Ctrl+Z then reverts an unrelated edit

- Severity: warning
- Steps: 1) Settings → Grass. 2) Animation FPS `0` → `12` + Enter. 3) `undo-state`; `Ctrl+Z`.
- Expected: `terrain grass animation fps` restores 0.
- Actual: no action is created (version stays 28) and the field keeps 12; the next `Ctrl+Z`
  undoes the previous terrain action (`terrain setting noise amp`, version 28 → 27) instead.
- Evidence: `qa/screenshots/r4-undodeep/50-grass-toggle.png` context (Grass page);
  `undo-state` before/after identical.
- Undo: broken (setting not recorded; unrelated undo is standard but surprising).

## PixelRenderer integer spin-slider drag is two undo steps

- Severity: warning
- Steps: 1) Reopen `hex_island`, select `PixelRenderer`, Renderer section. 2) Drag the **Pixel
  Size** spin slider 58 px over 1.2 s. 3) `undo-state`; `Ctrl+Z`; `Ctrl+Z`; `Ctrl+Shift+Z` twice.
- Expected: one drag = one action.
- Actual: the drag creates **two** `Set Renderer Pixel Size` actions (version 2 and 3). Values:
  3 → 27313; first undo → 148; second undo → 3; two redos → 27313. Float spin sliders were not
  re-tested this pass.
- Evidence: `qa/screenshots/r4-undodeep/57-pixelsize-drag.png`; ui dumps in
  `qa/screenshots/r4-undodeep/ui/58-history.json` context; history version 3 with `n=2`.
- Undo: broken granularity.

## Stale UI after undo (Grid Type label, texture slot name/preset)

- Severity: warning
- Steps: 1) Settings → Grid, change `Hex Rings` → `Square` (chunks 0), then `Ctrl+Z`.
  2) Read the Settings strip and the Inspector. 3) Paint tool, rename Texture 1 to `undotest`,
   Enter, `Ctrl+Z` (with focus outside the field).
- Expected: the strip and slot panel show the restored values.
- Actual: after the Grid Type undo the strip shows **Grid Type: Square** and Chunk Size 32 while
  the Inspector and `prop grid_type` say `Hex Rings` (3) and the terrain is back (294 chunks).
  After undoing the rename the Panel/Texture Name still show `undotest` / `1. undotest` until a
  strip rebuild. (Same class as the earlier “stale Material labels” finding.)
- Evidence: `qa/screenshots/r4-undodeep/47-gridtype-undo.png`, `48-settings-page.png`
  (strip Square vs Inspector Hex Rings), `53-lineedit-ctrlz.png`.
- Undo: data restored; UI labels stale.

## Duplicate / misleading History names

- Severity: minor
- Steps: 1) Chunks tool: change Merge Mode on the selected chunk, then **Apply Mode to All
  Chunks**. 2) Tint tool: paint, then RMB erase. 3) Read the History dock / Output.
- Expected: entries that distinguish single-chunk merge from apply-to-all, and tint paint from
  tint erase.
- Actual: both chunk operations are `chunk merge mode`; RMB tint erase is `terrain tint paint`
  (same as paint); Terraced and Sloped smooth strokes are both `terrain smooth` (acceptable but
  ambiguous). Terrain tool names otherwise read well (`terrain height draw`, `terrain level`,
  `terrain bridge`, `terrain stamp`, `terrain paste`, `terrain foliage fill`, `Bake terrain
  static meshes`).
- Evidence: `qa/screenshots/r4-undodeep/43-merge-mode.png`, `44-apply-all.png`,
  `39-tint.png`, `40-tint-undo.png`, `63-history-dock-actions.png`,
  `64-history-dock-actions-crop.png`.
- Undo: works; labels only.

## Chunks tool corner handles are hard to hit; invalid clicks only warn in the log

- Severity: minor (UI affordance)
- Steps: 1) Chunks tool on hex_island. 2) Click the small green markers at chunk corners / click
  water just outside the island.
- Expected: a visible “+” target (or hover feedback) for adding a chunk; invalid positions give
  on-screen feedback.
- Actual: two clicks 14–20 px away from the green markers produced only
  `WARNING: Marching Squares Terrain: new chunks must touch an existing chunk.` in the log
  (no on-screen message). Clicking an existing chunk removes it (`remove chunk`), and clicking
  the same point again adds an empty chunk (`add chunk`) — both undoable.
- Evidence: `qa/screenshots/r4-undodeep/42-chunks-view.png`, `41-chunk-remove.png`,
  `65-chunk-add-redo.png`; editor.log warnings at `plugin.rs:1244`.
- Undo: works.

## Settings page tab row still moves between pages; one click landed in the viewport

- Severity: ui
- Steps: 1) Settings → Grid (tabs at y≈273). 2) Undo Grid Type (strip shrinks, tabs move to
  y≈395). 3) Click the old New Chunks position (789, 289).
- Expected: a stable tab row, or a strip that scrolls instead of resizing.
- Actual: the click missed the moved tabs and landed in the 3D view; the tab row also sat at
  y≈378–448–518–553 on different pages this pass. Re-confirms the earlier layout finding.
- Evidence: `qa/screenshots/r4-undodeep/48-settings-page.png`, `45-settings-cellsize.png`.
- Undo: not applicable.

## Fill Terrain / Clear silently ignore Grass-only selection (hint only)

- Severity: ui (discoverability)
- Steps: 1) Foliage tool, select only **Grass**. 2) Click **Clear**, then **Fill Terrain**.
- Expected: either clear/fill grass or a disabled button.
- Actual: nothing happens; the only feedback is the transient status line
  “Fill Terrain / Clear act on props: select one or more props (not only Grass) in the palette.”
  Selecting Tree makes both work (`terrain foliage clear` / `terrain foliage fill`).
- Evidence: `qa/screenshots/r4-undodeep/25b-foliage-clear-click.png`,
  `27-foliage-fill.png`.
- Undo: n/a (no action when Grass-only).

## Covered

- [x] Brush: Drag selection + height draw (one step, exact digest, 2-stroke/2-undo/2-redo),
      Raise/Lower LMB raise, RMB lower, LMB+RMB flatten.
- [x] Level drag: `terrain level`, exact digest, redo exact; undo after tool switch.
- [x] Smooth Terraced and Sloped: one step each, exact digest/redo.
- [x] Bridge drag: `terrain bridge`, exact digest/redo.
- [x] Paint vertex stroke: `terrain vertex paint`, exact digest/redo.
- [x] Paint Walls stroke on a cliff: `terrain wall paint`, exact digest/redo.
- [x] Foliage grass scatter and RMB erase: `terrain foliage paint/erase grass`, grass counts
      4159↔4463, redo exact.
- [x] Foliage Fill Terrain and Clear with Tree: `terrain foliage fill` (2074→2254) and
      `terrain foliage clear` (2247→2074), one step each, undo/redo counts exact.
- [x] Stamp click and stamp drag: one `terrain stamp` each, exact digest/redo.
- [x] Gradient with two picked endpoints: `terrain gradient`, exact digest/redo.
- [x] Select paste, rotated paste (`Z`), mirrored paste (`M`): `terrain paste`, three different
      digests, each one step with exact undo/redo.
- [x] Tint paint and RMB erase: one `terrain tint paint` step each (history/screenshot).
- [x] Chunks: remove (295→294), add (293→294), merge mode, apply-to-all — each one step,
      digest/chunk-count restore and redo.
- [x] Settings: Cell Size, Grid Type (destructive but undoable), Noise Amp, Grass toggle,
      Display Pixel Size, LOD Distance — each one action with property restore/redo.
- [x] Storage Bake: `Bake terrain static meshes`, undo/redo.
- [x] Texture Preset MST Classic: one `terrain texture preset` step; `tex_assign` resource path
      restored by undo.
- [x] Texture slot rename: one `terrain texture 1 display_name` step (labels stale after undo).
- [x] Two gestures → two Ctrl+Z in reverse order → original; two Ctrl+Shift+Z → edited
      (Brush Drag, exact digests).
- [x] Multi-chunk stroke: 19 chunks changed; undo restored every per-chunk hash and the digest.
- [x] Redo stack cleared by a new edit after undo.
- [x] Save then undo/redo; close scene then reopen (history empty, Ctrl+Z inert, saved digest
      preserved) — with the freed-instance error documented above.
- [x] Undo after selecting `PixelSky` and after clicking the 3D view; undo after tool switches.
- [x] Keyboard focus: LineEdit with uncommitted text and an EditorSpinSlider value editor both
      consume Ctrl+Z locally; terrain history untouched. Clicking the 3D view releases focus.
- [x] History dock screenshot with `terrain stamp` / `terrain level` / `The Beginning`; names
      checked against every action recorded above.
- [x] Log scan: only known startup noise, two chunk-touch warnings (caused by this pass), two
      `get_scatter_count` call errors (QA harness misuse: called without its required argument),
      and the freed-instance panic burst.

## Not tested / gaps

- **Storage → Import** round-trip was not run (the native Open dialog was opened and cancelled);
  data-directory mode, Import of an exported `.res`, and Export GLB were out of time.
- **Chunk Size no-op step** and **Blend Mode / Merge Mode / Edge Style** enum writes were not
  re-tested this pass (prior undodeep pass covers them).
- **Erosion** and **Wild Flowers** nodes were not added/run; their undo could not be audited.
- **Foliage Erase All** checkbox and **wild-flower scatter** were not re-run this pass (prior
  passes cover Tree/Grass erase here).
- **PixelBuilderMesh** actions were not exercised, so “builder actions appearing in Global
  history” could not be checked (no builder action existed in this session).
- **Float spin-slider drag** (one step on the float path) was not compared side-by-side with the
  integer path; only the integer Pixel Size drag was retested.
- **Popup-open Ctrl+Z** (documented minor: popup consumes the shortcut) was not retested; all
  undo steps here were issued with popups closed.
- **Grass Fill Terrain** (Foliage Fill with Grass only) was not run after the hint revealed the
  props-only rule.
- Save/close tests wrote the session copy of `hex_island.tscn` only (bake node + edits); the repo
  scene and addon were never modified.
