# MarchingSquaresTerrain — UI feedback

Round 3 (r4) review, 2026-11-21. Godot **4.7.2** on Linux, Mesa **llvmpipe**, editor scale 1.0,
real X11 input. Measured in the live editor at **1920×1080, 1600×900, 1366×768, 1280×720 and
1024×600**, and in the runtime at 1280×720, 1024×600, 800×480 and 1600×900. Functional defects are
in [`ISSUES.md`](ISSUES.md); this file is about the interface: layout, resizability, affordances
and wording. Evidence is in `qa/screenshots/r4-uiresize/`, `r4-paint/`, `r4-pixel/`, `r4-builder/`
and `r4-runtime/`; measurements are in `qa/findings/r4-uiresize.md`.

## The short version

The addon has a coherent, distinctive editor UI: an icon toolbar in the 3D view, a wrapping
attribute strip at the bottom, and a 250 px side panel for textures/foliage/pixel nodes. The
compact toolbar, the `MstFlow` wrapping, the two builder layouts and the runtime pixel scaling are
good ideas and should be kept.

What it still lacks is a **resize story**:

1. **There is no protected 3D view.** At 1600 and below, the strip and toolbar take whatever height
   they want; the viewport falls to 291 px (Settings), 256 px (Paint), 124 px (Brush at 1366), and
   **39 px** at 1280/1024. At 39 px the tested brush drag produced no terrain change (issue 7), so
   the 1024 layout is effectively read-only in practice.
2. **The side panels are fixed at 250 px** with no splitter and no collapse. Paint's content is
   **5,767 px** tall — about 90 wheel notches to the export button, with only ~1.5–2 of 16 slots visible
   (issue 16). Foliage's palette is not a Control, so it cannot even be tabbed to or scrolled
   normally.
3. **Two resize paths crash the editor.** Selecting `PixelRenderer` at 1280×720 loops the
   container layout and exits (issue 1, independently re-reproduced); selecting a
   `PixelBuilderMesh` at 1366×768 while the pointer is over the builder toolbar panics the plugin
   in the UI pass (issue 2; hover/timing dependent, not re-reproduced on two fresh attempts). These
   are data-loss-grade and outrank every cosmetic item.
4. **The toolbar changes shape with the window**, from a 116 px single column to a 236 px
   two-column block to a 76/116 px compact block, so tools move while the user works (issue 17).
5. **Settings tabs move vertically** (y≈287 → 518), and the Grid page grows the strip to 385 px,
   which is what collapses the view at 1280 (issue 18).

The user preference — "resizable, but not everything needs to be resizable" — is the right one.
The recommendation below is deliberately selective: make the **viewport, strip, side panels and
dialogs** resizable/collapsible; keep the **icon rail, row heights, popup geometry and pixel grid**
fixed.

## Measured facts

`view` = 3D viewport size; `strip` = attribute strip height; `panel` = side panel width. The
Output dock was open (224 px) for every editor measurement; closing it adds height, not width.
Docks auto-shrink at 1366/1280/1024, so those widths are Godot-level, not addon-level.

| Surface | 1920×1080 | 1600×900 | 1366×768 | 1280×720 | 1024×600 |
|---|---|---|---|---|---|
| Terrain toolbar, expanded | 116×682, 1 col | 236×469, 2 col | 236×337, 2 col | 236×289, 2 col | 236×287, 2 col |
| Terrain toolbar, compact | 36×682, 1 col | n/t | n/t | n/t | 76×392 (2 col) / 116×252 (3 col) |
| Brush | view 1227×574; strip 105 | view 787×361; strip 105 | view 553×124; strip 210 | view **470×41**; strip 245 | view **470×39**; strip 245 |
| Paint | view 974×574; strip 105; panel 250 | view **534×256**; strip 210 | view **300×39**; strip 385 | view **292×39**; strip 385 | view **377×39**; strip 350 |
| Foliage | view 974×609; panel 250 | view 534×361; panel 250 | n/t | n/t | n/t |
| Settings — Grid | strip 140 | view 787×291; strip 175 | view 553×124; strip 210 | view **470×39**; strip 385 | view 630×39; strip 350 |
| Pixelizer (Renderer) | view 1055×574; strip min 670 | view 696×293; strip min 670 | view 670×161 (fits by 0 px) | **crash** | n/t |
| Builder — Object | view 871×617 | view 421×310 | **crash** | n/t | n/t |
| Builder — Face | view 795×502 | view **345×39**; strip 447 | n/t | n/t | n/t |
| Save Preset | 558×150; 400 request clamps to 558; reopen 558 | n/t | n/t | n/t | n/t |

