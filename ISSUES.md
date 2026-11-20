# MarchingSquaresTerrain — Issues

Testing was done in a real Godot 4.7.2 editor on Linux (Xvfb virtual display, Mesa llvmpipe,
editor scale 1.0; primary window 1600×900, plus 1920×1080, 1366×768, 1280×800 and 1024×600),
driving the actual UI with synthetic mouse and keyboard input. `--headless` (dummy renderer) cannot render or screenshot the editor, so the
virtual-display setup was the closest headless-capable equivalent that still produces images.
Screenshots and per-area reproduction notes live in [`qa/`](qa/) (`qa/screenshots/<area>/`,
`qa/findings/<area>.md`); the orchestrator runbook is
[`qa/METHODOLOGY.md`](qa/METHODOLOGY.md).

Legend: **Critical** = crash/blocker; **Error** = a documented control does nothing or data is
lost; **Warning** = a feature works but is misleading, unreachable, or not undoable;
**Minor** = edge case/polish; **UI** = layout, wording, or discoverability.

## Summary

| # | Severity | Area | Issue |
|---|---|---|---|
| 0 | **Critical** | Editor layout | Switching to Level, Smooth or Tint at 1024×600 crashes the editor with a container layout loop (`Container::_sort_children` / "Message queue out of memory"); reproduced 5 times |
| 1 | Error | Terrain Settings | Every enum/slot dropdown (Grid Type, Blend Mode, Floor/Wall Slot, Debug View, Storage Mode, …) changes only its label — the property is never written, nothing rebuilds, no undo step |
| 2 | Warning | Terrain Settings | Save Data / Data Directory unreachable because Storage Mode cannot be changed (consequence of #1) |
| 3 | Warning | Terrain Settings | Grass Animation FPS is not undoable; Ctrl+Z undoes the previous terrain action instead |
| 4 | Warning | Toolbar/Builder | Number keys 1–9 (tools) and 1–4 (builder modes) do not switch anything; Godot's 3D view shortcuts consume them |
| 5 | Warning | Brush (Raise/Lower) | The bottom strip clips the tool title and Brush Type row at 1600×900; no scrollbar, wheel does not scroll |
| 6 | Warning | Brush/Level/Smooth | Shift/Ctrl+wheel change Size/Strength internally but the strip readouts and slider handles stay stale |
| 7 | Warning | Brush (Drag) | Esc does not clear the draw selection (RMB does); a stale selection causes an accidental height drag |
| 8 | Warning | Paint | Slope Min spin slider is unbounded (one drag sets 8287.5°); painting then silently no-ops |
| 9 | Warning | Paint / Textures | Undo of texture-slot/preset edits restores the data but leaves the panel labels stale (Pattern, Color swatch, Grass Sprite, Scale, Material names, preset name) |
| 10 | Minor | Stamp | Size field stays stale after a drag-stamp or modifier+wheel |
| 11 | Minor | Settings / Chunks | Changing Chunk Size is refused with only an Output warning but still records a no-op undo step |
| 12 | Minor | Gradient | Strength slider ignores click position and jumps erratically on drag |
| 13 | Minor | Brush | Grid Aligned looks enabled on Square grid but silently refuses to toggle |
| 14 | Minor | Brush | Grid Size stays enabled while Grid Aligned is off |
| 15 | Minor | Foliage | Fill Terrain is a no-op for the Tree item on 02_island but records an empty undo step |
| 16 | Minor | Paint | Paint Walls status hint still says "Vertex Painting" and shows a height |
| 17 | Minor | Chunks | An invalid chunk add is only reported as a raw Output warning; the green + handle invited the click |
| 18 | Minor | Presets | Saving a preset with an empty name closes the dialog silently and writes nothing |
| 19 | UI | Presets | The Save Preset dialog has only OK; no Cancel button (Esc/X work) |
| 20 | UI | Settings | Settings page tabs jump vertically between pages; clicking the old tab position can land in the 3D view and clear the selection |
| 21 | UI | Paint/Textures | The ground-texture picker thumbnail is solid black for `mst_noise.png` while the slot preview is correct |
| 22 | UI | Hints | Status/hint lines are clipped at the right edge, and some hints advertise controls that do not exist (Level Strength) or omit working ones (Raise/Lower Ctrl+wheel strength) |
| 23 | Error | Builder | In Face/Edge/Vertex mode the builder strip is 447 px tall and not scrollable, crushing the 3D viewport to 345×39 px at 1600×900 (face selection impossible) |
| 24 | Warning | Builder | At 1280×800 the MST builder layout clips toolbar headers, cuts the strip behind the Materials panel, and leaves a ~172×60 px viewport |
| 25 | Minor | Builder | A newly drawn sibling shape is created but selection/Inspector stay on the previous mesh, so the next paint hits the old mesh |
| 26 | Minor | Builder | `H` cycles modes in reverse order (Face → Object → Vertex → Edge) |
| 27 | UI | Builder | The Modeler layout shows a centred "Modeler / Created Box / Dismiss" card over the model until dismissed |
| 28 | Warning | Pixelizer/Effects | Spin-slider drags leave their field range by orders of magnitude (Pixel Size 44243, Darken 290.19, Sky Coverage 377.4, Water Swell 142.85); same root cause as #8; the Inspector clamps its display while the strip keeps the bad value |
| 29 | Warning | Pixelizer | One integer spin-slider drag creates two undo steps (2→39497; first undo →4357, second →2) |
| 30 | Warning | Pixelizer | Undo of the Dawn preset restores every property but the viewport stays salmon until the scene is reloaded |
| 31 | UI | Pixel World | "+ Add" selects the new node, closing the pixel toolbar/strip/panel; adding a second effect needs a reselection |
| 32 | Warning | Pixel World | Sun Shafts "+ Add" leaves its Sun reference empty and the node inert (alpha/count do nothing); the visible streaks are the renderer's God Rays, not this node |
| 33 | UI | Pixelizer/Effects | Strip page-tab rows jump 35–70 px between pages, so fixed-position tab clicks miss (all pixel and effect strips) |
| 34 | UI | Pixel World | Disabled menu entries look as bright as enabled ones |
| 35 | UI | Pixel World | The panel keeps a scrolled offset when its rows rebuild, hiding the top rows |
| 36 | Warning | Effects | `PixelSmoke` shows no visible smoke plume in hex_island at plausible or large settings |
| 37 | UI | Pixel World | The Lighting Preset row (Dawn/Noon/Dusk/Night/Storm/Snow/Wire) sits under the editor's bottom status bar at 1600×900 until the panel is scrolled |
| 38 | Minor | Effects | Dawn / time 0.25 renders a saturated magenta sky with dense vertical stripes instead of the documented warm dawn |
| 39 | Minor | Effects | Fireflies are invisible at their default size/count at a ~30 m editor camera |
| 40 | Minor | Effects | No lightning flash or bolt was observed in 15 s of Storm frames (could be a long random interval; needs a longer watch) |
| 41 | Minor | Effects | The Inspector's value desyncs from the strip after a strip edit (e.g. Coverage 0.8 in the strip, 0.0 in the Inspector) |
| 42 | Error | Strip layout | Terrain attribute strips have no height cap and no usable scroll; the viewport collapses 574→326→159→121→39 px from 1920 to 1024 (the 39 px minimum is what the reflow loop crashes on) |
| 43 | UI | Paint/Foliage | The side panel is fixed at 250 px with no splitter and no visible scrollbar; only 2.5 of 16 slots are visible at 1600 and Export Texture Preset is ~100 wheel ticks away |
| 44 | UI | Pixel World | At 1024×600 the panel starts at x=1007 (width 250), leaving only 17 px on screen; all Select/Add buttons are unreachable |
| 45 | UI | Settings | At 1024×600 the page fields below ~9 rows clip with no visible scrollbar, the hint is cut and overlapped by "Perspective", and the 39 px viewport persists on short pages such as Storage |
| 46 | UI | Toolbar | The terrain toolbar silently grows to 2–3 columns at 1024, stealing 80–200 px of viewport width; there is no scrollbar or "more" affordance |
| 47 | Warning | Builder | The Builder Preferences dialog is 484×664 and non-resizable; at 600 px height its action row is 61 px off-screen with no scroll |
| 48 | UI | Builder | At 1024×600 the Modeler layout drops the bottom strip and Materials palette, leaving only the dense top toolbar and the viewport |
| 49 | Warning | Runtime | Camera zoom-in in `courtyard` passes through the ground and into the fox (fox becomes a cyan/white wireframe); one wheel-out restores it |
| 50 | UI | Runtime | F1 help is a single ~10 px grey line over bright terrain with no backing panel |
| 51 | UI | Runtime | The F3 Pixel tuner is cramped and clipped at 1024×600; weather controls are below the window and need wheel scrolling |
| 52 | Minor | Runtime | Zooming out exposes the finite water-plane edges; the far sea reads as flat blue with no waves/foam |
| 53 | Minor | Runtime | `courtyard`'s dusk view shows a straight-edged dark navy wedge instead of visible bright sun shafts |
| 54 | Warning | Paint / Textures | Ctrl+Z with the Texture Name LineEdit focused does not reach the terrain; the field's own text undo commits a new rename action, and clicking the 3D view does not release focus |
| 55 | Warning | Chunks | "Vertex Handles" exposes no draggable vertex grip; dragging at a chunk border removes (295→294) or adds (9→10) a chunk instead |
| 56 | Minor | History | Misleading action names: `terrain texture 1 magnify` for Scale, `terrain tint paint` for RMB erase, and all Quick Paint sculpt strokes named `terrain brush with quick paint` |

The remainder of this file gives reproduction steps and evidence. Per-area logs are in
`qa/findings/*.md`; screenshots are referenced inline.

---

## 0. Critical: switching to Level/Smooth/Tint at 1024×600 crashes the editor

**Severity: Critical (crash; unsaved work can be lost)** — editor layout.

Steps:
1. Open `demo/hex_island`, select the terrain, resize the editor to 1024×600.
2. Click **Level** (also reproduced by selecting Level at 1600×900 and then resizing down, and
   with **Smooth** and **Tint**).
3. The harness socket drops and the editor process exits.

Expected: the strip reflows (7 rows at 470 px) and stays interactive; the editor must survive a
tool switch at its own supported minimum window.

Actual: the first runtime message after the resize is
`Failed method: Container::_sort_children. Message queue out of memory...`, followed by hundreds
of thousands of `Object was deleted while awaiting a callback.` lines. This is a **container
layout loop** (a FlowContainer/ScrollContainer reflow at ~470 px that never converges), not a
GPU/llvmpipe allocation failure. Reproduced on 5 separate clean launches (Level twice, Smooth
once, Tint once, Level after a pre-select at 1600). Brush, Paint, Settings, Pixelizer and
Builder did **not** crash at the same size, so the trigger is in the Level/Smooth/Tint strip
content.

Aftermath: if the editor saved a layout at 1024×600, the next launch can enter the same queue-OOM
loop immediately. Reset the editor layout (delete `.godot/editor/editor_layout.cfg`, or the
editor's layout entry in the user config) or start from a clean editor home. The QA sessions
removed `home/` before each clean run.

Evidence: `qa/findings/ui-crash.txt` (excerpt of the 105,978-line / 4.7 MB editor log; OOM at
line 14 after a clean 13-line startup; an earlier crash log grew to 192 MB / 4.1 M lines),
screenshots `qa/screenshots/uiresize/09-brush-1600x900-clean.png` and
`10b-level-1600x900.png` before the crash; no screenshot after (the process was gone).
`qa/findings/ui.md` D1.

Undo: not applicable. Until this is fixed, 1024×600 should be treated as unsupported for
Level/Smooth/Tint.

## 1. Terrain Settings enum and slot dropdowns never apply

**Severity: Error** — Settings → Grid, New Chunks, Display, Storage.

Affected controls (all `OptionButton`s built from `TKind::Enum` / `TKind::Slot`):
Grid Type, Blend Mode, Default Floor Slot, Default Wall Slot, Merge Mode, Extra Collision
Layer, Debug View, Storage Mode, and the other enum rows on the six pages.

Steps:
1. Select `MarchingSquaresTerrain`, click **Settings**, stay on **Grid**.
2. Open **Grid Type** and choose **Hex Rings** (click the item in the popup).
3. The dropdown now reads `Hex Rings`, but:
   - the terrain stays a square grid (viewport unchanged),
   - the Inspector's `Grid Type` stays `Square`,
   - `python3 qa/drive.py prop grid_type` returns `0`,
   - `undo-state` stays empty (no action, `version` unchanged).
4. The label snaps back to the real value the next time the strip rebuilds (e.g. change Cell
   Size or re-select the tool).
5. Same for Blend Mode (`blend_mode` stays 0), Floor Slot (`default_floor_slot` stays 0),
   Debug View (`debug_view` stays 0) and Storage Mode (`storage_mode` stays 0).

Expected: the property is written, the terrain rebuilds, and an undo action
`terrain setting <name>` is recorded (the code path exists for these rows, and numeric/checkbox
rows do work).

Actual: only the `OptionButton` selection changes. The conversion in
`editor/terrain_ui.rs::terrain_setting_changed` for enum/slot/int rows uses
`value.try_to::<f64>()`; the `item_selected` signal supplies an integer, the conversion falls
back to `0`, and `new == old` returns before any undo action is created. This matches all
observed cases (changing away from a non-zero value would write 0 instead).

Evidence: `qa/screenshots/manage/20-settings-hex-select.png` (label `Hex`, Inspector `Square`),
`qa/screenshots/manage/ui/20-settings-hex-select.json`, plus my independent re-test in
`qa/screenshots/manage/verify-gridtype-clicked.png` / `verify-gridtype.png`:
after clicking **Hex Rings**, the strip label changed to `Hex Rings`, `grid_type` stayed `0`
and `undo-state` stayed `version 1 / undo_count 0`.

Undo: no action is created, so Ctrl+Z cannot revert it.

## 2. Save Data / Data Directory are unreachable from Settings

**Severity: Warning** — consequence of #1.

Steps: Settings → Storage with Storage Mode `Resource`; choose `Baked` or `Runtime`. The
Baked-only rows (Bake Grass, Bake Collision, Data Directory, Save Data) never appear. Setting
`storage_mode = 1` with the Inspector (`set-prop storage_mode 1`) and reopening Storage shows
all of them, proving the rows are implemented.

Expected: choosing Baked/Runtime exposes the persistence controls documented in
`docs/02 Your First Terrain.md` (Settings → Storage → Save Data).

Actual: the mode never changes, so a documented workflow is unreachable. Workaround: change
`Storage Mode` in the Inspector.

Evidence: `qa/screenshots/manage/38-storage-mode-baked.png` vs
`qa/screenshots/manage/43-storage-baked-page.png`.

## 3. Grass Animation FPS is not undoable

**Severity: Warning** — Settings → Grass.

Steps: set **Animation FPS** from `0` to `12` (typed) then `13` (arrow). The field and the
shader update, but no `terrain grass animation fps` action is created; Ctrl+Z leaves the value
at 13 and does not add a redo entry.

Expected: the special-case branch in `terrain_setting_changed` creates
`add_do_method`/`add_undo_method` on the turf material and an undo step.

Actual: no undo step; the value cannot be reverted from the UI. Worse, pressing Ctrl+Z after
the change undoes the **previous terrain action** (history 8→7) while the FPS field keeps 12,
so the user can lose an unrelated stroke.

Evidence: `qa/screenshots/manage/28-grass-fps.png`,
`qa/screenshots/undodeep/25-grass-fps.png`; `undo-state` unchanged by the setting.

## 4. Number keys 1–9 do not switch tools

**Severity: Warning** — every tool, documented in `docs/02`, `docs/12` and the tooltips.

Steps: select the terrain, click the 3D view, press `2`, `3`, `4`, `1`, `7`. Repeat with the
Scene tree focused.

Expected: `1`–`9` select Brush/Level/Smooth/Bridge/Foliage/Paint/Inspect/Chunks/Settings, as
the tooltips (`Brush Tool [1]`, …) and the cheat sheet say.

Actual: the tool never changes. With the 3D view focused the digits run Godot's view shortcuts
(`7` → Top Orthogonal, `1` → Front Orthogonal); with the Scene tree focused they do nothing.
Clicking the toolbar works. `MarchingSquaresTerrainPlugin::on_key` does handle `KEY_1..KEY_9`,
but the events are consumed by the editor's 3D view before they reach the plugin. The shortcut
advertisement is therefore incorrect. The builder has the same problem for `1`–`4`
(Object/Face/Edge/Vertex): its state stays `Object` after the digit sequence, while `H` works
(and cycles backwards, see #26).

Evidence: `qa/screenshots/sculpt/52-key7-view.png` (header `Top Orthogonal`),
`qa/screenshots/sculpt/53-key1-view.png` (header `Front Orthogonal`),
`qa/screenshots/sculpt/ui/11-key1-front-orthogonal.json` (`ToolTitle` = `Level`);
`qa/screenshots/builder/ui/27-state.json` (`"mode": "Object"` after `2`/`3`/`4`).

## 5. Raise / Lower strip clips the title and Brush Type row at 1600×900

**Severity: Warning** — Brush → Brush Mode `Raise / Lower`.

Steps: select Brush, set Brush Mode to `Raise / Lower`, look at the bottom strip, then wheel
over the strip.

Expected: every control stays visible, or the strip scrolls with a visible scrollbar.

Actual: the flow needs 5 rows (~175 px) but the strip viewport is 140 px. The title row
(`Brush (Raise / Lower)` + `Brush Type`) is laid out at y≈448, above the strip viewport
(y≈483..623). The visible part starts at `Brush Mode`. There is no scrollbar; the strip's
`ScrollContainer` has both scroll modes disabled, so the wheel does not help. Brush Type is
unreachable until the user switches back to Drag and returns. Reproduced on Square and
Hex Rings.

Evidence: `qa/screenshots/sculpt/18-brush-raiselower-strip.png`,
`qa/screenshots/sculpt/ui/08-hex-raiselower-clip.json` (`ToolTitle`/`Brush Type` rect y=448,
`Brush Mode` y=483), `qa/screenshots/sculpt/23e-strip-tab-scroll.png`. See #42 for the systemic
version (the same uncapped strip collapses the viewport at 1280/1024).

## 6. Shift/Ctrl+wheel change values but the strip readouts stay stale

**Severity: Warning** — Brush (Drag and Raise/Lower), Level, Smooth.

Steps: note Size `15.0` / Strength `4.000`; over the 3D view do Shift+wheel up ×3, then
Ctrl+wheel down ×2, Alt+wheel up ×3 (Raise/Lower).

Expected: the strip's numeric labels and slider handles follow the values (the hints/docs
advertise Shift+wheel = size, Ctrl+wheel = strength).

Actual: the internal values and the cursor footprint change (UI dump: Size 19.0, Strength
3.256, Rotation 45), but the strip still displays `15.0` / `4.000` and the handles do not move.
The labels refresh only when the strip is rebuilt (switching tools then showed Size `18.9`).
Rotation (a spin slider) does refresh. A separate paint test saw Size stay `15.0` through
Shift+wheel for the same reason.

Evidence: `qa/screenshots/sculpt/23-lr-wheel-changes.png`,
`qa/screenshots/sculpt/ui/07-lr-wheel-stale-values.json`,
`qa/screenshots/sculpt/24-brush-drag-strip-back.png`; `qa/screenshots/paint/42-brush-square-aspect-rot.png`.

## 7. Esc does not clear the Brush/Drag selection

**Severity: Warning** — Brush (Drag mode).

Steps: paint a small selection (status `4 selected`), press Esc twice; then right-click and
compare.

Expected: Esc clears the selection (docs/03, the cheat sheet and the on-screen hint
`RMB / Esc: clear`).

Actual: Esc leaves the white selection and the `4 selected` status. RMB clears it. The stale
selection is not inert: a later LMB-drag starting inside it moves the selected cells (height
draw) instead of painting a new selection, which happened during testing. The plugin's
`on_key` does handle Esc, but the event does not reach the 3D plugin (same event-routing
problem as #4), or the editor consumes it first.

Evidence: `qa/screenshots/sculpt/69-esc-clears-selection.png`,
`qa/screenshots/sculpt/69c-esc-clean-test.png`, `qa/screenshots/sculpt/14-brush-rmb-clear.png`.

## 8. Slope Min is effectively unbounded

**Severity: Warning** — Paint (and Foliage uses the same control type).

Steps: Paint tool → drag the `Slope Min` spin slider from x=640 to x=665 at y=605, then paint.

Expected: the value stays within the documented 0–90° range (the field is declared
`Spin(0.0, 90.0, 0.5)`), and painting is limited to that slope range.

Actual: the field becomes `8287.5` (a 25 px drag), because `spin_slider` sets
`allow_greater/allow_lesser`; painting then silently no-ops because no cell can exceed the
value. Typing a value works.

Evidence: `qa/screenshots/paint/37-slope-min-8287.png`,
`qa/screenshots/paint/38-slope-min-blocks-paint.png`.

## 9. Undo of texture-slot/preset edits leaves the panel labels stale

**Severity: Warning** — Paint → Texture slots / Texture Preset.

Steps: set Texture Preset `MST Classic` (Material becomes `1. Texture 1`), then Ctrl+Z.
Repeat for slot **Pattern** (Grass→Forest), **Scale**, **Color** (#ff3333), **Grass Sprite**
and the slot **name**.

Expected: the terrain's texture set and every visible control revert to the previous values
(the data does revert).

Actual: the resource properties revert (`prop tex_assign`, pattern, scale, color and sprite
are correct; a strip rebuild shows the restored values), but the visible controls keep the
edited values until the panel is rebuilt. After the preset undo the Material dropdown lists
`1. Texture 1`, the preset dropdown still says `MST Classic` and the slot names stay
`Texture 1…`; the Color swatch stays red after the tint action is undone. Exporting the terrain
writes the real (restored) names, proving only the UI is stale. Saving/refreshing a preset or
rebuilding the strip corrects the labels.

Evidence: `qa/screenshots/paint/17b-preset-panel.png`,
`qa/screenshots/paint/19-undo-preset-stale-panel.png`,
`qa/screenshots/paint/19b-undo-preset-window.png`;
`qa/screenshots/undodeep/07-pattern-undo.png`, `10-color-undo.png`,
`30-preset-undo-stale.png`, `ui/07-pattern-after-rebuild.json`;
`qa/findings/paint.md` (exported `qapainttest.tres` contains `test grass`).

## 10. Stamp Size field stays stale after drag-stamp or modifier+wheel

**Severity: Minor** — Stamp.

Steps: Stamp → Mountain, Size 24; drag in the viewport to pull size/rotation; or use
Shift+wheel (size), Alt+wheel (rotation). The hint shows `size 63.6` / `size 77.0`, Rotation and
Height update, but the Size field still reads `24.0`.

Expected: the field reflects the value that was used and will be used next.

Actual: only the transient hint shows the truth. The stamp that was placed used the pulled
value (digest changed); the next click stamps with the displayed 24 (or the hidden wheel
value — inconsistent).

Evidence: `qa/screenshots/manage/49-stamp-drag.png`,
`qa/screenshots/manage/52-stamp-alt-wheel.png`.

## 11. Chunk Size is editable but can never change; it still records a no-op undo step

**Severity: Minor** — Settings → Grid.

Steps: type `24` in Chunk Size. The field snaps back to `32`; Output prints
`WARNING: MarchingSquaresTerrain: chunk_size is fixed once chunks exist for this grid type
(clear the data or switch grid type first).` `undo-state` gains `terrain setting chunk size`,
but Ctrl+Z/Ctrl+Shift+Z change nothing.

Expected: the field is disabled (or an inline hint/confirmation explains the restriction) and
no undo step is recorded.

Evidence: `qa/screenshots/manage/23-settings-chunksize.png`,
`qa/screenshots/manage/24-settings-chunksize-redo.png`; editor.log `node.rs:1854`.

## 12. Gradient Strength slider ignores click position and drags oddly

**Severity: Minor** — Gradient strip.

Steps: Strength 0.301; click near the right end of the slider (stays 0.300); drag right
620→675 (goes 0.301→0.279); click x=615 then x=665 (ends 0.2345).

Expected: a normal HSlider jumps to the clicked position (right end ≈ 1.0) and drags
monotonically.

Actual: clicks do not map to the cursor; the value drifts slightly. Use the spin value for
precise strength.

Evidence: `qa/screenshots/manage/70-gradient-strength.png` and the saved UI dumps.

## 13. Grid Aligned looks enabled on Square grid but silently refuses to toggle

**Severity: Minor** — Brush.

Steps: on Square grid, with Falloff off, click Grid Aligned (or focus + Space).

Expected: disabled/greyed out when unsupported (as docs/03 describes), or it toggles.

Actual: the checkbox reports `disabled=false`, looks normal, and clicking does nothing (no
check, no feedback). Its tooltip says it needs Triangle/Hexagon/Hex Rings. On Hex Rings it
works, and enabling Falloff auto-unchecks it (correct).

Evidence: `qa/screenshots/sculpt/30b-gridaligned-click2.png`,
`qa/screenshots/sculpt/ui/09-gridaligned-square.json`.

## 14. Grid Size stays enabled while Grid Aligned is off

**Severity: Minor (UI)** — Brush, Gradient, Tint strips.

Steps: uncheck Grid Aligned; the Grid Size HSlider still reports `disabled=false` and shows 1.

Expected: dependent control disabled, or an explanation that it pre-sets the value for later.

Evidence: `qa/screenshots/manage/ui/70-brush-gridaligned.json`, `qa/screenshots/manage/01-initial.png`.

## 15. Fill Terrain is a no-op for the Tree item on 02_island

**Severity: Minor** — Foliage.

Steps: 02_island, Foliage, Tree selected, Slope Min 0, click Fill Terrain. `get_scatter_count(0)`
stays 97; the first click adds an empty `terrain foliage fill` undo entry; later clicks add
none. Rock fill on the same terrain works (766→752, undo/redo verified), so the button is
wired. Likely the Tree item's ground-slot mask matches no cell; the UI gives no reason.

Evidence: `qa/screenshots/paint/49-fill-terrain-slope0.png`,
`qa/screenshots/paint/53-fill-rock.png`.

## 16. Paint Walls hint still says "Vertex Painting"

**Severity: Minor** — Paint with Paint Walls ticked.

Expected: the live hint acknowledges wall mode.
Actual: the action is a distinct `terrain wall paint` undo step (good), but the hint reads
`Vertex Painting | height 5.00 cell ...`.

Evidence: `qa/screenshots/paint/34-paint-walls-snow.png`.

## 17. Invalid chunk add only reports a raw Output warning

**Severity: Minor** — Chunks.

Steps: click a green `+` that is not adjacent to an existing chunk.
Expected: invalid handles are not offered, or an in-viewport transient hint explains why.
Actual: Output prints `WARNING: Marching Squares Terrain: new chunks must touch an existing
chunk.` and nothing else happens. The handle was drawn, so the user is invited to click it.

Evidence: `qa/screenshots/manage/08-chunks-removed.png`; editor.log `plugin.rs:1244`.

## 18. Empty preset name closes the Save Preset dialog silently

**Severity: Minor** — Paint → Export Texture Preset.

Steps: clear the name field, press Enter. The dialog closes, no `.tres` is written, no message.
Expected: inline validation or the dialog staying open.

Evidence: `qa/screenshots/paint/22-save-preset-empty-name.png`.

## 19. Save Preset dialog has no Cancel button

**Severity: UI** — Paint → Export Texture Preset.

The dialog shows only `OK`; Esc and the window X cancel. docs/04 describe OK/Cancel. A
`window-resize 400x150` request is clamped to **558×150** because the one-line path label
("Preset name (saved to res://addons/.../)") has a 540 px minimum; 800×220 is accepted and
leaves a large blank area. Add a real Cancel button and a compact label/wrap so the dialog can
be ~420 px wide.

Evidence: `qa/screenshots/paint/20-save-preset-dialog.png`,
`qa/screenshots/uiresize/50-save-preset-1024x600.png`,
`qa/screenshots/uiresize/51-save-preset-narrow-400x150.png`,
`qa/screenshots/uiresize/52-save-preset-large-800x220.png`.

## 20. Settings page tabs jump vertically between pages

**Severity: UI** — Settings.

Steps: Grid (tabs at y≈378) → New Chunks (≈448) → Grass (≈483) → LOD (≈518) → Storage (≈448).
Because the strip grows upward from the bottom, the tab row moves under the pointer; clicking
where the tabs were can land in the 3D view and clear the terrain selection, which hides the
whole toolbar/strip.

Expected: a stable tab row (top-anchored/fixed height) or a strip that scrolls instead of
changing height. The page content itself fits at 1600×900.

Evidence: `qa/screenshots/manage/17-settings-grid.png`, `.../25-settings-new-chunks.png`,
`.../27-settings-grass.png`, `.../35-settings-lod.png`, `.../43-storage-baked-page.png`.

## 21. Ground-texture preview is black for `mst_noise.png`

**Severity: UI** — Paint → slot `Texture:` resource picker.

With Texture Preset `MST Classic`, slots using `textures/ground/mst_noise.png` show a solid
black picker thumbnail; the slot's small tinted preview on the left is correct, and the tooltip
names the right file. Other textures preview correctly.

Evidence: `qa/screenshots/paint/17b-preset-panel.png`,
`qa/screenshots/paint/19-undo-preset-stale-panel.png`.

## 22. Hint/status text is clipped and some hints disagree with the controls

**Severity: UI** — viewport status lines.

- The Brush selection status is cut at the right edge at 1600×900 (`... RMB/Esc: c...`).
- Raise/Lower lists `Shift+wheel: size` but omits `Ctrl+wheel: strength` (which works).
- Level says `Ctrl+wheel: strength`, but the Level strip has no Strength field to observe.
- At 1024×600 the Brush hint is taller than the 39 px viewport; the Settings hint is cut on the
  left and overlapped by the "Perspective" label.

Evidence: `qa/screenshots/uiresize/04-brush-1024x600.png`,
`qa/screenshots/uiresize/60-settings-1024x600.png`,
`qa/screenshots/sculpt/69c-esc-clean-test.png`,
`qa/screenshots/sculpt/18-brush-raiselower-strip.png`,
`qa/screenshots/sculpt/31-level-strip.png`.

## 23. Builder Face/Edge/Vertex strip squeezes the 3D viewport to 39 px

**Severity: Error** — PixelBuilderMesh, MST layout, any non-Object mode.

Steps: select `HexIsland/BuilderRuin/Ruin`, stay in the default MST layout, click
**Mode → Face** (the `2` key is dead, see #4).

Expected: the bottom attribute strip is scrollable/collapsible and leaves a usable 3D view for
selecting faces/edges/vertices.

Actual: Face mode shows the full operation set (Color, Grid, Dist, Inset, Profile, Inset by,
Subdiv, Axes, Trim at, T depth/height, Off x/y/z, Space, Grow, Extrude, To new object). The
strip's minimum size is **238×447** and it is not inside a `ScrollContainer`, so the VSplit
gives it 447 px and the 3D viewport becomes **345×39 px** (global rect `[706,154,345,39]`) at
1600×900. The HUD status lines are clipped away; clicks aimed at the model land on the strip.
Face selection is effectively impossible in this mode without switching to the Modeler layout.
Reproduced immediately after creating a Box and switching to Face.

At 1280×800 the strip wraps to **238×699** and the viewport is **238×39**; at 1024×600 the
same strip is **238×699** with its bottom rows clipped by the window.

Evidence: `qa/screenshots/builder/14-box-selected-face-mode.png`,
`qa/screenshots/builder/15-face-selected.png`,
`qa/screenshots/builder/ui/27-builder-1600x900.json` (strip min/rect; viewport rect);
`qa/screenshots/uiresize/21-builder-face-1600x900.png`,
`22-builder-face-1280x800.png`, `23-builder-face-1024x600.png`.

Undo: not applicable (layout).

## 24. Builder MST layout clips badly at 1280×800

**Severity: Warning** — same surface, smaller editor.

Steps: select the builder mesh at 1280×800, wait for the relayout.

Expected: toolbar labels, strip fields and the viewport stay readable; fields wrap or scroll.

Actual: the viewport shrinks to ~172×60 px; the bottom strip is cut by the Materials panel
(only Slot, Color, Grid, Subdiv, Weld and the mirror checkboxes are visible; the "Editing
builder mesh" label is clipped); the toolbar headers collide with the icons (`Combi` for
Combine); the Materials palette loses its lower content. Restoring 1600×900 restores it.

Evidence: `qa/screenshots/builder/28-builder-1280x800.png`,
`qa/screenshots/builder/29-builder-restored-1600x900.png`.

## 25. A newly drawn sibling shape is not selected

**Severity: Minor** — Shape draw without Shift.

Steps: select `Ruin`, Shape Box, `C`, drag a footprint, move up, click to finish.

Expected: the new sibling is ready to edit/paint (docs continue with `W`/`B`).

Actual: the new `Box` is added under `BuilderRuin/Box`, but the Scene tree, Inspector and
builder target remain on `Ruin` (the HUD still reports the 209-vertex Ruin). Pressing `B` right
after drawing paints the old mesh. Selecting `BuilderRuin/Box` explicitly fixes it.

Evidence: `qa/screenshots/builder/13-shape-created.png`; undo works (`Pixel Builder: Create
Box`, one Ctrl+Z removes the new Box).

## 26. `H` cycles builder modes in reverse

**Severity: Minor** — Builder.

Steps: start in Face, press `H` four times.

Expected: a predictable cycle in the documented 1→2→3→4 order.

Actual: `Face → Object → Vertex → Edge → Face` (mode − 1). Works, but is surprising next to
the dead number keys.

Evidence: `qa/screenshots/builder/06-mode-vertex.png`.

## 27. Modeler layout shows a blocking centred card

**Severity: UI** — Builder → Modeler layout.

Steps: switch to Modeler layout and draw a shape; look at the centre of the 3D view.

Expected: status stays in the top-left HUD; the viewport stays clear.

Actual: a centred "Modeler / Created Box / Dismiss" card covers the model and blocks clicks on
the faces underneath until dismissed. It also appears immediately after toggling the layout.

Evidence: `qa/screenshots/builder/16-modeler-layout-face.png`.

Builder log note: the six `ERROR` lines mentioning `PixelBuilderPlugin::debug_placement` in
`qa/screenshots/builder/editor.log` were induced by the QA harness passing JSON to a
`Vector2`-typed test hook; they are not addon defects and are excluded from the findings.

## 28. Spin-slider drags leave their field range by orders of magnitude

**Severity: Warning** — Pixelizer sections; same root cause as #8 (Paint Slope Min).

Steps: select `PixelRenderer`; on the Renderer page drag **Pixel Size** (value 3) 55 px to the
right over 1.2 s; on the Outlines page drag **Darken** (0.55, range 0–1) 40 px.

Expected: drags clamp to the documented ranges (Pixel Size 1–16, Darken 0–1); typing larger
values is a separate affordance.

Actual: `pixel_size` became **44243** and `outline_darken` became **290.19**. The viewport
degrades to 1-px blocks / near-black terrain. `editor/ui_kit.rs::spin_slider` calls
`set_allow_greater(true)` and `set_allow_lesser(true)` on every field, including Float fields
that did not ask for it. This is the same defect that lets Paint → Slope Min reach 8287.5°
(#8); the systemic fix is to reserve `allow_greater/lesser` for fields that explicitly opt in
and clamp slider drags to the declared range.

The same thing reproduces on effect strips: PixelSky **Coverage** went 0.5 → **377.40** and
PixelWater **Swell H.** 0.121 → **142.85** in one 20–30 px drag. The node property keeps the
out-of-range value while the Inspector clamps its own display (Coverage shows 1.0/0.0 while the
strip holds 377.4), so the two disagree (#41).

Evidence: `qa/screenshots/pixel/20b-pixelsize-44243.png`,
`qa/screenshots/pixel/21-outlines-darken.png`,
`qa/screenshots/pixel/20e-pixelsize-restored.png`,
`qa/screenshots/effects/10-sky-coverage-377.png`,
`qa/screenshots/effects/46-water-waves-swell.png`.

## 29. One integer spin-slider drag creates two undo steps

**Severity: Warning** — Pixelizer integer fields.

Steps: on the Renderer page record `undo-state` (0 actions), drag **Pixel Size** 55 px,
`undo-state` → `undo_count: 2`. Ctrl+Z once goes to 17448 (a mid-drag value); a second Ctrl+Z
goes to 3; one Ctrl+Shift+Z returns to 17448.

Expected: one continuous drag is one undo step, like the Float **Darken** field (verified one
step).

Actual: two steps, so the first undo lands on an intermediate value. In the deep-undo re-test
the value went 2 → **39497**, the first undo → **4357** (mid-drag), the second → **2**, and one
redo returned 4357. This can also interact with the out-of-range drag in #28.

Evidence: `qa/screenshots/pixel/20c-pixelsize-undo1.png`,
`qa/screenshots/undodeep/51-pixelsize-drag.png`,
`qa/screenshots/undodeep/ui/51b-pixelsize-undo1.json`,
`qa/screenshots/undodeep/ui/51c-pixelsize-undo2.json`,
`qa/screenshots/pixel/20d-pixelsize-undo2.png`,
`qa/screenshots/pixel/20e-pixelsize-restored.png`; Output shows "Set Renderer Pixel Size"
twice.

## 30. Undo of the Dawn preset leaves the viewport tinted salmon

**Severity: Warning** — Pixel World lighting presets.

Steps: Pixel World → **Dawn** (sky/cloud deck turns salmon), then Ctrl+Z. `time_of_day`,
deck colors, god rays, beam tint, ray alpha, day cycle and fog all return to their pre-preset
values, but the viewport stays salmon; toggling the cloud deck, god rays off, and scrubbing
time does not clear it. Closing the scene without saving and reopening restores blue.

Expected: undo restores the rendered look, not only the properties.

Actual: the editor preview keeps a cached dawn-tinted sky/global state until the
scene/viewport is rebuilt. Editor-only (nothing is saved), but the user cannot tell the preset
was undone.

Evidence: `qa/screenshots/pixel/72-lighting-dawn.png`,
`qa/screenshots/pixel/83-godrays-off.png`, `qa/screenshots/pixel/84-daynight-time-05.png`,
`qa/screenshots/pixel/85-fresh-scene.png`.

## 31. Pixel World "+ Add" closes the panel by selecting the new node

**Severity: UI** — Pixel World panel.

Steps: select `PixelRenderer`, click **+ Add** next to `Sun Shafts` (or `Wild Flowers`).

Expected: the panel stays open long enough to add another effect (or focus returns to the
renderer/terrain).

Actual: the new node is added and selected, and the pixel toolbar, strip and Pixel World panel
all disappear because the pixelizer UI follows the selection. Adding two effects in a row
takes two reselections.

Evidence: `qa/screenshots/pixel/68-sunshafts-added.png`,
`qa/screenshots/pixel/71-wildflowers-added.png`,
`qa/screenshots/pixel/70-pixel-ui-restored.png`; undo of Add works as one step.

## 32. Sun Shafts "+ Add" leaves the Sun reference empty and the node inert

**Severity: Warning** — Pixel World "+ Add" / "Wire Sun / Env Refs".

Steps: add `Sun Shafts`; inspect the new node (Sun: `Assign...`); click **Wire Sun / Env
Refs**.

Expected: the `+ Add` tooltip and docs/08 promise wired sun/environment references.

Actual: the Sun stays empty and the wire action prints `Pixel World: wired 0 reference(s).`
(`wire_references` only covers PixelSky, PixelDayNight and PixelWeather). The node is also
inert: changing Alpha 0.011 → 0 and Count 12 → 30 leaves the viewport unchanged. The thin
yellow streaks visible in the scene are the renderer's **God Rays**, not this node. Visible
shafts need a manual Sun assignment.

Evidence: `qa/screenshots/pixel/68-sunshafts-added.png`,
`qa/screenshots/pixel/74-wire-refs.png`,
`qa/screenshots/effects/67-sunshafts-default.png`,
`qa/screenshots/effects/69-sunshafts-alpha0.png`,
`qa/screenshots/effects/70-sunshafts-count30.png`; editor.log informational line.

## 33. Strip page-tab rows jump vertically between pages

**Severity: UI** — every pixelizer section and the effect strips.

Steps: Renderer page (tabs at y≈485); click `Rays` (y≈520); click `Sheets` (y≈555); try a tab
click at the previous y. For effects: PixelWater Surface page (tabs y≈519) → Waves (y≈449) →
try to click Light at the old y.

Expected: a stable tab bar (the strip is bottom-anchored, but the tab row should not move by a
whole row).

Actual: the tab bar shifts 35–70 px as the page's field count changes; a fixed-position click
lands in the content area. This is the same class as #20 (Settings pages) and explains an
earlier interrupted run whose fixed-position captures were identical.

Evidence: `qa/screenshots/pixel/11-renderer-renderer.png`,
`qa/screenshots/pixel/13-renderer-rays.png`,
`qa/screenshots/pixel/15-renderer-sheets.png`; UI dumps in `qa/screenshots/pixel/ui/`;
`qa/screenshots/effects/ui/45-water-page-*.png` (Waves tab `[535,449]` vs `[535,519]`).

## 34. Disabled Pixel World menu entries look enabled

**Severity: UI** — Pixel World menu.

Steps: open **Pixel World**; compare entries disabled because the node exists (Renderer, Sky &
Clouds, Day/Night, Weather, Water, Diagnostics) with enabled ones.

Expected: disabled entries are visibly greyed.

Actual: the text brightness is the same; only the tiny leading icon is dimmer. Clicking an
already-present effect appears to do nothing.

Evidence: `qa/screenshots/pixel/04-pixel-world-menu.png`.

## 35. Pixel World panel keeps a scrolled offset when rows rebuild

**Severity: UI** — Pixel World panel.

Steps: with `PixelRenderer` selected, the panel starts at the top; click a section icon; the
panel now starts at `Particle Field`, hiding Renderer/Sky/Water; the vertical scrollbar is only
visible on hover.

Expected: the panel returns to the top after a section switch, or scrolls the selected row into
view.

Actual: the offset is preserved/re-applied (content y ≈ −104), so the top of the list
disappears without an obvious cue.

Evidence: `qa/screenshots/pixel/02-pixel-toolbar.png` vs
`qa/screenshots/pixel/03-sky-section.png`,
`qa/screenshots/pixel/73-panel-scrolled-bottom.png`.

## 36. PixelSmoke shows no visible plume in hex_island

**Severity: Warning** — Smoke & Fire node.

Steps: select `Smoke` (Emitting on, Fire on, position 14,5,−2, Rate 2.5, Start/End 0.1/0.45),
watch 4 s with Animate on, then raise Rate to 5 and Start/End to 1.0/2.0.

Expected: a grey plume rising from the fire, with a flickering light.

Actual: only a tiny orange flame pixel near the building base; no smoke puffs at either size and
no plume above the terrain. No addon error is logged. The authored transform was restored after
a harness `set-prop` mistake; the result is the same at the authored position. Worth checking
whether the puff material/emission or the authored position is at fault.

Evidence: `qa/screenshots/effects/58-smoke-a.png`,
`qa/screenshots/effects/61-smoke-fire-center.png`,
`qa/screenshots/effects/64-smoke-big.png`,
`qa/screenshots/effects/66-smoke-restored.png`.

## 37. Lighting Preset row is hidden under the bottom status bar

**Severity: UI** — Pixel World panel.

Steps: select any pixel node so the Pixel World panel opens; look at the **Lighting Preset**
row (Dawn/Noon/Dusk/Night/Storm/Snow/Wire Sun/Env Refs) without scrolling.

Expected: all preset buttons are visible and clickable at 1600×900.

Actual: the row sits at y = 848–917 in a 900 px window; the editor's bottom status bar covers it.
Only a 1-px sliver of Night is reachable; Dawn/Noon/Dusk do not respond until the panel is
scrolled (the panel does scroll, so the block is reachable). The presets are a primary action
and should be pinned or placed above the fold.

Evidence: `qa/screenshots/effects/07-sky-strip.png`,
`qa/screenshots/effects/19-daynight-preset-night.png`; UI dump rects `Dawn [1067,848,72,33]`,
`Night [1067,884,72,33]` at window height 900.

## 38. Dawn / time 0.25 sky is saturated magenta with stripes

**Severity: Minor** — PixelDayNight / PixelSky.

Steps: set **Time** to 0.25, then apply the **Dawn** preset.

Expected: a warm dawn sky (the preset's deck tint is warm white/orange in the docs).

Actual: the whole sky is neon pink/magenta with a dense vertical dither pattern; terrain reads
bright green/orange. At 0.4 the same sky is grey-blue and foggy. The preset's color values may
be intended for a different color space/tonemapping, or the deck's dither is too strong.

Evidence: `qa/screenshots/effects/16-daynight-time-0.25.png`,
`qa/screenshots/effects/20-daynight-preset-dawn.png`.

## 39. Fireflies are invisible at their default size/count

**Severity: Minor** — PixelFireflies.

Steps: set `PixelDayNight.Time = 0.9` (night); select Fireflies (count 48, size 0.12, energy 3,
night-only 1); then raise size to 0.6 and count to 200.

Expected: a visible blinking swarm at the documented 0.1–0.2 size.

Actual: nothing visible at 0.12/48 even at ~30 m; at 0.6/200 the swarm appears, but the dots
read white/blue rather than the warm yellow tint at energy 3. Defaults are too small for the
demo camera, and the tint may be washed out.

Evidence: `qa/screenshots/effects/51-fireflies-night-close.png`,
`qa/screenshots/effects/54-fireflies-big-night.png`,
`qa/screenshots/effects/53-fireflies-noon-close.png` (invisible at noon, expected).

## 40. Storm lightning was not observed

**Severity: Minor** — PixelWeather.

Steps: Weather = Storm, Transition 0.2, Strength 1, Lightning enabled, Flash 2.5; capture
15 frames over ~15 s with Animate on.

Expected: periodic flashes/bolts.

Actual: frame brightness stayed 95.1–96.2; no bolt or flash was captured. This may be a long
random interval, so it needs a longer watch before being called a defect; the current sample is
inconclusive.

Evidence: `qa/screenshots/effects/31-storm-shot-1.png` … `31-storm-shot-5.png`.

## 41. Inspector values desync from the strip after strip edits

**Severity: Minor** — effect nodes.

Steps: select `PixelSky`, change **Coverage** in the bottom strip to 0.8, read the Inspector's
Coverage.

Expected: both show 0.8.

Actual: the strip shows 0.8 and the node property is 0.8, while the Inspector still shows 0.0.
After an out-of-range drag the Inspector showed 1.0 while the strip held 377.4. The strip's
`renew()` path updates its own controls but does not notify the Inspector.

Evidence: `qa/screenshots/effects/11-sky-coverage-08.png` (strip 0.8, Inspector 0.0).

## 42. Terrain attribute strips have no height cap; the viewport collapses to 39 px

**Severity: Error** — every terrain tool strip.

Steps: select the terrain, keep Brush active, resize 1920 → 1600 → 1366 → 1280 → 1024.

Expected: the strip keeps a bounded height (or scrolls internally) and the 3D viewport keeps a
usable size (at least 480×300).

Actual: viewport heights **574 → 326 → 159 → 121 → 39 px**; strips **105 → 140 → 175 → 245 →
245 px**. At 470 px strip width the MstFlow is 7 rows / 245 px; the `ScrollContainer` reports
its content minimum, so the VSplit gives the strip 245 px and clamps the viewport to its 39 px
minimum. There is no visible scrollbar and no height cap. 1280×800 is worse than 1366×768
because the narrower strip wraps into more rows. At still narrower effective widths the same
reflow produces the queue-OOM crash in #0.

Expected fix: a fixed max height (e.g. 240 px and ≤35% of the 3D area) with a visible vertical
scrollbar and a collapse chevron; never let the viewport drop below 480×300, auto-collapsing
the side panel, then the strip, then the toolbar as needed.

Evidence: `qa/screenshots/uiresize/01-brush-1920x1080.png`,
`09-brush-1600x900-clean.png`, `02-brush-1366x768.png`, `03-brush-1280x800.png`,
`04-brush-1024x600.png` and the matching UI dumps.

## 43. Paint/Foliage sidebar is fixed at 250 px with no visible scrollbar

**Severity: UI** — Paint and Foliage side panels.

Steps: Paint at 1600×900; try to resize the right "Textures" panel; wheel over it; look for the
Export Texture Preset action.

Expected: a draggable splitter (220–420 px), a visible scrollbar, and the export action
reachable without dozens of wheel turns.

Actual: fixed 250 px, no splitter, no visible scrollbar (it only appears on hover, if at all).
Only **2.5 of 16** slots are visible at 1600; 40 wheel ticks advanced about 7 slots (~6 ticks
per slot), so the bottom Export button is ~100 ticks away. At 1024 the panel covers a 39 px
viewport, the slots are cut at the window bottom, and the Paint strip is clipped after
Slope Min/Max.

Evidence: `qa/screenshots/uiresize/40-paint-1600x900.png`,
`41-paint-scrolled-1600x900.png`, `42-paint-bottom-1600x900.png`,
`43-paint-1024x600.png`.

## 44. Pixel World panel is 17 px on screen at 1024×600

**Severity: UI** — Pixelizer.

Steps: select `PixelRenderer`; resize to 1024×600.

Expected: the Pixel World panel stays fully visible, or collapses to a tab.

Actual: `PixelizerAttributes`/`PixelizerToolbar` consume the width; `PixelWorldPanel` is placed at
x = **1007** with width 250, so only 17 px is inside the 1024 window. All Select/Add buttons are
unreachable. The viewport is **670×39**; the nine page tabs just fit (Debug Views ends at
x=1004).

Evidence: `qa/screenshots/uiresize/31-pixelizer-1024x600.png`,
`qa/screenshots/uiresize/ui/31-pixelizer-1024x600.json`.

## 45. Settings fields and hint are clipped at 1024×600

**Severity: UI** — Settings pages.

Steps: visit Grid, New Chunks, Grass, Display, LOD & Nature, Storage at 1024×600.

Expected: all six tabs and every field reachable; the strip scrolls; the hint is readable.

Actual: the six tabs fit (x=426..854), but fields below the first ~9 rows are clipped with no
visible scrollbar; the hint is cut on the left ("…ings: edit them in the strip below") and
overlapped by the "Perspective" label. The 39 px viewport persists even on Storage, which only
needs 4 rows, because the split ratio is not returned.

Evidence: `qa/screenshots/uiresize/60-settings-1024x600.png`,
`61-settings-*.png`, `45-settings-1600x900.png`.

## 46. Terrain toolbar grows to 2–3 columns at 1024 and steals width

**Severity: UI** — terrain toolbar.

Steps: at 1024×600 compare Brush (2 columns, 236 px) with Paint/Settings (3 columns, ~320 px);
observe where the 3D view starts.

Expected: a fixed 30 px icon rail, optionally scrollable; compact mode explicit.

Actual: the `VFlowContainer` wraps based on available height and expands horizontally to keep
all 16 tools visible. At 1024 Paint the viewport starts at x≈507 instead of 418. All tools are
reachable in the Brush layout, but the cost is 80–200 px of viewport width with no scrollbar or
"more" affordance.

Evidence: `qa/screenshots/uiresize/04-brush-1024x600.png` (2 columns),
`43-paint-1024x600.png` and `60-settings-1024x600.png` (3 columns),
`00-terrain-toolbar-1600x900.png`.

## 47. Builder Preferences dialog is taller than a 600 px window and not resizable

**Severity: Warning** — Builder → Preferences.

Steps: select `BuilderRuin/Ruin`, Builder menu → Preferences, resize to 1024×600.

Expected: the dialog fits, scrolls, or is resizable; OK/Close reachable.

Actual: window `Pixel Builder Preferences` is **484×664** at (270,−3), `resizable: false`,
`min [0,0]`; the bottom 61 px (including the action row) is off-screen and there is no scroll.

Evidence: `qa/screenshots/uiresize/24-preferences-dialog-1024x600.png`.

## 48. Modeler layout loses its strip and Materials palette at 1024

**Severity: UI** — Builder, Modeler layout.

Steps: Builder menu → Modeler layout at 1024×600.

Expected: the larger palette/Add Mesh grid remains usable alongside the viewport.

Actual: only the dense top toolbar (Add Mesh, Actions, icons) and the viewport remain; the
bottom strip and Materials panel are gone, and the viewport is still halved by the Output panel.
At 1600 the same layout looks correct.

Evidence: `qa/screenshots/uiresize/24c-builder-modeler-1024x600.png`,
`25-builder-modeler-1600x900.png`.

## 49. Camera zoom clips through the ground and the character

**Severity: Warning** — runtime camera, `demo/courtyard.tscn`.

Steps: open the scene at 1280×720, wait for the default camera, middle-wheel up twice at the
centre of the view.

Expected: zoom-in stops before the camera enters the terrain or the fox.

Actual: the camera drops below the grass plane and into the fox; the view becomes a flat
dark-green plane with trees on the horizon, and the fox is drawn as a cyan/white
wireframe-like silhouette (backfaces/outline colour). One wheel-out restores the normal view.

Evidence: `qa/screenshots/runtime/39-courtyard-orbit.png` (broken close-up),
`qa/screenshots/runtime/40-courtyard-restored.png`.

## 50. F1 help is one tiny, low-contrast line

**Severity: UI** — runtime HUD.

Steps: open `demo/hex_island.tscn`, press F1 at 1280×720, 1920×1080 and 1024×600.

Expected: a readable controls hint/overlay (docs/11: "F1 — Show the controls again").

Actual: a single ~10 px line of grey text pinned to the bottom edge
(`WASD move · Shift walk · Space jump · drag / arrows orbit · Wheel zoom · F3 tuner · F1 help`)
drawn directly over bright grass. It is readable at 1:1 but small, contrast varies with the
terrain behind it, and there is no panel/background or multi-line help. It fits at all three
sizes and toggles cleanly.

Evidence: `qa/screenshots/runtime/02-hex-1280-f1.png`,
`qa/screenshots/runtime/31-hex-1920-f1.png`,
`qa/screenshots/runtime/36-hex-1024-f1.png`.

## 51. F3 Pixel tuner is cramped and clipped at 1024×600

**Severity: UI** — runtime diagnostics overlay.

Steps: open `hex_island` at 1024×600, press F3.

Expected: the tuner fits the window, or scrolls with a visible scrollbar / can be collapsed.

Actual: the panel starts at x≈604 (59% of the 1024-wide window) and its content is cut at the
bottom edge; only the top of the `PixelWeather: Weather` section is visible, and the
Clear/Cloudy/… buttons and fields are below the window. It requires wheel-scrolling and leaves
almost no room for the game. At 1280 and 1920 the same panel is fine and scrolls.

Evidence: `qa/screenshots/runtime/35-hex-1024-f3.png` (clipped),
`qa/screenshots/runtime/03-hex-1280-f3.png` (fine at 720p).

## 52. The water plane's finite edge is exposed when zooming out

**Severity: Minor** — runtime water.

Steps: open `tutorial/02_island.tscn`, wheel down twice and orbit; in `hex_island`, orbit to the
coast and zoom out five steps.

Expected: water continues to the horizon or fades into the sky; waves/foam stay visible near
the shoreline.

Actual: tutorial 02 shows hard diagonal edges of the finite water plane with flat light blue
beyond it. In hex_island the zoomed-out coast is a large uniform blue field with a hard horizon
and no readable waves or shoreline foam at that distance (foam/caustics are clear at the default
distance in tutorials 01/02).

Evidence: `qa/screenshots/runtime/44-t02-orbit.png`,
`qa/screenshots/runtime/33-hex-1920-sea-view.png`.

## 53. Courtyard dusk shows a dark wedge instead of bright sun shafts

**Severity: Minor** — runtime lighting, `demo/courtyard.tscn`.

Steps: open the courtyard and wait for the default dusk view.

Expected: `PixelSunShafts` adds bright shafts through the trees/pillars (docs/11, docs/09).

Actual: the upper-left sky has a large, straight-edged dark navy wedge with no bright shaft
visible. It may be a cloud shadow or a shaft sheet failing to add light; orbiting/zooming
changes the wedge, so it is view-dependent.

Evidence: `qa/screenshots/runtime/38-courtyard-1280.png`,
`qa/screenshots/runtime/40-courtyard-restored.png`.

## 54. Ctrl+Z is trapped in the Texture Name LineEdit and records a new rename

**Severity: Warning** — Paint → Texture Name.

Steps: click the slot **Texture Name** field, type `undotest`, press Enter, then Ctrl+Z while
the field still has focus; click the 3D view and press Ctrl+Z again.

Expected: Ctrl+Z in the field edits the text (or at least does not touch terrain history);
clicking the 3D view leaves the field so Ctrl+Z undoes the rename.

Actual: with the field focused, Ctrl+Z does not reach the terrain; the field's text undo changes
the text, but because the field applies on every change the restored text is committed as a
**new** `terrain texture 1 display_name` action (history 24→25, redo stack lost). Clicking the
3D viewport does **not** release the LineEdit focus, so Ctrl+Z remains stuck in the field.
Clicking a toolbar button (e.g. Brush) frees it and the rename can then be undone normally.

Evidence: `qa/screenshots/undodeep/32-rename.png`,
`qa/screenshots/undodeep/33-lineedit-ctrlz.png`,
`qa/screenshots/undodeep/34-lineedit-raw-ctrlz.png` (`undo-state` 23→24→25 while focused,
then 25→24 after clicking Brush).

## 55. "Vertex Handles" has no draggable grip; chunk-border drags add/remove chunks

**Severity: Warning** — Chunks → Vertex Handles.

Steps: Chunk Manager, tick **Vertex Handles**; drag the green handle/line at the selected chunk
edge on hex_island; repeat on 02_island with the red circle handle; drag the green vertical line
endpoint.

Expected: a vertex handle can be dragged to change the chunk-corner height; the History entry
names a vertex edit.

Actual: no drag produced a vertex edit (`undo-state` unchanged). A drag at the hex chunk border
**removed** the selected chunk (`remove chunk`, 295→294), and a drag at a red circle in
02_island **added** a chunk (`add chunk`, 9→10). Both are undoable, but the handles look like
vertex grips and the hint only says "LMB +: add chunk · LMB on a chunk: remove it", so it is
easy to remove a chunk while trying to grab a vertex. Whether true vertex grips exist for this
grid was not proven.

Evidence: `qa/screenshots/undodeep/45-vertex-handles.png`,
`qa/screenshots/undodeep/46-vertex-drag.png`,
`qa/screenshots/undodeep/48-02-vertex-handles.png`.

## 56. Misleading History action names

**Severity: Minor** — History dock.

Steps: after a Paint slot **Scale** drag, a Tint **RMB erase**, and any sculpt stroke with
Quick Paint enabled, read the History dock.

Expected: an entry that names what happened (Scale, tint erase, level with Quick Paint).

Actual: Scale is `terrain texture 1 magnify`; RMB tint erase is `terrain tint paint`; with
Quick Paint set, Level/Smooth/Bridge strokes are all named `terrain brush with quick paint`, so
the tool that changed the terrain is not in the name. The order itself is correct.

Evidence: `qa/screenshots/undodeep/53-history-dock.png`,
`qa/screenshots/undodeep/ui/59-gradient-strip.json`,
`qa/screenshots/undodeep/ui/41-stamp-strip.json`.

---

## Undo / redo — verified working

The user-requested Ctrl+Z check is positive for terrain edits. Every stroke captured one
`terrain …` undo action, Ctrl+Z restored the exact terrain digest, and Ctrl+Shift+Z re-applied
it. The matrix (`qa/screenshots/sculpt/undo-matrix.txt`) covers:

- Brush selection moves, Raise/Lower, Level, Smooth (Terraced and Sloped), Bridge — one stroke
  = one step; two strokes = two Ctrl+Z; redo after each.
- Undo after switching tools, after clicking in the 3D view, and after opening another node.
- Ctrl+Z while a LineEdit has focus edits the text, not the terrain.
- Cross-chunk stroke touching 5 chunks restores all per-chunk hashes in one step; the redo
  stack is cleared by a new edit.
- Paint strokes, wall paint, foliage paint/erase/fill/clear, stamps (click and drag), gradient,
  selection paste, chunk add/remove/merge/apply-all, bake, erosion add, tint paint/erase, and
  the numeric/checkbox settings all undo correctly.
- Builder: Create Box, Extrude, Paint and Add preset slot each resolved as one
  `Pixel Builder: …` action and redid correctly. A save + scene reopen preserved the edited
  mesh, its transform and three material slots.
- Pixelizer: Add PixelSunShafts / MarchingSquaresWildFlowers, the Dawn preset's properties,
  outline/deck toggles, quantize mode, debug view, time-of-day entry and object outline changes
  each created one action and redid correctly. Exceptions: an integer spin-slider drag creates
  two steps (#29), the Dawn preset undo leaves a stale rendered tint (#30), and the settings
  enum/slot dropdowns create no action (#1).
- Effects: Sky coverage/shadow, the Dawn preset (one multi-property step), all six weather
  types and the Weather dropdown, water level/swell, erosion (digest 3064656926 →
  3692886391), wildflower scatter (15,434 flowers, one step), feature switches and Add
  SunShafts/Erosion/WildFlowers all undo/redo correctly. The strip sliders can leave their range
  (#28) but the drag itself is one undo step.
- Deep pass: 15 paint strokes produced 15 actions; 15 Ctrl+Z returned to the exact pre-series
  digest/version and 15 Ctrl+Shift+Z returned to the exact post-series state. Cross-tool
  ordering (Level → Paint → Paint), scene-tab switches, selecting another node, Ctrl+S, the
  Scene-menu Undo/Redo, and close/reopen (empty history) all behaved correctly.
- Slot Pattern/Scale/Color/Grass Sprite/name edits, Paint Walls on a vertical cliff, Quick
  Paint, grass paint/erase, Erase All, Fill/Clear on hex_island, Mesa/Valley/Hill stamps, the
  Gradient wheel shortcuts then a stroke, tint paint/erase, and chunk add/remove each resolved
  as one action and redid correctly (the slot panel labels lag; #9).
- Add-ons run through `EditorUndoRedoManager`; most action names are sensible (`terrain level`,
  `terrain paint`, `terrain foliage paint props`, …), with the misleading cases in #56.

Non-undoable or broken undo paths found: terrain settings enum/slot dropdowns (#1), Grass
Animation FPS (#3, where Ctrl+Z then undoes the **previous** terrain action), the stale texture
panel labels after slot/preset undo (#9), the no-op Chunk Size step (#11), integer spin-slider
drags taking two steps (#29), the stale dawn tint after a preset undo (#30), and the
Texture Name LineEdit focus trap that records a new rename on text undo (#54). No other
terrain/scene edit that changed state lacked an undo entry in the deep pass.

---

## Coverage

| Area | Pass | Evidence |
|---|---|---|
| Terrain sculpting (Brush Drag + Raise/Lower, Level, Smooth, Bridge, modifiers, toolbar, grid) | `sculpt` | `qa/screenshots/sculpt/` (93), `qa/findings/sculpt.md` |
| Painting, 16 texture slots, presets/export, Foliage, grass | `paint` | `qa/screenshots/paint/` (62), `qa/findings/paint.md` |
| Inspect, Chunks, six Settings pages, Storage save/bake/GLB, Stamp, Gradient, Select, Tint | `manage` | `qa/screenshots/manage/` (73), `qa/findings/manage.md` |
| PixelRenderer 9 pages, Palette/LUT, Object/OutlineColors, Preview/Animate, Pixel World add/presets/features | `pixel` | `qa/screenshots/pixel/` (108), `qa/findings/pixel.md` |
| Sky, Day/Night, Weather (6 types), Water, Fireflies, Birds, Smoke/Fire, Particles, LightVolume, SunShafts, Erosion, WildFlowers, feature switches | `effects` | `qa/screenshots/effects/` (112), `qa/findings/effects.md` |
| Pixel Builder (UI, modes, draw/extrude/paint/materials/UV/save, both layouts) | `builder` | `qa/screenshots/builder/` (36), `qa/findings/builder.md` |
| Runtime game (hex island, courtyard, tutorials 01–04, 3 window sizes, F1/F3, controls) | `runtime` | `qa/screenshots/runtime/` (52), `qa/findings/runtime.md` |
| UI/resizability (5 sizes, every surface, dialogs, popups) | `uiresize` | `qa/screenshots/uiresize/` (51), `qa/findings/ui.md`, `qa/findings/ui-crash.txt` |
| Deep undo/redo matrix and gaps | `undodeep` | `qa/screenshots/undodeep/` (62), `qa/findings/undodeep.md` |

649 PNGs (~960 MB) plus UI JSON dumps are stored under `qa/screenshots/<area>/`; each
area's markdown lists the exact file→finding mapping. The generated session copies, editor logs
and the project template are disposable and live under `MST_QA_STATE` (default
`/tmp/opencode/qa`).

## Testing limitations

- The editor ran on a virtual X11 display with Mesa llvmpipe at ~5–10 fps. Timing-sensitive
  drags were given 0.8–2 s; one transient two-page frame in the pixelizer was observed under
  load and was not counted as a defect until re-tested.
- Embedded popups (`OptionButton` lists, menus) do not receive injected keyboard focus on
  Xvfb; items were selected with real mouse clicks at their popup coordinates. Typing into
  popups was not exercised.
- The harness's `key` helper releases GUI focus by default (so Ctrl+Z reliably reaches the
  terrain). Focused-LineEdit undo was separately verified with the release disabled: Ctrl+Z
  then edits the text field only, as expected — and that path exposed #54.
- An open `OptionButton` popup swallows Ctrl+Z until Esc; this is normal menu behavior, but QA
  scripts must close popups before testing undo.
- `--headless` (dummy renderer) cannot render or screenshot the editor UI, so the virtual
  display is the closest headless-capable setup; it is still a real X11 window and real input.
- The 1024×600 pass is incomplete because of the crash in #0: only Brush was measured at every
  size; Foliage/Inspect/Chunks/Stamp/Gradient/Select at 1024, Edge/Vertex modes, the overwrite
  confirmation, bake/export dialogs, long OptionButton lists/submenu flipping, Tab order/focus
  rings and HiDPI/editor-scale changes were not exercised. 800×600 is clamped by Godot and was
  not tested; custom sizes use `editor_scale` in source but HiDPI was not run.
- Builder coverage was time-boxed: shape variants other than Box, Shift+C append,
  inset/bevel/edge-loop/merge/connect/fill/triangulate/duplicate/delete/grow-shrink/loop-ring,
  vertex paint, flood fill, collider/trigger, UV persistence, STL/GLB export and the History
  dock were not exercised. `qa/findings/builder.md` lists them.
- Terrain coverage gaps: hex-only Settings fields are blocked by #1; texture import round-trip
  and a true vertex-grip drag (see #55) were not exercised. The deep pass covered slot
  pattern/scale/color/grass-sprite, grass palette/Erase All, Fill/Clear on hex_island,
  Mesa/Valley/Hill stamps, Paint Walls on a vertical face and chunk add/remove.
  `qa/findings/paint.md` and `qa/findings/manage.md` list the remaining gaps.
- Effect coverage gaps: embedded ColorPicker popups did not take injected keyboard focus, so
  typing palette hex values (water shallow/deep/foam, sky deck, bolt color) was not exercised;
  waterfall cells, snow accumulation, longer lightning watches and player-distance LightVolume
  grading were not tested. `qa/findings/effects.md` lists them.
