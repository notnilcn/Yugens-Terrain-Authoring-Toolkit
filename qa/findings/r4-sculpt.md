# Sculpting QA r4 — Brush / Level / Smooth / Bridge / toolbar / undo

Session `r4-sculpt`, display `:31`, port `8811`, Godot 4.7.2 (llvmpipe), editor window
1600x900 at 0,0. Scene `res://demo/hex_island.tscn` (Hex Rings, 295 chunks), terrain
re-selected after opening. All work was done in the disposable session copy
(`/tmp/opencode/qa/sessions/r4-sculpt/project`); no addon code or repository scene was edited.
The scene **was saved** (twice) inside the session copy only, to exercise undo-after-Save and
close/reopen; nothing was saved into the repository.

Editor log `/tmp/opencode/qa/sessions/r4-sculpt/editor.log`: the known startup noise (V-Sync,
debug-adapter/GDScript LSP ports, missing `.uid` re-creation) was present; no other log lines
appeared until the scene-close test below, which produced 196 `ERROR` lines (all the same
use-after-free panic). Log growth was measured command-by-command (13 -> 91 -> 355 -> 601 lines).

## Closing / reopening a scene with the terrain tool active triggers a godot-rust use-after-free panic

- Severity: error
- Steps: 1) Open `demo/hex_island.tscn`, select `HexIsland/MarchingSquaresTerrain`, leave the
  Brush tool active (Brush, Drag or Raise / Lower). 2) Right-click the scene tab,
  **Close Tab** (Ctrl+W did not close it). 3) Read the Output panel / `editor.log`; then reopen
  the scene with `open-scene`. 4) Repeat the close once more.
- Expected: closing/reopening the scene frees the terrain node quietly; the tool reattaches to
  the new instance when the scene is opened again.
- Actual: Closing the tab emits a burst of `Node3D::upcast_ref: access to instance with ID
  ... after it has been freed` panics from `godot-core 0.5.5` (first burst 26 errors / 78 log
  lines; second close of the reopened scene 88 errors / 264 lines; the following reopen added
  another 82 errors). The editor stays alive and the reopened scene works, but the Output is
  flooded and the error badge reaches 26+; the first close had 26 errors where the startup
  baseline was 0. Both bursts were reproduced from a clean saved scene, twice, same session.
- Evidence: `qa/screenshots/r4-sculpt/50-close-scene-error-spam.png` (Output panel full of
  panics), `qa/screenshots/r4-sculpt/48-brush-drag-square-selection.png` (Output already shows
  the panic + error badge 26); `editor.log` line 14 and line 92:
  `ERROR: [panic .../godot-core-0.5.5/src/classes/class_runtime.rs:379]  Node3D::upcast_ref: access to instance with ID 1652941727204 after it has been freed`
  then the same for ID `12919832130081`.
- Undo: not applicable (scene lifecycle / use-after-free, not an undo entry).

## Number keys `1`–`9` do not switch tools; the 3D view keeps the keystroke

- Severity: warning
- Steps: 1) Select the terrain (tool strip shows `Level` after the Level tests). 2) Click an
  empty part of the 3D view to focus it. 3) Press `2`, then `1`; read `ToolTitle`.
- Expected: Level (`2`) / Brush (`1`) become the active tool, as documented in
  `docs/12 Cheat Sheet.md` and in every tool tooltip (`Brush Tool [1]`, `Bridge Tool [4]`, ...).
- Actual: `ToolTitle` stays `Level` for both keys. The digits are consumed by the 3D view
  (view orientation), so the shortcut advertised by the tooltips/docs does nothing for tools.
- Evidence: `qa/screenshots/r4-sculpt/37-key2-after.png` (viewport after `2`, strip still
  `Level`); immediate `ui --filter ToolTitle` returned `Level` after `key 2` and after `key 1`.
- Undo: not applicable.

## Shift/Ctrl-wheel change Size and Strength but the strip labels/sliders stay stale

- Severity: warning
- Steps: 1) Brush -> Raise / Lower; note the strip shows Size `15.0`, Strength `4.000`.
  2) Over the 3D view: `Shift`+wheel up x3, `Ctrl`+wheel down x2, `Alt`+wheel up x3.
  3) Compare the strip text with the actual values.
- Expected: The numeric labels and slider handles update with the values (the docs advertise
  Shift+wheel as the fast Size control and Ctrl+wheel for Strength).