Other measured facts:

- **Viewport floor:** the last pass used 480×300; this pass found **39 px** at 1024/1280 and
  345×39 for Builder Face at 1600. Every size below 1920 violates the 480×300 floor for at least
  one surface.
- **Paint panel:** 250×682, content **5,767 px**; only ~1.5–2 texture slots fit; 90 wheel notches
  to the bottom action. The vertical scrollbar exists but is 14 px and easy to miss.
- **Builder Materials:** 245–259 px wide, 659 px content in a 469–682 px view; bottom slots need
  scrolling.
- **Pixel World:** 250 px wide, 941 px content; at 1024 the panel is placed at x≈968 (56 px
  visible), and the Lighting Preset row is below the panel viewport.
- **Runtime F3:** fixed ~420 px wide; at 1024 starts at x≈604 and clips its lower weather fields;
  at 800×480 covers the player. F1 is a single ~15 px line with no backing panel.
- **Runtime pixel scaling works:** 1280×720 → 1024×600 → 800×480 → 1600×900 → 1280×720 kept a
  crisp 3×3 pixel grid and reflowed the low-res buffer correctly (430×242 → 344×202 → 270×162 →
  536×302). HUD text stayed sharp. This is a genuine strength.
- **All UI constants scale:** `TOOL_BUTTON_SIZE = 30`, `ROW_HEIGHT = 35`, `PANEL_WIDTH = 250`,
  `FORM_LABEL_WIDTH = 74` are multiplied by `EditorInterface.get_editor_scale()` through
  `ui_kit::sz`/`px`; a 2× editor scale would make the 250 px panel 500 px and amplify every layout
  failure. HiDPI was not measured (scale 1.0 only).
- **Hit targets:** toolbar buttons are 116×32 expanded and 36×32 collapsed; Pixelizer rail 30×30;
  Builder mode 34×34; strip rows 35 px; panel labels 74 px. Mouse-sized, not touch-sized; fine for
  a desktop editor, but the 30 px icons are at the low end.

## Resizability policy (opinion)

This is the explicit answer to "resizable, but not everything needs to be":

| Surface | Verdict | Min / default / max | Behaviour |
|---|---|---|---|
| **3D viewport** | **protected** | **≥480×300**, ideally 640×360 | Takes remaining space; layout must auto-collapse before it falls below the floor |
| **Attribute strip** | **resizable** | 35 / 105 / 175 px, and ≤35% of view height | Vertical splitter; scroll inside; chevron to collapse to the title row |
| **Settings page content** | **not strip-growing** | 1–3 rows, then internal scroll or sub-tabs | Page tabs stay on row 1; the title goes top-left |
| **Terrain toolbar** | **fixed rail** | 30–36 px wide; vertical scroll | Labels at ≥1400 px; Collapse = 36 px icon rail, not 76/116 |
| **Paint / Foliage / Pixel World / Materials** | **resizable + collapsible** | 220 / 250 / 420 px | Horizontal splitter; collapse to a 24 px tab; visible scrollbar and fast wheel |
| **Builder Face/Edge/Vertex strip** | **resizable + grouped** | 3 rows + scroll, max 240 px | Section tabs/popovers instead of one 447 px block |
| **Save Preset** | **resizable** | 420×150 / 420×150 / 900×260 | OK **and Cancel**; remember last size |
| **Runtime F3 tuner** | **resizable + collapsible** | fit the window, scroll | Collapse/fit button; do not cover the player at 800×480 |
| **Runtime F1 help** | **readable overlay** | multi-line, backing panel | Toggle, not one-shot; do not collide with F3 |
| **Popups** | **fixed** | viewport-clamped | Flip/scroll beyond ~12 rows; no need for user resizing |
| **Toolbar icons / row heights / pixel grid** | **fixed** | 30 px icon, 35 px row | Scale with `editor_scale`; no user resizing needed |
| **File dialogs** | **leave to Godot** | Godot defaults | Fix only the reported minimum (issue 40) |
| **Preferences** | **resizable + scroll** | 480×400 / 484×600 | Clamp to screen−80; remember size |

