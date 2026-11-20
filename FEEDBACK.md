# MarchingSquaresTerrain — UI Feedback

Reviewed in a real Godot 4.7.2 editor (Linux, Xvfb + Mesa llvmpipe, editor scale 1.0), using
the actual docked controls, popups and dialogs at 1600×900 and at smaller window sizes.
Functional defects are in [`ISSUES.md`](ISSUES.md); this file is about the interface itself:
layout, resizability, affordances and wording.

## The short version

The addon has a distinctive, coherent editor UI: an icon toolbar in the 3D view, a wrapping
attribute strip at the bottom, and a side panel for textures/foliage/pixel nodes. Custom sizes
are multiplied by `editor_scale` (source-verified; HiDPI was not tested), it uses the editor
theme, and the compact toolbar, the `MstFlow` wrapping strip and the two builder layouts are
genuinely good ideas.

What it lacks is a **resize story**:

1. At 1024×600, switching to Level, Smooth or Tint crashes the editor in a container layout
   loop — the most serious symptom of the layout not being bounded.
2. The side panels are hard-fixed at 250 px and cannot be widened, narrowed or collapsed.
3. The bottom strip is bottom-anchored and not scrollable; when it needs more rows it silently
   clips (Brush → Raise/Lower) or eats the 3D view (Builder → Face mode: 447 px strip, 39 px
   viewport).
4. Page-tab rows move vertically between pages, so muscle-memory clicks miss.
5. Dialogs are inconsistently resizable and the Save Preset dialog has no Cancel button.

The recommendations below are deliberately opinionated: make the **panels and strip**
resizable/collapsible, keep the **toolbar and pixel grid** fixed.

## Resizability policy (what should and should not resize)

### Should be resizable