- Actual: The internal values change (UI dump `Size: 19.0`, `Strength: 3.2565`, `Rotation: 45`)
  but the strip still displays `Size 15.0` and `Strength 4.000`, and the two slider handles do
  not move. Rotation is a spin slider and does refresh to `45`. The labels only refresh when the
  strip is rebuilt (switching tool/mode; after the rebuild the same dump reads Size `18.9` /
  Strength `3.303`).
- Evidence: `qa/screenshots/r4-sculpt/28-rl-wheel-values.png` (strip still 15.0 / 4.000 after
  wheel), `qa/screenshots/r4-sculpt/29-rl-wheel-footprint.png`,
  `qa/screenshots/r4-sculpt/ui/28-after-wheel.json` (values 19.0 / 3.2565), and
  `qa/screenshots/r4-sculpt/42-brush-square-footprint.png` (refreshed 18.9 / 3.303 after a
  rebuild).
- Undo: not applicable (tool settings, not terrain).

## `Esc` clears the brush selection but the status HUD keeps showing "N selected"

- Severity: minor
- Steps: 1) Brush -> Drag; drag a selection (status `175 selected`). 2) Press `Esc` once.
  3) Compare the overlay status line immediately after `Esc` with the following mouse drag.
- Expected: the status line drops the selection count and returns to `LMB: select cells`
  (docs: RMB/Esc clears the selection).
- Actual: After `Esc` the overlay still reads `175 selected | LMB-drag: move selection` (two
  separate captures, 2.5 s after the key). The selection itself is gone: the next drag starts a
  new selection (digest unchanged, status becomes `134 selected`), and a RMB also clears. The
  HUD only refreshes after the next mouse interaction, so the user is told a stale selection is
  active (and might expect the next drag to move it).
- Evidence: `qa/screenshots/r4-sculpt/11-status-after-selection.png` (175 selected),
  `qa/screenshots/r4-sculpt/12-status-after-esc.png` (still 175 selected, no mouse move),
  `qa/screenshots/r4-sculpt/14-status-after-esc-then-drag.png` (new selection, 134 selected);
  terrain digest `3064656926` before and after the post-Esc drag.
- Undo: not applicable (selection is editor state, not an undo step); the earlier diagnostic
  observation that the selection "survives" Esc did not reproduce — only the HUD text is stale.

## Status/hint line is clipped at the right edge and the Level hint advertises a missing Strength control

- Severity: ui
- Steps: 1) Brush -> Drag, paint a selection. 2) Read the status line (second line under the
  viewport hint). 3) Select Level and read its hint line and strip controls.
- Expected: the full hint/status fits or wraps; hints only mention controls that exist.
- Actual: At 1600x900 the status is cut at the viewport edge (`... RMB/Esc: c...`, and after a
  selection `... RMB/Esc` with no `clear` suffix). The Level hint says
  `Ctrl+wheel: strength` but the Level strip contains only Height/Size/Aspect/Rotation/
  Falloff/Peak/Grid Aligned/Grid Size/Quick Paint — there is no Strength value to watch.
  The Raise / Lower hint lists `Shift+wheel: size` but omits the working `Ctrl+wheel: strength`.
- Evidence: `qa/screenshots/r4-sculpt/06-brush-selection-painted.png` and
  `qa/screenshots/r4-sculpt/10-esc-cleared.png` (clipped right edge),
  `qa/screenshots/r4-sculpt/30-level-strip-hex.png` (Level hint vs. strip),
  `qa/screenshots/r4-sculpt/22-brush-raiselower-strip.png` (Raise/Lower hint).
- Undo: not applicable.

## Grid Aligned looks enabled but silently refuses to toggle while Falloff is on

- Severity: minor
- Steps: 1) Hex Rings, Brush -> Raise / Lower (or Drag), Falloff **on**. 2) Click the
  **Grid Aligned** checkbox at its visible position. 3) Turn Falloff off and click Grid Aligned
  again.