The non-negotiable rule: **collapse in this order before the viewport drops below 480×300** —
panel → tab, strip → scroll, toolbar → rail. Today every size below 1920 violates it, and 1600
violates it for Paint (534×256), Settings (291) and Pixelizer (293); Builder Face violates it even
at 1600 (345×39).

## Surface-by-surface

### 1. The 3D viewport and the bottom strip (issues 7, 8)

The strip is bottom-anchored and grows upward with its flow content. At 1600 the Paint strip wraps
to six rows because the expanded toolbar leaves only 534 px; at 1366/1280/1024 the view is a
39–124 px slit. This is not label clipping — the content fits, the **height** is unbounded. The
same problem is worse in Builder Face mode: a 447 px strip with a 39 px view even at 1600.

**Fix:** a real `VSplitContainer` with a 175 px cap, internal `ScrollContainer`, and a chevron to
collapse the strip to its title row. Group Builder Face/Edge/Vertex fields into tabs/popovers
(max 240 px). Protect the view at 480×300.

### 2. The terrain toolbar (issue 17)

The toolbar is 116 px at 1920 (one column), 236 px at 1600/1366/1280 (two columns), and 76 or
116 px compact at 1024 depending on the strip height. The column count changes while the user
works, so tool positions move; at 1600 the toolbar takes 15% of the width and forces the Paint
strip to wrap.

**Fix:** one fixed 30–36 px icon rail with vertical scrolling; labels only at wide sizes; compact
mode always 36 px. Do not derive the column count from the available height.

### 3. The side panels (issue 16)

Paint/Foliage/Pixel World/Materials are all fixed at 250 px. Paint's 5,767 px content makes the
panel an eight-to-twelve-screen scroll; the export button is 90 wheel notches away. The panel can
only grow the viewport, never the content area. Foliage's palette is not exposed as Controls at
all, which breaks keyboard navigation and the control dump.

**Fix:** a horizontal splitter (220–420 px), a 24 px collapse tab, a visible scrollbar, and fast
wheel scrolling. Make the Foliage palette a real `Control` tree. Start at 250 px, remember per
project.

### 4. Settings and tabs (issues 18, 25)

The Settings tab row moves from y≈287 to y≈518 between pages because the strip height changes, and
the Grid page alone grows to 350–385 px. Fixed-position clicks land in the 3D view; at one point
`click-text "Grid"` hit the toolbar Grid overlay instead of the tab. Clicking the 3D view while
Settings is open deselects the terrain and closes the whole tool.

**Fix:** keep the page tabs on a stable row (row 1), put the tool title top-left, scroll page
content inside a three-row strip, and do not let a viewport click without a tool action clear the
selection.

### 5. Pixelizer and Pixel World (issues 1, 4, 24, 33)

The Pixelizer strip has a hard 670 px minimum. At 1366 it fits by 0 px; at 1280 the layout loop
crashes the editor. At 1024 the Pixel World panel is placed off-screen (x≈968), and “+ Add” closes
the pixelizer UI instead of keeping the panel open. The `on_float` re-entrant borrow panic is
intermittent but from the same family.

**Fix:** make the strip scroll/overflow below 670 px instead of looping; clamp the panel to the
window and keep it on-screen; keep the panel open after Add; make the strip rebuild re-entrancy-safe.

### 6. Builder (issues 2, 8, 34, 42)

Face mode is unusable at 1600 (345×39 view, 447 px strip, 416 px toolbar). The Modeler “Created
Box” toast covers the viewport at 1280×800. The documented Shift+C append path opens Bezier, and
the digit/H/E/F shortcuts collide with Godot's 3D editor. The crash at 1366 is the same
re-entrant-borrow class as the Pixelizer panic.