| Surface | Why | Suggested behaviour |
|---|---|---|
| Paint / Foliage / Pixel World / builder Materials side panels | Long slot names, material names, presets and per-node rows are truncated at 250 px; at 1600 only 2.5/16 slots are visible | Drag handle or `HSplitContainer` on the panel edge; min 220 px, max 420 px (× editor scale), default 250; remember per project; collapse to a 24 px tab |
| Bottom attribute strip | It must show 4–6 wrapped rows for some tools; today the overflow is clipped or steals the viewport | Vertical splitter with a **fixed max height** (240 px and ≤35% of the 3D area), a visible scrollbar, and a collapse chevron; never let the 3D view go below 480×300 |
| Builder Face/Edge/Vertex strip | Blocker: 447 px strip / 345×39 px viewport at 1600×900, 699 px / 238×39 at 1280–1024 (ISSUES #23) | Cap at 240 px, group fields into tabs (Transform / Inset / Trim / Offsets) + scroll; default to Modeler layout on small screens |
| Settings / Pixelizer page content | Different pages have different heights; the tab row jumps with the content | Keep the tab row top-anchored and fixed; scroll or page the content below it |
| Dialogs | Save Preset has a long path label and a 558×150 default; `min_size` reports 0,0 but a 400-wide request clamps to 558 because of the label | Set an honest content-based minimum, keep them user-resizable, add Cancel, persist size per dialog |
| Main editor window | Already resizable; the layouts must adapt down to 1024×600 | Test matrix: 1600×900, 1366×768, 1280×800, 1024×600; no clipped controls at any of these |

### Should stay fixed (or collapse instead)

| Surface | Reason |
|---|---|
| Terrain/pixel toolbar buttons | It is an icon palette: 30–32 px buttons × editor scale, compact mode for icons-only. Free width would break the icon+label grid. Max 2 columns (currently 3 at 1024, stealing width), with a vertical scrollbar and auto-compact below ~1400 px. |
| Pixel-art rendering | The whole point is an integer pixel grid. `PixelRenderer`'s Pixel Size is the user control; do not smoothly scale the pixel output. |
| Confirmation dialogs (overwrite, delete) | Short, focused messages; fixed and centred is correct. Give them a minimum width and keep the buttons in a stable footer. |
| Colour pickers, popup menus | Native editor controls; fixed is fine. Ensure they flip/fit at 1024×600. |
| Row/control heights | 35 px rows × editor scale keep the dense professional look; only widths should adapt. |

### Collapse is often better than resize

- Toolbar: the existing **Collapse** button is the right idea; auto-collapse (or auto-wrap into
  a second column) when the editor height is below ~500 px.
- Side panels: a small chevron that collapses to a tab/handle would recover the viewport on
  small screens without a splitter.
- Builder: the **Modeler layout** already demonstrates the value; offer it (or auto-select it)
  at narrow widths.

## Observed layout problems

These are the interface-level symptoms behind the functional issues.

### Side panels are fixed at 250 px

`editor/ui_kit.rs` sets `PANEL_WIDTH = 250.0` (× editor scale) and the panel is added to
`SPATIAL_EDITOR_SIDE_RIGHT`; there is no splitter or width preference. At 1024 px wide, the
terrain toolbar (236 px) + 250 px panel leaves roughly half the window for the 3D view. The
texture panel's content is also ~5,800 px tall (16 slots with several rows each), so it is a
long scroll with no grouping/collapse. Recommendation: resizable panel with per-slot
collapsible groups, plus a "collapse panel" control.

The panel also ignores the editor's bottom status bar: at 1600×900 the Pixel World
**Lighting Preset** row (Dawn/Noon/Dusk/Night/Storm/Snow/Wire) sits at y = 848–917 and is
covered by the status bar; only a 1-px sliver of Night is reachable until the panel is
scrolled (ISSUES #37). Primary actions should be pinned above the fold or the panel should
reserve the status-bar height at the bottom.

### The bottom strip has no overflow story

`strip_root` disables both scroll modes and relies on `MstFlow` to wrap. That works when the
editor gives the strip the height it asks for, but:

- Brush → Raise/Lower asks for 5 rows and gets 140 px; the title and Brush Type row are clipped
  above the viewport (ISSUES #5). Wheel does not scroll; there is no scrollbar.
- Builder Face mode asks for 447 px; the editor gives it 447 px and the 3D view collapses to
  345×39 px (ISSUES #23). At 1280×800 the strip is also cut behind the Materials panel
  (ISSUES #24).
- The same mechanism pushes the Settings/Pixelizer tab rows up and down between pages
  (ISSUES #20, #33), so the tab you just clicked is not where the next one is.

Recommendation: a strip container with `vertical_scroll_mode = AUTO`, a min/max height, and a
top-anchored tab row that does not move. Cap the strip so the 3D view keeps a usable minimum.

### Status text and hints clip

The brush selection status is cut at the right edge (`... RMB/Esc: c...`) and some hints
advertise controls that do not exist (Level's Strength) or omit working ones (Raise/Lower's
Ctrl+wheel strength). Recommendation: wrap the hint to two lines (or elide with a tooltip) and
generate it from the same table as the controls.

### Dialogs

- **Save Preset**: 558×150, resizable in principle, `min_size = 0,0`, only an OK button. A
  400×150 request is clamped to **558×150** by the one-line path label (540 px), while 800×220
  is accepted and leaves a large blank area. Cancelling requires Esc or the window X. Add
  Cancel, wrap/shorten the label so it can be ~420 px wide, and remember the size.
- **Overwrite confirmation**: has Cancel/OK and is fine; keep it fixed.
- **Import file**: native editor dialog; fits at its 819×480 default. It reports `min 200×70`
  but a 400×200 request clamps to **498×200** (content minimum) — harmless, but the reported
  minimum should match the real one.
- **Empty preset name** closes silently and writes nothing (ISSUES #18) — keep the dialog open
  with inline validation.

### Runtime overlays

- **F1 help** is a single ~10 px grey line pinned to the bottom edge, drawn directly over bright
  grass, with no backing panel (ISSUES #50). Give it a translucent rounded panel/backdrop, a
  readable type size (≥14–16 px at 720p), multi-line grouping, and a short fade-in. The current
  line is easy to miss and its contrast depends on the terrain behind it.
- **F3 Pixel tuner** works and is legible at 1280/1920, but at 1024×600 it starts at x≈604 and
  its weather section is below the window (ISSUES #51). Make the panel responsive: cap it at
  ~40% of the width, collapse sections (Effects/Renderer/Scene) with the active tab remembered,
  keep a visible scrollbar, and allow the whole tuner to be hidden with one more F3.
- The runtime window scaling itself is good: the pixel art stays crisp (nearest-neighbour) at
  1920×1080, 1280×720 and 1024×600, with no in-window letterboxing. Keep the integer pixel
  scaling; only the overlays need the responsive treatment.

### Menus and panels lose their place

- Pixel World's disabled `+ Add` entries are as bright as enabled ones (ISSUES #34); only the
  icon dims. Use the theme's disabled text colour.
- The Pixel World panel keeps its scroll offset when its rows rebuild after a section switch,
  hiding the top rows; the scrollbar is only visible on hover (ISSUES #35). Reset to the top
  or scroll the selected row into view.
- Pixelizer page tabs move vertically between pages (ISSUES #33); Settings pages too (#20).
- After an undo/redo of a texture slot edit, the Paint panel keeps showing the edited Pattern,
  Color swatch, Grass Sprite, Scale, slot names and preset even though the data reverted
  (ISSUES #9). The panel should refresh on history changes; otherwise undo looks broken.

## Measured small-screen behaviour

The editor's hard minimum is **1024×600**; an 800×600 request is clamped to 1024×600 (do not
force it). The measured 3D viewport (and strip) at each size:

| Surface | 1920×1080 | 1600×900 | 1366×768 | 1280×800 | 1024×600 |
|---|---|---|---|---|---|
| Brush viewport / strip | 574 / 105 px | 326 / 140 px | 159 / 175 px | 121 / 245 px | **39 / 245 px** |
| Builder Face viewport / strip | n/t | **39 / 447 px** | n/t | **39 / 699 px** | **39 / 699 px** |
| Paint sidebar | n/t | 250 px fixed, 2.5/16 slots | n/t | n/t | 250 px over a 39 px viewport |
| Pixelizer viewport | n/t | 662×320 | n/t | n/t | **670×39**, panel 17 px on screen |
| Toolbar | 1 col | 2 cols | 2 cols | 2 cols | 2–3 cols (~236–320 px) |

Consequences, all confirmed in `qa/findings/ui.md`:

- **The editor crashes** when Level, Smooth or Tint is active/selected at 1024×600
  (`Container::_sort_children` / "Message queue out of memory", reproduced 5×). This is the
  single highest-priority defect; 1024×600 is unsupported for those tools until it is fixed,
  and a saved 1024 layout can make the next launch crash immediately.
- Brush is unusable at 1024 (39 px viewport); 1280 is marginal (121 px); even 1366 gives only
  159 px.
- The Paint/Foliage sidebar shows 2.5 of 16 slots at 1600 with no visible scrollbar; the Export
  action is ~100 wheel ticks away.
- The Pixel World panel is 17 px on screen at 1024; the Select/Add buttons are unreachable.
- Settings tabs fit at 1024 but the fields below ~9 rows clip, the hint is cut/overlapped, and
  short pages do not give the viewport its space back.
- The toolbar grows to 3 columns at 1024, taking 80–200 px of viewport width.
- Builder Preferences is 484×664, non-resizable, with its action row 61 px off-screen at 600.
- The Modeler layout at 1024 loses the bottom strip and Materials palette.
- At 1024 the editor's own Inspector collapses to ~126 px (Godot-level), so addon layouts cannot
  rely on Inspector fallbacks at that size.

Minimum target I would enforce: **3D viewport ≥ 480×300** (ideally 640×360). When a layout would
violate it, auto-collapse in this order: side panel → tab, strip → 2 rows + "More", toolbar →
icon rail.

## Affordances and interaction

- **Tool shortcuts are advertised but dead.** Tooltips and docs promise `1`–`9` for tools and
  `1`–`4` for builder modes, but Godot's 3D view shortcuts consume the digits
  (ISSUES #4). Either register real editor shortcuts (`InputMap`/`Shortcut`) or remove the key
  hints. `H` works but cycles backwards (#26).
- **Esc is unreliable.** It does not clear the brush selection (#7) even though the hint and
  docs say it does. Esc does close dialogs and the UV editor.
- **Focus rings and Tab order** are mostly the editor defaults; the icon toolbar has
  `FocusMode.NONE`, which is fine for a palette, but the strip controls should keep a visible
  focus ring for keyboard users.
- **Disabled state** is inconsistent: Grid Aligned looks enabled but refuses to toggle (#13);
  Grid Size stays enabled with alignment off (#14); Pixel World menu entries do not dim (#34).
- **Slider trust**: spin sliders accept out-of-range drags by orders of magnitude (#8, #28):
  Slope Min reached 8287.5°, Pixel Size 44243, Darken 290.19, Sky Coverage 377.4 and Water
  Swell 142.85 in single drags. An integer drag also creates two undo steps (#29). Users cannot
  tell the difference between a typo-safe clamp and a broken field, and the Inspector then
  disagrees with the strip (#41). Clamp drags to the declared range; keep "type a larger
  value" as an explicit opt-in.
- **Adding effects closes the tool.** `+ Add` selects the new node, so the pixel toolbar, strip
  and Pixel World panel disappear (#31). Adding two effects in a row takes two reselections.
  Return focus to the renderer/terrain, or keep the panel pinned.
- **New props are not selected.** Drawing a sibling Builder shape leaves the selection on the
  old mesh (#25), so the next `B` paints the wrong object. Select the new node and say so in
  the HUD.
- **Undo focus trap.** Ctrl+Z with the Texture Name field focused edits the text; because the
  field commits on every change, the text undo records a **new** rename action and the redo
  stack is lost. Clicking the 3D view does not release the field — only clicking another control
  does (#54). Release focus on a viewport click (or commit only on Enter/focus loss).
- **Vertex Handles look like grips but are chunk add/remove targets.** Dragging a "vertex"
  handle removed or added a chunk in the deep pass (#55). If true vertex grips are not
  available for a grid, hide the toggle or label the handles explicitly; the current hint only
  mentions chunk add/remove.
- **History names should describe the action.** Scale is logged as `terrain texture 1 magnify`,
  tint erase as `terrain tint paint`, and Quick Paint strokes lose the tool name (#56). History
  is the user's undo safety net; make the names match the tools.

## Defaults and first-run impression

Several effects look "broken" at their shipping defaults, which is a UI problem even where the
implementation is fine:

- `+ Add` Sun Shafts leaves the Sun empty and the node inert (#32); the only shafts on screen
  are the renderer's God Rays, so the user has no way to tell the add worked.
- Fireflies at count 48 / size 0.12 are invisible from the demo's ~30 m camera (#39); the
  effect only appears at 0.6 / 200.
- The Smoke node shows a flame pixel but no plume in hex_island at rate 2.5–5 (#36).
- `+ Add` selects the new node and closes the whole pixel UI (#31), so trying a second effect
  requires reselecting the renderer.
- The Dawn preset renders magenta rather than a warm dawn (#38), which reads as a broken
  preset rather than a stylistic choice.

Recommendation: ship "demo-visible" defaults (or a visible fallback/marker), wire the Sun on
add, and keep the Pixel World panel open until the user closes it. First-run trust matters more
than matching the original plugin's numbers.

## What to keep

- The **compact/Collapse** toolbar mode: exactly the right way to trade labels for viewport.
- `MstFlow`'s balanced wrapping: the strip looks deliberate when it fits; keep it, add
  overflow handling.
- The **two builder layouts** (MST and Modeler): a strong answer to different screen sizes and
  workflows.
- The **runtime pixel scaling**: crisp nearest-neighbour output at 1920×1080, 1280×720 and
  1024×600 with no in-window letterboxing. Only the runtime overlays (F1/F3) need responsive
  treatment.
- Sizes go through `editor_scale` in source (the 1.0 pass cannot confirm HiDPI): keep that
  foundation rather than hard-coding pixels.
- Section/page tabs on the pixelizer and the six Settings pages: good grouping; just anchor the
  tab row.
- The right-side panel's scrollbar gutter and the editor-theme styling: consistent with Godot.

## Priority list

1. **P0 — Fix the 1024×600 crash** with Level/Smooth/Tint (container layout loop). Until then
   the addon cannot claim support for its own minimum window.
2. **P0 — Stop the strip from eating the viewport**: fixed max height + visible scroll +
   collapse, with a 480×300 viewport floor. Fixes Brush at 1280/1024 and Builder Face at every
   size.
3. **P0 — Give side panels a width control** (splitter or collapse, min/max, remembered); this
   also gives the Paint panel a usable visible scrollbar.
4. **P1 — Anchor page tabs** and scroll the page content (Settings, Pixelizer); keep the Pixel
   World panel on screen at 1024.
5. **P1 — Fix dialog affordances**: Cancel on Save Preset, min size/wrap, Preferences resizable
   and scrollable, validation on empty name.
6. **P1 — Make shortcuts real or remove the hints**; fix Esc; fix out-of-range spin sliders.
7. **P1 — Runtime overlays**: readable F1 help with a backing panel; responsive F3 tuner
   (collapsible sections, visible scrollbar) at 1024×600.
8. **P2 — Feedback polish**: disabled-state contrast, scrollbar visibility/scroll retention,
   wrap hints, select newly added nodes, avoid the blocking Modeler toast, pin the Lighting
   Preset row above the status bar, and keep the Inspector in sync with strip edits.

## Suggested verification (regression matrix)

For each surface, capture 1600×900, 1280×800 and 1024×600 and assert:

- every visible control's `rect` is inside its scroll viewport (`clipped == false`),
- no horizontal scrollbar appears in the strip or panels,
- the 3D viewport is at least 480×300 (or the strip scrolls/collapses),
- the tab row's y position is identical across pages,
- the panel can be widened/narrowed and remembers the width,
- dialogs cannot be shrunk below their content and keep their action buttons visible,
- all advertised shortcuts fire (or are not advertised).

The harness in [`qa/`](qa/) supports this directly (`resize`, `ui --root all`, `capture`,
`windows`, `window-resize`, `popups`); the per-surface evidence for this pass is in
`qa/screenshots/uiresize/` and `qa/findings/ui.md`.