- Expected: per `docs/03 Sculpting.md` ("If **Grid Aligned** is grayed out, switch back to
  Brush, turn **Falloff** off"), the control should be disabled/grayed while unavailable, or
  it should toggle.
- Actual: With Falloff on, the checkbox reports `disabled=false` and looks normal, but clicking
  it leaves `toggled=false` with no feedback. After turning Falloff off, the same click toggles
  it on (`toggled=true`). Reproduced at 1600x900 on Hex Rings.
- Evidence: `qa/screenshots/r4-sculpt/43-grid-aligned-on.png` (click did nothing, falloff still
  checked), `qa/screenshots/r4-sculpt/44-grid-aligned-falloff-off.png` (Falloff off, Grid
  Aligned on); UI dumps show `disabled=false, toggled=false` then `toggled=true`.
- Undo: not applicable.

## Covered

- [x] Selected the terrain; toolbar present; **Collapse** toggles the compact icon-only
      toolbar (buttons 36x32, tooltips still work: `Bridge Tool [4]` shown); expanding restores
      the labels. `qa/screenshots/r4-sculpt/38-toolbar-collapsed.png`, `39-collapsed-tooltip-bridge.png`.
- [x] Tool buttons Brush/Level/Smooth/Bridge each stay pressed and rebuild the strip
      (`ToolTitle` = Brush / Level / Smooth / Bridge); strips captured:
      `05-brush-strip.png`, `02-level-strip.png`, `03-smooth-strip.png`, `04-bridge-strip.png`.
- [x] Brush **Drag**: LMB-drag paints a white selection (`173 selected`); digest unchanged
      (`3064656926`), no undo step. Starting a drag on the selection performs a height draw:
      digest `3064656926` -> `1171660968`, undo action `terrain height draw`.
- [x] Brush Drag modifiers: Shift adds (134 -> 293 selected), Ctrl removes (293 -> 245),
      Alt drags one cell (digest -> `439507659`, status `1 selected`), RMB clears, Esc clears
      the cells (with the stale-HUD finding above). Evidence `15`, `16`, `17`, `18`.
- [x] Brush **Raise / Lower**: switch via Brush Mode popup (`popups` pos 998,518 / item 1);
      strip shows Ceiling 16.0 / Height 0.0 / Floor -8.0 / Strength 4.0 / Peak / Falloff /
      Grid Aligned / Grid Size / Quick Paint. Hold LMB raises (digest -> `2093333320`, action
      `terrain raise/lower`), hold RMB lowers (digest -> `33596374`), LMB+RMB flattens toward
      Height (digest -> `884650251`). All undo with Ctrl+Z.
- [x] Brush picks in Raise / Lower: `Ctrl+LMB` picks Ceiling (2.863 from the hovered cell),
      `Ctrl+MMB` picks Height (4.9735), `Ctrl+RMB` picks Floor (-3.8705); terrain digest
      unchanged by the picks.
- [x] Wheel: Shift+wheel size 15.0 -> 19.0, Ctrl+wheel strength 4.000 -> 3.2565, Alt+wheel
      rotation 0 -> 45. Values change and the footprint/terrain reacts; strip labels stale
      (finding).
- [x] Brush Type: Round and Square selected via the popup; Square drag painted a square-cell
      selection. (`42`, `48`, `49`). Hexagon/Hexagon30/Triangle/Triangle180 only enumerated.
- [x] Brush settings visible and reachable: Size, Aspect, Rotation, Flatten, Falloff, Peak,
      Grid Aligned, Grid Size, Quick Paint. Falloff toggles (it is a real checkbox); Grid
      Aligned toggles after Falloff off. Quick Paint stayed `None` (not driven).
- [x] Level: `Ctrl+LMB` picks the hovered height (4.9735); a drag flattens the hex terrain
      (digest `884650251` -> `617194426`, action `terrain level`); Ctrl+Z restores. Level works
      on Hex Rings.
- [x] Smooth: Terraced drag changes the terrain (`884650251` -> `1381840711`), Sloped drag
      changes it differently (`-> 2679497365`), both undo; Strength changed via the slider
      (0.150 -> 0.3005) and a drag still edits. Terraced vs Sloped captures `32`, `33`, `34`.
- [x] Bridge: two-point drag builds a ramp (digest -> `279877088`, action `terrain bridge`),
      Ctrl+Z restores. Size 18.9 used as width; Ease Value default -1.0. Strip `35`, result `36`.
- [x] Grid overlay: toggle adds visible cell lines (`40` before / `41` after) and records a
      separate `terrain grid overlay` undo entry (terrain digest unchanged, as expected for a
      display toggle).
- [x] Tooltips on collapsed toolbar icons show full multi-line tooltip windows.
- [x] Raise / Lower strip at 1600x900: the strip grows to 175 px (526,448 787x175) and every
      control including Brush Type is visible; the prior "title/Brush Type clipped" symptom did
      **not** reproduce in this build (`22-brush-raiselower-strip.png`, `ui/22-raiselower.json`).
- [x] Undo/redo matrix (all with real Ctrl+Z / Ctrl+Shift+Z; digests from `terrain`):
      - one Brush move stroke: `3064656926` -> `1171660968`; Ctrl+Z -> `3064656926`;
        Ctrl+Shift+Z -> `1171660968`. **works**
      - one Brush Raise/Lower stroke, flatten, Level, Smooth (both modes), Bridge: each digest
        restored by one Ctrl+Z. **works**
      - two strokes: baseline `3064656926` -> A `1171660968` -> B `1496637932`;
        Ctrl+Z -> `1171660968`, Ctrl+Z -> `3064656926`, in reverse order. **works**
      - undo after switching tools (Level stroke, click Smooth, Ctrl+Z): restored. **works**
      - undo after clicking the 3D view sky (focus change): restored. **works**
      - undo after selecting another node (PixelSky): restored. **works**
      - undo after `Ctrl+S`: stroke `4166240857`, save, Ctrl+Z -> `884650251`. **works**
      - close/reopen scene: undo history is empty (`undo_count 0`), Ctrl+Z leaves the saved
        digest `1327250373` unchanged. **works** (history correctly cleared)
      - undo -> redo -> new stroke -> one Ctrl+Z: landed exactly one stroke back
        (`3522641338` -> `1327250373`), not the pre-first-stroke baseline, and not nothing.
        The r2 "one Ctrl+Z reverts two strokes" sequence did **not** reproduce. **works**
      - Alt one-cell drag: digest `439507659`, Ctrl+Z restored. **works**
- [x] Editor log scan: clean during all sculpting/undo steps; only the scene-close panic
      described above produced `ERROR` lines.

## Matrix (tool x action x undo)

| Tool | Action driven | Digest changed | One Ctrl+Z restores | Redo restores |
|---|---|---|---|---|
| Brush Drag | paint selection | no (selection only) | n/a | n/a |
| Brush Drag | move selection (raise) | yes | yes | yes |
| Brush Drag | Shift add / Ctrl remove | no | n/a | n/a |
| Brush Drag | Alt one-cell move | yes | yes | not re-run |
| Brush Drag | RMB / Esc clear | no | n/a | n/a (HUD stale after Esc) |
| Brush Raise/Lower | LMB raise | yes | yes | not re-run |
| Brush Raise/Lower | RMB lower | yes | yes | not re-run |
| Brush Raise/Lower | LMB+RMB flatten | yes | yes | not re-run |
| Brush Raise/Lower | Ctrl pick Ceiling/Height/Floor | no | n/a | n/a |
| Level | drag flatten | yes | yes | not re-run |
| Level | Ctrl+LMB pick Height | no | n/a | n/a |
| Smooth Terraced | drag | yes | yes | not re-run |
| Smooth Sloped | drag | yes | yes | not re-run |
| Bridge | A->B drag | yes | yes | not re-run |
| Grid overlay | toggle | no (display only) | undo entry exists | n/a |

## Not tested

- Brush Type Hexagon / Hexagon30 / Triangle / Triangle180: popup items only enumerated; no
  footprint/selection was compared. The green outline footprint itself was not clearly visible
  in the captures at 1600x900, so cursor-footprint offset/stuck, edge clamping, Aspect, Peak,
  Flatten on/off and Grid Size were not conclusively verified.
- Quick Paint: options were not changed (left at `None`).
- Bridge Ease Value curves (`1`, `2`, `0.5`, `-1`, `-5`) and explicit width comparison
  (size was changed only indirectly through the wheel test).
- Smooth Strength was changed and edits still landed, but a quantitative before/after compare
  (strong vs weak) was not made.
- Number-key shortcuts under other focus contexts (Scene tree, toolbar) were not re-tested
  beyond the viewport-focus failure above.
- 1280x800 / 1024x600 strip layouts and compact toolbar at low height were not part of this
  pass; only 1600x900 was driven.
- Undo of multi-chunk strokes, undo of Fill/Clear/Erosion, and LinkEdit-focused Ctrl+Z belong
  to other areas and were not re-run here.