**Fix:** group the Face/Edge/Vertex fields, cap the strip, keep the toast out of the viewport, and
audit shortcut collisions (`H` = hide node, `E` = rotate mode, digits = view orientation).

### 7. Dialogs (issues 28, 40)

Save Preset opens at 558×150, refuses to shrink to 400×150 despite reporting `min [0,0]`, accepts
800×220 with blank space, and forgets the size. It has only OK; Esc/X close it. Empty names close
silently. The Import dialog reports `min [200,70]` but clamps to 498×200.

**Fix:** a real Cancel button, validation for empty names, an honest minimum, and remember the
last size. File dialogs can stay Godot-standard once the reported minimum is honest.

### 8. Runtime HUD (issue 39)

F3 is fixed at ~420 px; at 1024 it starts at x≈604 and clips the lower weather fields, and at
800×480 it covers the player. F1 is one small unbacked line that is hard to read over bright sand
and is cut by F3. F1 is one-shot, not a toggle. The runtime pixel scaling itself is excellent.

**Fix:** make F3 fit/scroll/collapse; give F1 a backing panel and a toggle; keep both out of each
other's way at 1024.

### 9. State and affordances (issues 13, 14, 22, 26, 32, 41)

The UI often lags the data: undo restores a texture preset but the labels keep the old name; Grid
Type undo leaves the strip saying `Square`; Builder undo leaves stale vertex/face counts and a
stale selected face; Shift/Ctrl-wheel changes Size/Strength but the labels do not move; Esc clears
the selection but the HUD still says “175 selected”. Pixel Preview/Animate have no pressed state.
Grid Aligned looks enabled but silently refuses with Falloff on. These are individually small but
together they make the editor feel unreliable.

**Fix:** refresh the owning panel/HUD from the same signal that changes the value; give toggles a
pressed state; disable Grid Aligned when Falloff makes it unavailable; clear the selection HUD on
Esc.

### 10. Shortcuts (issues 9, 42)

The tooltips and docs advertise `1`–`9` for sculpt tools, `1`–`4`/`H` for builder modes, `E` for
extrude and `F` for frame. None of them work: the 3D view consumes digits, `H` hides the node, and
`E`/`F` are built-in editor shortcuts. `Shift+C` opens Bezier instead of appending. This is a
documentation/implementation split, not just a UI nit — a user following the cheat sheet cannot
use the advertised workflow.

**Fix:** claim the shortcuts in the viewport input handler before the 3D editor, or change the
documented keys and tooltips to ones that do not collide. Audit `H/E/F/R/G/1–4` as a set.

## Priority fixes

Ordered by risk to the user's work, then by how many surfaces one change repairs:

1. **Stop the two crash paths** (issues 1, 2; same family as 4). The editor exits and unsaved work
   is lost. Fix the Pixelizer 670 px minimum/overflow loop and the builder `on_button_hover`
   re-entrancy before any cosmetic work.
2. **Protect the 3D view** (issues 7, 8). Cap the strip at ~175 px with a real scroll, add a
   collapse chevron, and group the Builder Face/Edge/Vertex fields. This one change restores
   1280/1024 and Paint/Settings at 1366.
3. **Side-panel splitter + collapse + fast scroll** (issue 16). Paint's 5,767 px content and 90
   wheel notches is the worst example; Foliage needs to become a real Control.
4. **One fixed icon rail** (issue 17). Stop the 1/2/3-column jumps; compact must be 36 px.
5. **Stable Settings tabs + page scroll** (issues 18, 25). Keep tabs on row 1; stop the viewport
   click from closing the tool.
6. **Dialog consistency** (issues 28, 40): Cancel, validation, honest minimum, remembered size.
7. **Runtime HUD fit** (issue 39): F3 fit/scroll/collapse; F1 backing + toggle.
8. **State refresh + pressed states** (issues 13, 14, 22, 26, 32, 41): make every panel reflect the
   data after undo and after wheel/slider changes.
9. **Shortcut audit** (issues 9, 42): claim or re-document the keys that collide with the 3D editor.

## What to keep

- The **compact toolbar** idea (icons-only with tooltips) is good; it just needs a fixed 36 px rail.
- **`MstFlow` wrapping** is the right model for the bottom strip; it should wrap inside a capped,
  scrollable region, not grow the whole panel.
- The **two builder layouts** (MST and Modeler) are a sensible answer to different workflows; the
  Modeler layout is what made Face mode usable at 1600.
- **Runtime pixel scaling** is genuinely good: 1280→1024→800→1600→1280 kept a crisp pixel grid,
  correct buffer reflow and sharp HUD text.
- **Tooltips are informative** (the Brush and Settings tooltips are unusually complete), and the
  `editor_scale` plumbing is already in place for HiDPI.
- **Undo itself is in good shape** for terrain tools: 35 of 42 matrix rows pass, including a
  19-chunk stroke and all the context/focus cases. The UI refresh around undo is the weak part,
  not the edit history.

## Method and limitations

Measurements come from live `ui` dumps and screenshots taken immediately after each resize; the
full dumps for the three 1920 states are in `qa/screenshots/r4-uiresize/ui/`, and the rest are
filtered dumps recorded in `qa/findings/r4-uiresize.md`. `resize` is clamped to Godot's 1024×600
minimum (800×600 was not forced). All numbers include the 224 px Output dock; closing it adds
height but does not change any width result. HiDPI, Tab order/focus rings, and Pixelizer/Builder at
1024 were not measured (the latter because of the crashes). The runtime resize used live X11
`ConfigureWindow` because `drive.py resize` is harness-only.

---

## r5 pass — what the fixed build feels like (2026-11-21)

Measured on the post-fix build by five QA drivers; functional defects are in
[`ISSUES.md`](ISSUES.md) N1–N31, evidence in `qa/findings/r5-*.md` and `qa/screenshots/r5-*`.
This section is the interface verdict.

### Kept / now working

- **The resize story mostly exists now.** The toolbar is a fixed single-column rail
  (120 px labelled, 40 px compact, never a 1/2/3-column jump). Attribute strips are capped
  at 175 px with a visible vertical scrollbar and one row per wheel notch; nothing spills
  below the window. Settings pages no longer jump their tab row. Side panels have a 220–420 px
  grip and a 24 px collapse tab. The Save Preset dialog is 420×179 with OK **and** Cancel, a
  live path label and an inline empty-name error.
- **The 3D view no longer collapses to 39 px.** Measured view height: 1920 → 734–874,
  1600 → 554–694, 1366 → 389–529, 1280 → 341–446, 1024 → 297.
- **Runtime pixel scaling stays excellent** at 1280/1024/1600, and the sea now reaches the
  horizon in the tutorials.

### The remaining resize gap (updated after Round 2)

1. **The width axis now has a floor.** A side panel collapses to its tab before the 3D view drops
   below 480 px: Paint at 1280 goes from a 333 px view to 551 px, and 1600 re-expands it. The
   builder palette keeps its own 277 px default and still needs the same floor (N32).
2. **Strip height is now a maximum, not a target.** Settings pages shrink to their content
   (Storage 107 px), pinned strips keep a high-water height per tool, and the title/tab header
   stays pinned while the body scrolls.
3. **Pixelizer first paint is still broken (N1).** On a fresh session, the *first* selection of a
   `Pixel*` node leaves the rail 39×0, the strip 0×35 and the panel 250×0. Selecting it a second
   time (or a 2D→3D round-trip) lays everything out correctly, so it is a missing first-frame
   relayout. Nothing is logged, so it reads as “the addon has no pixel UI”.
4. **The page-tab grid is now width-aware.** Columns are picked from the strip width (two rows at
   narrow sizes); at 1024 the header becomes a normal scrolling row instead of covering the strip.
5. **Panels and dialogs remember their state.** Panel width/collapse is written through to
   `res://.godot/editor/mst_editor_state.cfg`; Save Preset reports an honest 340×178 minimum,
   honours a 400×150 request as 400×179, and remembers its size.

### Surface notes

- **Paint / Foliage:** the panel is 250 px with **5,764 px** of content (~8 screens; 1.7 of 16
  slots visible). Collapse exists; a 2-column slot grid or a denser slot row would fix the
  scroll marathon more than a splitter does. Foliage Fill Terrain still paints nothing for Tree
  yet records an undo step.
- **Settings:** the Grid page no longer clips Blend Mode: the title and page tabs are pinned and
  the body scrolls. The Settings “Grid” tab is now **“Layout”**, so it no longer collides with
  the toolbar Grid overlay.
- **Chunks:** the status line is clipped, the green `+` handles are near-invisible on bright
  terrain, and add/remove read three different ways (hint, tooltip, status). Show “N selected”.
- **Storage:** Export GLB has no path field and silently wrote 32 MB; Save Data is always
  enabled and reports only to Output. Give both a row and a toast, and gate Save on dirty.
- **Import dialog:** the honest 500×300 minimum is cramped (2-row list, elided filter text,
  dead Favorites/Recent); 700×420 or hide the dead rows.
- **Builder:** the viewport is the squeezed surface at ≤1366 (86 px rail + 245–277 px palette
  before editor docks). The Materials palette now has the terrain panel's grip and collapse
  (657→910 px at 1600×900) but still expands to 277 px by default and does not auto-collapse, so
  at 1280×800 the view is 340 px. The builder rail still has no collapse. Element modes now show
  one section at a time (Extrude / Ops / Move / Paint), so the Face strip is 62–132 px instead of
  194–428 px. The HUD's leak and the badge overlap are fixed; `F` still only re-centres (Godot's
  command, no zoom), so tooltips/docs should stop calling it “frame”. `Ctrl+D` no longer runs
  Godot's Duplicate Node.
- **Sculpt:** hints and status still do not wrap (first line clipped at 1280/1024) and the
  status sits on the strip edge at 1600. `MstValueLabel` drifts from the slider after wheel
  edits. Grid Aligned stays checked-but-disabled after a hex→square switch.
- **Pixel World:** the panel is 250 px with ~940 px of content; at 1024 it is 24 px wide and
  the Inspector ~136 px. Keep it a 32 px rail/tab that expands, and give the Inspector a
  260 px minimum.
- **Runtime:** F3 clips at 1024×600/800×480 and cannot be collapsed; F1 is a single unbacked
  line that F3 overdraws. The F1 *toggle* now works. Give F3 a fit/scroll/collapse and F1 a
  backing panel.

### One-line verdict

Round 2 landed the **width floor for side panels**, made strip caps a true maximum with pinned
tabs, made page tabs width-aware, and gave panels/dialogs remembered state. The remaining
interface work is: the **Pixelizer's zero-size first paint** (N1 — one extra selection fixes it,
so it is a first-frame relayout), the builder palette's 277 px default / missing auto-collapse
(N32), and the smaller Chunks/Storage wording and feedback items.

### r5 remediation status

Round 2 (the width contract, strip-cap-as-maximum, Pixelizer first paint, builder palette,
effects) mostly did not run: the Claude provider hit its weekly limit on 2026-11-21. Two
interface changes landed:

- `Ctrl+D` in the builder no longer runs Godot's *Duplicate Node(s)*.
- The runtime HUD is now right: F3 has a hard width cap (307 px at 1024, 260 px at 800), scrolls
  both ways and collapses by clicking its title; F1 is a dark backed panel that stays clear of
  the tuner and of an expanded object inspector.

Note: the r5 runtime observations came from a stale `qa/runtime.sh` project copy (it reuses
`$MST_QA_STATE/runtime/project`), so the F1/F3 and sea/orbit items should be read against the
current build. **N1 (Pixelizer UI zero-size on first selection)** is still the first thing to
fix.

Round 2 also landed the effects work: smoke now reads as a puff at the hearth, dawn/dusk keep the
island green and the water distinct (fog was scaled to camera distance), the water rim fades on a
circle in sRGB so no straight edge shows, and SunShafts warns when the camera is outside the
sun's shadow range or nothing occludes. The boot log noise (N6/N31) is still open and points at
the editor-layout work rather than the effects.

---

## r6 pass — interface verdict on the Round-2 build (2026-11-21)

Measured on `fix/r4-all` @ `e50088d` by four drivers; functional defects are R6-1…R6-18 in
`ISSUES.md`, evidence in `qa/findings/r6-*.md`.

### Now working

- **The resize story is real.** Side panels collapse before the view drops below 480 px (Paint at
  1280: 333 → 551 px), strips cap at their content (Storage 107 px) with pinned title/tab
  headers, page tabs are width-aware (two rows, no sliding), panel width/collapse persists to
  `res://.godot/editor/mst_editor_state.cfg`, and Save Preset is a proper 420×179 dialog with
  Cancel, an honest 340×178 minimum and remembered size.
- **The Pixelizer exists on first selection.** Rail 39×732, strip 731×175, panel 250×732 — no
  more reselect ritual. The remaining wart is a 2–3 s empty window while it builds.
- **The runtime HUD is right.** F3 caps at 308 px @1024 / 261 px @800, scrolls and collapses by
  clicking its title; F1 is a dark backed panel that does not collide with F3.
- **The builder strip is grouped.** One section at a time (Extrude/Ops/Move/Paint), 62–132 px at
  1600 instead of 194–428; dialogs have a real 500×300 minimum; Ctrl+D duplicates faces.
- **Runtime pixel scaling stays excellent**, and the game log is clean.

### What still needs a pass

1. **First-paint polish.** The first `Pixel*` selection is blank for 2–3 s (R6-1), and the first
   page-tab build overflows to a 670 px row until the next rebuild (R6-2). Both are “build it,
   then fix the layout” — a cheap placeholder/queued rebuild would remove the visible pop.
2. **Settings → Layout is 26 px too wide at 1600** (929 in 903) with an H scrollbar; the first
   Settings open also inherits the previous tool's 175 px strip (R6-3, R6-4).
3. **The builder palette is half-collapsed.** Collapse keeps a 277 px allocation, the 220 px
   minimum is ignored, and the split dragger bypasses the 420/480 limits (537 px palette → 397 px
   view). The view is still 340 px at 1280/1024 (R6-7, R6-8, N32).
4. **HUD discipline in the builder.** The generic status line is sticky across mesh/mode/undo,
   line 2 still clips at ≤1366, and the strip stays pinned at 175 px after a tall section
   (R6-10, R6-11).
5. **#38 is only half done.** `hex_island` reads as open sea, but the tutorials still show a
   straight sea edge and a pale void at the extreme zoom, and `04_world_mood` shows the plane as
   a rotated square (R6-12). Dawn is also still a flat pale wash (R6-18).
6. **Small runtime misses.** F1 covers the player at 800×480 (R6-14); the F3 object picker never
   selects, so the inspector keep-clear path is unverified (R6-13).
7. **`EditorSpinSlider` ignores the wheel** (R6-6), so only some numeric fields respond to the
   wheel.

### One-line verdict

The height **and** width contracts now hold, and the pixelizer, builder strip, dialogs and
runtime HUD all work as designed. The remaining work is mostly **first-paint polish** (pixelizer
placeholder, first tab build), the **builder palette's collapse/limit plumbing**, and finishing
**#38** in the tutorial scenes.

### r7 evaluation verdict

The builder palette now behaves like the terrain panel (collapse gives the space back, 220–420
limits persist, splitter clamps, 585 px view at 1280/1024); Ctrl+D, HUD and strip heights are
right. The runtime HUD (F3/F1) is fully validated, the tutorials' sea reaches the horizon, and
the first tab row no longer overflows.

What still reads wrong on screen: the **Pixelizer appears ~2–3 s late on its first selection**
(no placeholder; R7-3), the **builder keeps an empty Extrude tab in Edge mode** and expanding its
palette at ≤1280 leaves a 397 px view with an unlabeled 32 px tab (R7-5/6), the **first Settings
open still inherits a 175 px strip** (R7-4), **dawn is still a lavender wash** (R7-7), and a
**hard-edged rotated square sits over the sea** at the extreme tutorial zoom (R7-1). The F3
object picker only works on `MeshInstance3D` (R7-2), and `03_pixel_look` shows its field edge
after panning (R7-8).
