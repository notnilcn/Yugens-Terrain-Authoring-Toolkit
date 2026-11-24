# Capture notes — page `09-builder-greybox`

> Source of truth for the writer. Area C08. Documents **everything in the Roblox UI's Model tab**
> for greyboxing (blocking out levels/buildings with simple parts) in the MarchingSquaresTerrain
> addon's "Roblox UI" builder layout. The Edit tab (faces/edges/vertices) belongs to page 10 —
> here we only show it exists.
>
> Session: `docs-greybox`, display `:78`, port `8878`, scene `res://demo/hex_island.tscn`
> (has `BuilderRuin/Ruin`, a `PixelBuilderMesh`). Editor scale 1.0, viewport 1600×900, ~3 fps (llvmpipe).

## Orientation for the writer (what this UI is)

The MarchingSquaresTerrain addon is a **Godot** plugin. Godot is the game engine; the big window is
the **editor**. The parts you block out are **PixelBuilderMesh** nodes (a *node* = one object in the
scene tree, like an Instance in Roblox's Explorer). A **scene** is saved as a `.tscn` file; paths that
start with `res://` are relative to the project folder.

The addon's builder has three UI **layouts** (chosen per user in Editor Settings ›
Marching Squares Terrain › Builder, key `marching_squares_terrain/builder/ui_layout`):
- **MST** (0, default): left icon toolbar + bottom strip + material palette.
- **Modeler** (1): toolbar in the 3D menu bar, Actions menu, popovers, Options dialog, viewport HUD.
- **Roblox** (2): a **Roblox Studio-style ribbon** (Model and Edit tabs) under the 3D menu bar, with
  Roblox shortcuts and Roblox-style viewport handles. **This page is about layout 2.**

All three run the *same* builder — switching layouts keeps the mode, tool, selection and settings.

---

## 1. Feature inventory

### 1.0 Turning the Roblox UI on / off

**Toolbar toggle (primary).** The second toolbar row of the 3D view reads
`Builder | Pixel Preview | Animate | Pixel World | [Roblox UI]`. The **Roblox UI** control is a
flat **switch** (looks like an on/off pill: blue dot on the **left = off**, blue dot on the
**right = on**). Rect in this session `[668,115,118,44]` (whole row y≈103–159).
- Tooltip: *"Roblox UI: show the modelling tools as a Roblox Studio-style ribbon with Roblox handles
  and shortcuts.\nOff: the regular Builder layout."*
- Clicking it on makes the **ribbon** appear under the 3D menu bar (Model/Edit tabs) and switches the
  viewport handles + shortcuts to Roblox style. Clicking off restores whatever layout you had before.
- It is visible whenever the 3D editor is shown. A `PixelBuilderMesh` does **not** have to be selected
  for the toggle to appear (unlike the MST toolbar).
- Media: `01-toolbar-toggle-off.png`, `01-toolbar-toggle-on.png` (before/after close-ups of the row),
  `02-editor-overview.png` (whole editor in Roblox mode).

**Options dialog Layout row (alternative).** Editor Settings route: the layout is stored per-user in
`Editor Settings › Marching Squares Terrain › Builder › ui_layout` (enum **MST / Modeler / Roblox**,
default MST). The Modeler layout's *Options* dialog also has a "UI Layout" row listing the same three.
Picking **Roblox** there is identical to flipping the toggle on; the toggle button follows the setting
(re-read every ~0.25 s). The addon remembers the layout you were in before Roblox
(`ui_layout_before_roblox`) and returns to it when you toggle off. (See `10-options-layout-row.png`.)

Layout dropdown tooltip (verbatim, from `builder/layout.rs`):
> Builder UI layout (per user, Editor Settings › Marching Squares Terrain › Builder):
> MST: left toolbar, bottom strip and material palette.
> Modeler: the original Modeler UI (toolbar in the 3D menu bar, Actions menu, Paint / Selection &
> Handles popovers, Options dialog, viewport HUD).
> Roblox: a Roblox Studio-style ribbon (Model and Edit tabs) under the 3D menu bar, with Roblox
> shortcuts; the Roblox UI button in the 3D menu bar switches it on and off.
> All run the same builder: switching keeps the mode, selection and settings.

### 1.0b Adding a PixelBuilderMesh (a "Part")

A greybox part is a **PixelBuilderMesh** node. Two ways to get one:
- **Insert Part** on the ribbon (the cube button, or Part ▾ → a shape). It drops a block in front of
  the camera and selects it — the Roblox way. No Scene-tree digging. (See §1.4.)
- The Godot way: Scene dock → **Add Child Node** (the `+` at the dock's top-left) → search
  "PixelBuilderMesh" → Create. The new empty mesh appears at the scene origin.

The demo scene `res://demo/hex_island.tscn` already has one: `BuilderRuin/Ruin`. Selecting any
`PixelBuilderMesh` shows the builder UI; in Roblox mode the ribbon is always shown regardless.

### 1.1 The ribbon — tabs and layout

The ribbon sits directly under the 3D menu bar. It has two tabs: **Model** (greyboxing — this page)
and **Edit** (mesh editing: faces/edges/vertices — **page 10**). At the default editor UI scale
(1.0) the ribbon groups **wrap onto two rows** inside the ~880 px viewport column, and each button
shows an **icon only** (no text label — the label is clipped at the 36 px button width; it appears
under the icon only at a larger Editor Display Scale). **Hover any button for its name + tooltip.**
Thin vertical rules separate the groups. Media: `03-ribbon-model-tab.png` (full ribbon, both rows),
`03b-ribbon-tabs.png` (the two tabs).

Model-tab groups, left→right, top row then bottom row at scale 1.0:
Row 1: **Tools** (Select · Move · Scale · Rotate · Transform) │ **Snap** (Geometric ▾, ☑ 1.0 m, ☑ 45°)
│ **Pivot** (Pivot ▾ · Reset) │ **Insert** (Part ▾) │ **Appearance** (Material · Color) │
**Model** (Group ▾ · Lock ▾ · Anchor ▾).
Row 2: **Align** │ **Solid Modeling** (Union · Intersect · Separate · Negate).

> **Icon-only caveat for the writer:** because labels are hidden at normal scale, the guide should
> lean on the **hotspot tour** (icon → name) and always give the keyboard shortcut. Button *names*
> below are the hover-tooltip titles.

### 1.2 Tools group (Select / Move / Scale / Rotate / Transform)

One tool is active at a time (they behave as a radio group even though there is no visible group box).
Pressing a tool shows its Roblox handles on the selected part (see §1.9 Handles). Shortcuts **1–5**.

| Button | Key | Tooltip (verbatim) |
|---|---|---|
| **Select** | `1` | "Pick parts in the viewport with no handles on them" |
| **Move** | `2` | "Drag an axis arrow, the centre square moves in the view plane / Hold V while dragging to snap onto the vertex under the cursor / [Face / Edge / Vertex modes]" |
| **Scale** | `3` | "Drag an axis handle, or the centre square for uniform scale / (snaps to 0.1 steps while grid snapping is on) / [Face / Edge / Vertex modes]" |
| **Rotate** | `4` | "Drag a ring to turn the selection about its centre / (snaps to the Rot° step while grid snapping is on) / [Face / Edge / Vertex modes]" |
| **Transform** | `5` | "Move, resize and rotate with one combined set of handles" |

### 1.3 Snap group

- **Geometric ▾** (drag-mode menu button; shows the current mode as its text). Popup items:
  `Geometric` · `Surface` · (separator) · `Collisions` · `Snap: Use Bounding Boxes` ·
  `Snap to Parts` · `Snap to Terrain Cells`. Geometric = free drag; Surface = slide along the
  surface under the part; Collisions = stop the drag at the first overlap; the three Snap items are
  independent toggles. (Studio calls these "Dragger modes"; see Roblox bridge.)
- **Grid snap** checkbox + **Grid step** spin field, default **☑ 1.0 m**. Tooltip (checkbox):
  "Moves and shapes snap to the Grid step ([ / ] halve / double it)". `[` halves and `]` doubles the
  step. Unit is **metres** (Godot units), not studs.
- **Rotate snap** checkbox + **Rot° step** spin field, default **☑ 45 °**. Tooltip: "Turn the
  rotation step on or off". Rotation handles snap to this many degrees.

### 1.4 Pivot group

- **Pivot** (toggle, [Object mode]) — tooltip: "Move and rotate the pivot of the part without moving
  the part". Turns on an Edit-Pivot tool: move arrows + rotate rings + a white pivot marker sit on the
  node origin; dragging moves the **pivot** (origin), the mesh stays put.
- **Pivot ▾** — one item: **Snap** (snap the pivot to the grid).
- **Reset** (Center pivot, [Object mode]) — tooltip: "Move the node origin to the centre of the mesh
  bounds (the mesh stays where it is)".

### 1.4b Insert group

- **Insert part** (the cube button body) — tooltip: "Add a block in front of the camera, on top of
  what it looks at". Clicking the body inserts a **Block** (4×1×2 m) on the surface under the view
  centre (stacking on a part already there), or 10 m ahead if nothing is hit, and selects it.
- **Part ▾** popup: **Block · Sphere · Wedge · Corner Wedge · Cylinder** · (separator) ·
  **More Shapes ▸** (submenu with the rest of the palette: Cone, Pipe, Torus, Stairs, Arch, Door,
  Hexagon, Icosphere… — see captured list in §Shapes). Studio's five shapes come first, matching the
  Roblox Part menu; Corner Wedge is a real shape kind (5 verts / 5 faces).
- Default sizes (metres, Studio-matched where measured): Block 4×1×2, Wedge/Ramp 4×1×2,
  Cylinder ≈1.97×4×1.99.

### 1.5 Appearance group

- **Material** [Object / Face modes] — tooltip: "Browse the materials and apply one to the selection".
  Opens the **Material picker** popup: title "Material", hint "Select a material to apply", a
  **Search materials** box, **Grid**/**List** view toggle (tooltips "Grid view" / "List view"), shelf
  tabs (**Recent / Project / Built-in**), an ItemList of swatches, and "No materials here" when a
  shelf is empty. Clicking a swatch applies it (whole object in Object mode; the selected faces in
  Face mode). Usable with nothing selected (sets the default for new parts).
- **Color** [Object / Face modes] — tooltip: "Pick a color from the palette and apply it to the
  selection". Opens the **Color palette** popup: a **honeycomb** of preset colours + a **greys row**,
  a separator, a **"Click object to apply color"** switch (paint-bucket mode — click parts to paint
  them), and a custom **ColorPickerButton** ("Pick any colour; Apply paints it") with an **Apply**
  button ("Paint the whole part (undoable)").

### 1.6 Model group (Group / Lock / Anchor)

- **Group** (button body = Group as Model, **Ctrl+G**) — "Put the selected parts under a new Model
  node". **Group ▾**: `Group as Model` · `Group as Folder` · `Ungroup`. (Group as Model wraps the
  selection in a `Node3D` named "Model"; Folder uses a `Node3D` named "Folder"; Ungroup = **Ctrl+U**.)
- **Lock** [Object mode] — "Locked parts cannot be selected or moved in the viewport". **Lock ▾**:
  `Click object to lock` (click-to-lock mode). Locking sets Godot's `_edit_lock_`; a locked part then
  can't be clicked in the viewport.
- **Anchor** [Object mode] — "Seat the part on its terrain cell so it follows the terrain (the terrain
  anchor of this addon, not a physics anchor)". **Anchor ▾**: `Click object to anchor`.

### 1.7 Align group

- **Align** — "Line the selected parts up on their minimum, centre or maximum along X, Y and Z".
  Opens the **Align** panel: **Mode** `Min | Center | Max`; **Align In** `World | Local`;
  **Axes** ☐X ☐Y ☐Z; **Relative To** dropdown (`Selection Bounds` / `Active Object`); an **Align**
  button. Uses the selection's bounding box.

### 1.8 Solid Modeling group (CSG)

All [Object mode]. Greyed with a reason when the selection can't support them.
| Button | Key | Tooltip | Disabled reason (nothing / one mesh) |
|---|---|---|---|
| **Union** | `Ctrl+Shift+G` | "Join the selected parts into one; negated parts are cut away" | "Select two or more builder meshes." |
| **Intersect** | `Ctrl+Shift+I` | "Keep only the volume the selected parts share" | "Select two or more builder meshes." |
| **Separate** | `Ctrl+Shift+U` | "Split a union or intersection back into its parts" | "Select a Union or Intersection to separate." |
| **Negate** | `Ctrl+Shift+N` | "Mark the part as a hole that a later Union cuts away" | (enabled on any one mesh) |

Union of positive + negated parts **subtracts** (the Roblox doorway trick). A Union result is a
normal `PixelBuilderMesh` named "Union" ("Intersection" for Intersect); operands are kept as hidden
children so **Separate** can restore them. Negated parts draw as a translucent **magenta ghost** and
are skipped by bake/collision/export.

### 1.9 Viewport handles (Studio-style) — see clips

Drawn by the builder's own overlay gizmo (Godot's gizmo is suppressed), so Roblox handles appear in
**Object mode** too. Selection is shown as a thin **blue** outline box around the part.
- **Move**: 6 **arrow cones**, two per axis — red X, green Y, blue Z. Drag an axis to translate,
  snapped to the Grid step; the **centre square** moves in the view plane. **Ctrl+L** toggles
  world/local handle space.
- **Scale**: 6 **balls on the face centres**, in axis colours. **Plain drag is one-sided** — the
  dragged face moves, the opposite face stays, so the part grows on one side (the Roblox "extrude").
  **Ctrl+drag is symmetric** (both faces move, centre kept). The **centre square** does uniform scale.
  Shift does nothing special. Recipe parts regenerate (`ShapeSpec.size`) so they stay crisp.
- **Rotate**: 3 **rings** — red X, green Y, blue Z. Drag to rotate about that axis, snapped to Rot°.
- **Transform** (combined): white corner **cube** resize handles, three coloured **rotation arcs**,
  a yellow **plane cone** to move on a plane, and a dark **grid plane** under the part; a round
  grid-plane **#** button at the viewport top-left hides/shows that grid.
- **Pivot** tool: move arrows + rotate rings + white pivot marker on the origin (moves the pivot).

### 1.10 Part inspector (Roblox mode) — right dock

In Roblox mode the Godot Inspector shows a **Part** section above the mesh section, laid out like
Studio's Properties:
- **Part — Appearance:** **Color** (swatch), **Material** (button → opens the picker), **Transparency** (0–…).
- **Part — Transform:** **Size** (x,y,z — edits the recipe; read-only/greyed on a mesh without a
  recipe, taken from the AABB), **Position**, **Orientation** (degrees), **Pivot Offset**.
- **Part:** **Shape** (Block/Sphere/Cylinder/Wedge/Corner Wedge/… or "Mesh"), **Locked** (checkbox),
  **Anchored** (checkbox + "terrain").
- **Behavior:** **Collision** (dropdown — `entity_type`: None/…).
Then Godot's usual **Editable Mesh / Mesh** (vertex/edge/face counts, material slots, UV & Lightmap).

### 1.11 Shortcuts claimed by the Roblox layout (verify each live)

| Keys | Action |
|---|---|
| `1`/`2`/`3`/`4`/`5` | Select / Move / Scale / Rotate / Transform |
| `Alt+1`/`Alt+2`/`Alt+3`/`Alt+4` | Object / Face / Edge / Vertex mode (`H` cycles) |
| `Alt+5` | Vertex paint |
| `Ctrl+L` | Toggle local/world handles |
| `Ctrl+G` / `Ctrl+Alt+G` / `Ctrl+U` | Group as Model / Group as Folder / Ungroup |
| `Ctrl+Shift+G` / `I` / `U` / `N` | Union / Intersect / Separate / Negate |
| `Ctrl+D` / `Del` / `F` | Duplicate / Delete / Zoom-to (Godot's) |
| `Alt+F` | Fill hole (moved off `F`) |
| `[` / `]` | Halve / double the grid step |
| `E` / `I` / `Ctrl+B` / `Ctrl+R` | Edit tab: Extrude / Inset / Bevel / Loop Cut (page 10) |

<!-- live shortcut verification results appended under §Shortcut checks -->

### 1.12 Edit tab (exists; page 10 owns it)

The **Edit** tab holds the mesh-level tools (not Studio-faithful — Studio has no face editing):
mode buttons **Object · Face · Edge · Vertex**; mesh ops **Extrude · Inset · Bevel · Loop Cut ·
Bridge · Merge · Delete**; **Select ▾** (Select Loop/Ring/Grow/Shrink/Similar); **Pivot to Sel.**;
**UV** (opens the UV editor). All greyed in Object mode with "Use … selection mode." reasons. This
page only shows the tab exists; details live on page 10.

<!-- captured tooltips above are verbatim from the live `ui --root all` dump -->


## 2. Walkthroughs

### Walkthrough A — Meet the handles on one part (what I actually did)

1. Turn on **Roblox UI** (toolbar switch). The ribbon appears; the viewport shows Roblox handles.
2. Click **Insert Part** (cube button) → a **Box** (4×1×2 m) drops in front of the camera, selected.
   New parts look **dark and see-through** until you give them a look.
3. Open **Color**, click an orange swatch → "Pixel Builder: Paint colour". Open **Material** →
   **Built-in → Grid**, click it → "Pixel Builder: Apply material". Now it's a solid orange grid box.
4. Press **2 (Move)**: drag a coloured arrow → the box slides on that axis, snapped to 1 m
   (`27-move-drag.mp4`). Ctrl+Z puts it back.
5. Press **3 (Scale)**: drag a **face ball** → only that face moves (one-sided "extrude",
   `25-scale-extrude.mp4`). Hold **Ctrl** and drag → both faces move (symmetric, `26-scale-symmetric.mp4`).
6. Press **4 (Rotate)**: drag a ring → turns 45° at a time (`28-rotate-drag.mp4`).
7. Press **5 (Transform)**: one combined gizmo (corner cubes + arcs + plane cone + grid plane).
8. Press the **Pivot** button: move the part's pivot without moving the part; **Reset** re-centres it.

### Walkthrough B — Greybox a small watchtower (the saved scene)

Built on `demo/hex_island.tscn`'s terrain, saved as `res://tutorial/guide/greybox_tower.tscn`.
Sizes/positions were typed into the **Part inspector** (the reliable, Roblox-Properties way); you can
also drag the handles. Each part was given the **Grid** built-in material so it reads as greybox.

1. **Floor.** Insert Part → in the Part inspector set **Size 8 × 1 × 6**. (`30-build-01-floor.png`)
2. **Back wall.** Insert Part → **Size 8 × 4 × 1**, **Position** behind the floor (z −10.5).
3. **Left wall.** Insert Part → **Size 1 × 4 × 6**, Position x −7.5 (bottom sits on the floor top).
4. **Right wall.** Insert Part → **Size 1 × 4 × 6**, Position x −0.5. (`30-build-02-walls.png`)
5. **Front wall.** Insert Part → **Size 8 × 4 × 1**, Position z −5.5.
6. **Doorway (Negate + Union).** Insert Part → **Size 2 × 3 × 1.5** in the middle of the front wall;
   select it and press **Ctrl+Shift+N** (**Negate**) → it turns a magenta ghost (`40-doorway-01-negate.png`).
   In the **Scene dock**, click the front wall, then **Ctrl+click** the negated box (negated parts
   aren't clickable in the viewport). Press **Ctrl+Shift+G** (**Union**) → the negated box is cut out,
   leaving a doorway; the result is a `PixelBuilderMesh` named **"Union"** (`40-doorway-02-union.mp4`,
   `40-doorway-03-result.png`). (**Ctrl+Shift+U** = Separate puts the wall and door box back.)
7. **Roof.** Part ▾ → **Wedge** → **Size 8 × 2 × 6** on top of the walls.
8. **Stairs.** Part ▾ → **More Shapes ▸ → Stairs** → **Size 2.5 × 1.5 × 2.5** in front of the door.
9. **Group & save.** In the Scene dock, select all the parts, press the ribbon **Group** button
   (Ctrl+G) → they go under a **"Model"** node (renamed **GreyboxTower**). Right-click it →
   **Save Branch as Scene…** → `res://tutorial/guide/greybox_tower.tscn`. (`30-build-03-final.png`)

> The saved scene is self-contained (the Grid material is embedded; `ext_resource` count = 0), so the
> single `.tscn` copies into the repo cleanly.

### 1.4c Shape palette (Part ▾ → More Shapes ▸) — full list

Part ▾ top level (Studio's five): **Block, Sphere, Wedge, Corner Wedge, Cylinder**.
More Shapes ▸ submenu (14): **Prism, Sprite, Plane, Cone, Pipe, Torus, Door, Stairs, Icosphere,
Arch, Hex prism, Hex cluster, Curved stairs, Trim**. (See `11-part-menu.png`, `12-more-shapes-menu.png`.)

### 1.5b Built-in materials (Material picker → Built-in shelf)

Non-texture presets first: **Grid, Checker, No Draw, Collider** (green), **Trigger** (orange), then
**Texture 1 …** (the project's painted textures). Project shelf shows the mesh's own slots
("Slot 0: Material", "Slot 1: Material #ffffff"). Recent shelf holds recently applied ones.

## 3. Media manifest

| File | Kind | Shows |
|---|---|---|
| 01-toolbar-toggle-off.png | before | 3D toolbar row, Roblox UI switch OFF (highlighted) |
| 01-toolbar-toggle-on.png | after | same row, switch ON |
| 02-editor-overview.png | shot | whole editor in Roblox mode (ribbon, viewport handles, Part inspector) |
| 03-ribbon-model-tab.png | shot | the full Model-tab ribbon (both rows) + Model/Edit tabs — hotspot base |
| 03b-ribbon-tabs.png | shot | the Model / Edit tab buttons close-up |
| 11-part-menu.png | shot | Part ▾ menu (Block/Sphere/Wedge/Corner Wedge/Cylinder/More Shapes) |
| 12-more-shapes-menu.png | shot | Part ▾ + More Shapes ▸ submenu (all 14 extra shapes) |
| 13-dragmode-menu.png | shot | Geometric ▾ drag-mode menu (Geometric/Surface + 4 snap toggles) |
| 14-group-menu.png | shot | Group ▾ (Group as Model / Folder / Ungroup) |
| 15-material-picker.png | shot | Material picker, Project shelf (search, Grid/List, shelves) |
| 15b-material-builtin-list.png | shot | Material picker, Built-in shelf, List view |
| 15c-material-grid-view.png | shot | Material picker, Built-in shelf, Grid view |
| 16-color-palette.png | shot | Color palette (honeycomb, greys, Click-to-apply switch, custom+Apply) |
| 17-align-panel.png | shot | Align panel (Mode/Align In/Axes/Relative To/Align) |
| 20-handles-move.png | shot | Move handles: red/green/blue arrow cones + blue selection box |
| 21-handles-scale.png | shot | Scale handles: 6 face-centre balls in axis colours |
| 22-handles-rotate.png | shot | Rotate handles: 3 rings (red X, green Y, blue Z) |
| 23-handles-transform.png | shot | Transform combined gizmo: corner cubes, grid plane, arcs, plane cone |
| 24-handles-pivot.png | shot | Pivot tool: white pivot dot + move arrows + rotate rings |
| 25-scale-extrude.mp4/.png | clip | One-sided Scale drag = Roblox "extrude" (dragged face moves, far face fixed; size.X 4→5) |
| 26-scale-symmetric.mp4/.png | clip | Ctrl+Scale drag = symmetric (both faces move, centre kept; size.X 4→6) |
| 27-move-drag.mp4/.png | clip | Move arrow drag, snapped to the 1 m step (moved +1 m on X) |
| 28-rotate-drag.mp4/.png | clip | Rotate ring drag, snapped to 45° (rotated 45° about X) |

**Verified handle readouts (from `debug_roblox_handles` AABB before/after):**
- Move +X arrow: pos snapped by the 1 m Grid step. Action **"Pixel Builder: Move part"**.
- Scale +X ball (plain): AABB max.X −2 → −1, min.X **fixed** at −6 → one-sided. Action **"Pixel Builder: Resize part"**.
- Scale +X ball (**Ctrl**): min.X −6 → −7 **and** max.X −2 → −1 → symmetric, centre kept. Same action name.
- Rotate X ring: `rotation_degrees` 0 → **45**, snapped to the 45° Rot step. Action **"Pixel Builder: Rotate part"**.
- Every one is **one Ctrl+Z** step (verified: each undo restored the exact previous AABB / rotation).

> **Caveat (capture, not a product bug):** the rotate rings are thin; if a drag **starts off** a
> handle it is treated as an empty-space click and **clears the selection** (handles vanish). This is
> normal Studio-like behaviour, just easy to trip at low frame rates. Re-click the part to reselect.

> **Caveat (important for the writer):** a **brand-new inserted part renders as a dark, see-through
> ghost** until you give it a **Material** or a **Color**. Applying a built-in material (e.g. "Grid")
> or a palette colour makes it a solid, readable greybox part. (All the handle shots use a Box painted
> orange + the "Grid" material.)

### Solid-modeling verified behaviour (live)

- **Negate** (`Ctrl+Shift+N`) on a part → action **"Pixel Builder: Negate"**, sets node meta
  `metadata/mst_negate = true`, and the part becomes a translucent magenta ghost.
- **Union** (`Ctrl+Shift+G`) on a solid wall + an overlapping **negated** box → action
  **"Pixel Builder: Union"**, creates a new `PixelBuilderMesh` named **"Union"**; the operands
  (wall + door box) become **hidden children** of it. Result: the wall now has a doorway hole.
- **Separate** (`Ctrl+Shift+U`) on the Union → action **"Pixel Builder: Separate"**, deletes the
  Union and restores the original `Box5` (wall) and `Box6` (still negated) at their transforms.
- **Union needs ≥2 selected builder meshes; Separate needs a Union/Intersection selected** (otherwise
  the buttons are greyed with those reasons).
- **Multi-select for a boolean:** click one part, then **Ctrl+click** the others **in the Scene dock**.
  (Caveat: a **negated** part is a ghost and is **not clickable in the viewport**, so select it in the
  Scene dock. Also, viewport tree items are not exposed to the capture harness — this is a capture
  detail, not a user issue.)

### Media manifest (continued) — walkthrough & booleans

| File | Kind | Shows |
|---|---|---|
| 30-build-01-floor.png | step | Floor slab (Block 8×1×6) placed on the terrain |
| 30-build-02-walls.png | step | Three walls up on the floor (back + two sides) |
| 40-doorway-01-negate.png | step | Front wall + a door-shaped box **Negated** (magenta ghost in the wall) |
| 40-doorway-02-union.mp4/.png | clip | 287,270,896,353 | **Union** cutting the doorway: grey wall + magenta negated door box → **"Union done"**, a doorway hole left in the wall |
| 40-doorway-03-result.png | step | The wall with a doorway hole (the Union result) |

### Media manifest (continued) — inspector, states, final

| File | Kind | Shows |
|---|---|---|
| 30-build-03-final.png | step | The finished greybox watchtower (walls, doorway, wedge roof) on the terrain |
| 50-part-inspector.png | shot | Part inspector for the Floor: Color/Material/Transparency, Size/Position/Orientation/Pivot, Shape/Locked/Anchored, Collision |
| 51-ribbon-nothing-selected.png | shot | Ribbon with nothing selected — solid-modeling & edit buttons greyed |
| 52-builder-preferences.png | shot | Builder menu → "Preferences…" dialog (Vertex/Handle size, Grid step, Rotate snap, Roblox rotate step, Grid snap, etc.) |
| 60-edit-tab.png | shot | The **Edit** tab (Object/Face/Edge/Vertex, Extrude…Delete, Select ▾, UV) — details on page 10 |

### Shortcut verification (live, this session)

| Keys | Result observed | Verified |
|---|---|---|
| `2` then drag arrow | "Pixel Builder: Move part", snapped 1 m | ✅ |
| `3` then drag face ball | "Pixel Builder: Resize part", one-sided | ✅ |
| `3` + **Ctrl** drag | "Pixel Builder: Resize part", symmetric | ✅ |
| `4` then drag ring | "Pixel Builder: Rotate part", 45° snap | ✅ |
| `Alt+2` / `Alt+1` | switch to Face / Object mode | ✅ (Edit-tab mode followed) |
| `[` | Grid step 1.0 m → **0.5 m** | ✅ |
| `]` | Grid step 0.5 m → **1.0 m** | ✅ |
| `Ctrl+Shift+N` | "Pixel Builder: Negate" (meta `mst_negate=true`) | ✅ |
| `Ctrl+Shift+G` | "Pixel Builder: Union" | ✅ |
| `Ctrl+Shift+U` | "Pixel Builder: Separate" | ✅ |
| `Ctrl+G` (ribbon Group) | "Pixel Builder: Group as Model" | ✅ |
| `Delete` | deletes the selected part (asks to confirm) | ✅ |
| `Ctrl+L` | toggles world/local handles (transient overlay "Handles: local/world") | ⚠ not re-confirmed here; proven in the addon's own runs (RBX-0003) |

## 4. Roblox bridges

| Roblox Studio | MST Roblox UI | Match / difference |
|---|---|---|
| Ribbon Model tab, big labelled buttons | Model tab, same groups | **Matches** the *layout*; MST shows **icons only** at normal editor scale (hover for names). |
| Select/Move/Scale/Rotate/Transform on 1–5 | Same tools on 1–5 | **Matches.** |
| Red X / green Y / blue Z — arrows, face balls, rings | Same handle set + blue selection box | **Matches.** |
| One-sided face-ball drag = "extrude"; Ctrl = symmetric | `FaceResize`, same feel; recipe parts regenerate (stay crisp) | **Matches** the gesture. |
| Snap fields always visible (1 stud / 45°) | Same, but unit is **metres** ("m"), default 1 m / 45° | Differs: **metres, not studs**. |
| Part ▾ (Block, Sphere, Wedge, Corner Wedge, Cylinder) | Same five, plus **More Shapes ▸** (14 more) | **Matches** + extends. |
| Material picker (search, grid/list, 43 named mats) | Same controls; shelves Recent/Project/**Built-in** (Grid, Checker, No Draw, Collider, Trigger, textures) | Controls match; the material set is the addon's, not Studio's 43. |
| Color ▾ honeycomb + greys + "Click object to apply color" | Same honeycomb + greys + switch + custom-colour Apply | **Matches.** |
| Union/Intersect/Separate/Negate (Ctrl+Shift+G/I/U/N), pink negated parts, reversible Union | Same keys, same magenta ghost, Separate restores operands | **Matches** (Intersect/Negate are N-ary here). |
| Group as Model (Ctrl+G), Ungroup (Ctrl+U) | Same | **Matches.** |
| Lock (click-to-lock), Anchor (physics) | Lock = Godot `_edit_lock_`; **Anchor = terrain seating** (not physics) | Lock matches; **Anchor means terrain**, with a tooltip saying so. |
| Align Tool (Min/Center/Max, World/Local, X/Y/Z, Relative To) | Same panel | **Matches.** |
| Properties panel (Size, Position, Orientation, Color, Material, Anchored, Shape) | Part inspector section | **Matches** the fields. |
| Explorer / Properties docks, Home tab, Terrain/Avatar/Effects | — | **Dropped** — Godot's Scene dock + Inspector cover it; no Home tab; no physics/runtime buttons. |
| Studio has **no** mesh/face editing | **Edit** tab adds it (page 10) | MST **adds** what Studio lacks. |

## 5. Caveats

- New parts are solid Medium Stone Grey (the old dark/translucent look was a defect, now fixed —
  see the verdict section below).
- The ribbon is **icon-only** at the default Editor Display Scale (labels are clipped at 36 px). Rely
  on hover tooltips; labels only appear under the icons at a larger editor UI scale.
- The **3D viewport is short** in Roblox mode (the ribbon eats a row), so framing a whole structure
  needs zooming out after **F** (Focus Selection).
- **Negated parts can't be clicked in the viewport** (they're ghosts) — select them in the Scene dock
  for a Union.
- **Rotate rings are thin**: a drag that *starts off* a handle is read as an empty click and clears
  the selection. Re-click the part. (Normal Studio-like behaviour.)
- **Save Branch as Scene does not create missing folders**: `res://tutorial/guide/` had to exist first
  (the dialog errored "Cannot save file" until the folder was created).
- **Options/Layout row:** the three layouts (MST / Modeler / Roblox) are chosen in **Editor Settings ›
  Marching Squares Terrain › Builder › `ui_layout`** (enum; tooltip quoted in §1.0). The Modeler
  layout's own **Options** dialog exposes the same "UI Layout" row (per the addon's T1 run). The Builder
  menu's **"Preferences…"** dialog (`52-builder-preferences.png`) holds builder prefs but **not** the
  layout enum. The **toolbar toggle** is the one-click way and is what readers should use.
- There is a **"Roblox compact ribbon"** checkbox in the Preferences dialog (seen live) — a labels-off
  one-row mode (not exercised here; worth a follow-up capture if the writer wants the compact look).

## 6. Scenes created

- **`res://tutorial/guide/greybox_tower.tscn`** — copied to the repo at
  `tutorial/guide/greybox_tower.tscn`. Root `GreyboxTower` (Node3D) containing:
  `Floor` (Box 8×1×6), `Box2`/`Box3`/`Box4` (back + two side walls, 8×4×1 and 1×4×6),
  `Union` (front wall with a doorway cut by Negate+Union; holds hidden operands `Box5` wall + `Box6`
  door), `Ramp` (Wedge roof 8×2×6), `Stairs` (2.5×1.5×2.5). All use the embedded **Grid** material;
  the scene is self-contained (no external resources). A small greybox watchtower on the terrain.

---

# Follow-up capture session (`docs-greybox2`, 2026-11-23)

> Second pass on page `09-builder-greybox`: adds **21 new MP4 clips** (one per tool / gesture /
> operation the reader performs), re-takes seven stills whose HUD/subject were clipped, and records
> the source-level verdict on the "new parts render dark/translucent" behaviour. Captured in the
> `demo/hex_island.tscn` sandbox; the base scene was never saved (the shipped `greybox_tower.tscn`
> is unchanged). Session: display `:78`, port `8878`, viewport 1600×900, ~4–5 fps (llvmpipe).

## Clips (follow-up) — media manifest

All clips are MP4 (H.264, browser-playable) with a matching `.png` poster; verified by inspecting
the poster plus a mid-frame of each. Crops are tight on the 3D viewport (and the ribbon strip / Part
inspector / picker popup where the action happens there). Rect given as `X,Y,W,H` in window coords.

| File | Kind | Rect | Shows |
|---|---|---|---|
| 04-roblox-ui-toggle.mp4/.png | clip | 288,100,900,320 | Clicking the **Roblox UI** toolbar switch ON → the Model-tab ribbon appears under the 3D menu bar (tooltip shows, then the ribbon renders) |
| 05-select-tool.mp4/.png | clip | 287,318,896,304 | **Select** tool (`1`): click a part → thin blue selection box (no handles); click empty sky → deselect; click again → reselect |
| 06-transform-gizmo.mp4/.png | clip | 287,318,896,304 | **Transform** combined gizmo (`5`): white corner **cube** resize handles + dark **grid plane** + coloured **rotation arcs** + yellow **plane cone** + the round **`#`** grid-plane toggle at the viewport top-left; dragging a corner shows the `W×H×D` size readout, then a rotate |
| 07-pivot-move.mp4/.png | clip | 287,318,896,304 | **Pivot** tool: white pivot marker + move arrows + rotate rings on the node origin; dragging the pivot shows the `+0.xx m` readout and **moves the pivot, the mesh stays put** |
| 08-snap-step.mp4/.png | clip | 490,195,694,428 | **Snap** group: pressing `]` then `[` `[` `[` walks the **Grid step** field `1.0 m → 0.25 m` (field updates live), then a snapped Move drag |
| 09-snap-onoff.mp4/.png | clip | 490,195,694,428 | **Grid snap** checkbox toggled OFF: a snapped drag vs a free drag (the ☑ → ☐ change is visible in the ribbon) |
| 10-insert-shapes.mp4/.png | clip | 287,196,896,428 | **Insert Part** (cube button) drops a Block; then **Part ▾** → Sphere, Wedge, Cylinder — four different shapes, all solid Medium Stone Grey, nudged out from the view centre so each reads separately |
| 11-apply-material.mp4/.png | clip | 490,195,762,428 | **Material** picker: Built-in shelf → click **Checker** → the selected box changes from **Grid** to a checkerboard |
| 12-apply-color.mp4/.png | clip | 490,270,800,352 | **Color** palette (honeycomb + greys + "Click object to apply color" switch + Apply): clicking a swatch tints the selected box |
| 13-undo-move.mp4/.png | clip | 490,340,696,282 | Drag a Move axis arrow (box moves) then **Ctrl+Z** → the box returns to its previous spot (undo after a handle drag) |
| 14-group-ungroup.mp4/.png | clip | 0,120,1120,504 | **Ctrl+G** groups two selected parts under a new **Model** node (the node appears in the Scene dock), then **Group ▾ → Ungroup** removes it. (Scene dock included so the hierarchy change is visible.) |
| 15-lock.mp4/.png | clip | 490,195,694,428 | **Lock** a selected part → its handles vanish and it draws a **yellow locked outline**; a drag over it does nothing (a locked part can't be moved/selected in the viewport) |
| 16-anchor.mp4/.png | clip | 490,195,694,428 | Lift a part off the terrain, then **Anchor** → it **seats onto its terrain cell** (snaps down to the surface, position cell-snapped). Terrain anchor, not physics. |
| 17-align.mp4/.png | clip | 287,320,896,303 | **Align** panel with 3 parts selected: set **Axes Y**, **Mode Center**, click **Align** → all three snap to the same height (verified: Y 11/14/12 → all **12.5**) |
| 18-intersect.mp4/.png | clip | 287,270,896,353 | **Intersect** two overlapping solid-grey parts (4×2×2 and 2×2×4, so the shared volume is a clear 2×2×1 slab) → **"Intersection done"**, operands kept as hidden children |
| 19-union-separate.mp4/.png | clip | 287,270,896,353 | **Union** two overlapping parts → one merged shape (**"Union done"**), then **Separate** → **"Separate: 2 parts restored"** (the originals come back) |
| 20-size-fields.mp4/.png | clip | 490,330,1090,290 | **Part inspector**: editing the **Size** X field `4 → 8` (and Y) — the box grows live in the viewport as the field changes |
| 21-tower-floor.mp4/.png | clip | 287,318,1300,305 | Watchtower build 1/4: Insert Part → set **Size 8×1×6** in the Part inspector (Size fields in the crop) → a solid grey floor slab |
| 22-tower-walls.mp4/.png | clip | 287,318,1300,305 | Watchtower build 2/4: insert + size two walls (8×4×1 back, 1×4×6 side) rising on the floor slab; Size fields in the crop |
| 23-tower-roof.mp4/.png | clip | 287,318,1300,305 | Watchtower build 3/4: **Part ▾ → Wedge**, sized **8×2×6**, placed on top as the roof |
| 24-tower-final.mp4/.png | clip | 287,318,896,304 | Watchtower build 4/4: camera orbits the finished block-out (floor + walls + wedge roof) on the terrain |

**Verified live (follow-up session):**
- Snap `[` / `]`: Grid step `1.0 → 0.5 → 0.25 m` and back; the shortcut needs **keyboard focus in the 3D viewport** (click a part with the Select tool first, then press `[`/`]`).
- Align on **Y / Center**: three parts at Y 11, 14, 12 all became **Y 12.5** (centre of the selection bounds). Panel: Mode `Min|Center|Max`, Align In `World|Local`, Axes `☐X ☐Y ☐Z`, Relative To `Selection Bounds|Active Object`.
- **Intersect** makes a new `PixelBuilderMesh` named **"Intersection"**; **Union** makes **"Union"** (18 verts from two boxes); **Separate** on either restores the operands (`Box` + `Box2`). HUD confirms each: "Intersection done" / "Union done" / "Separate: N parts restored".
- Multi-select for booleans/group/align: **Shift+click adds to the selection** (not Ctrl+click), and a **viewport marquee** (drag the Select tool over several parts) also multi-selects — both verified. (Ctrl+click did **not** add here; this corrects the earlier note that said Ctrl+click.)
- **Lock** draws a yellow outline and blocks viewport selection/drag of the part; you can still re-select it from the Scene dock / by `select --path`.
- **Anchor** moved the part to its cell centre and dropped it onto the terrain surface (y 12.7 → 10.0, z snapped), `anchored` meta → true.

> **Capture gotcha (not a product issue):** in `hex_island`, clicking the terrain/water in the 3D
> view can select **PixelWater** and flip the editor into the **Pixel World** workspace, which also
> freezes the builder camera (f / scroll / orbit stop responding). Re-select a `PixelBuilderMesh`
> (e.g. the ruin) to return to the builder context and the camera unlocks. Deselect by clicking the
> **sky**, never the terrain.

## Retakes (same filenames, re-cropped so nothing is clipped)

The brief flagged stills that crop off the top-left builder HUD text. Fixed by cropping from the
**viewport's left edge (x≈287)** so the full HUD (`Pixel Builder — Object mode, … | selected: …`)
shows, with the subject centred and complete:

- **20-handles-move / 21-handles-scale / 22-handles-rotate / 23-handles-transform / 24-handles-pivot** —
  re-taken as a consistent set. The handle box is now a **solid white "Grid" box** (not the earlier
  orange one — see the material note below); every handle set reads clearly against it, and the HUD
  ("…Grid applied to 6 faces") is fully in frame.
- **30-build-01-floor** (the example in the brief) and **30-build-02-walls** — re-taken with the full
  HUD; floor slab / floor+walls shown on the terrain.
- **30-build-03-final** was already a clean crop (no HUD, complete subject) and was **not** re-taken.

> Writer note: update any caption that calls the handle box "orange" — the re-taken handle stills use
> a neutral **white Grid** box.

## Verdict on "new parts render dark / translucent" (was a defect — now FIXED)

**Symptom (reproduced on the pre-fix build):** a freshly **Insert**ed part rendered as a dark,
see-through ghost — you could see the terrain through its faces — even though its **Transparency**
was `0.0`. Part of the mesh also copied the *selected* mesh's material slots, so a new part could
inherit whatever look the previously selected part had.

**This was a real defect, and it is fixed.** The fix (in the working tree while this guide was
being written) is:

- `part_rules::grey_bare_faces` (`builder/roblox_ui/part_rules.rs`) adds `NEW_PART_TINT =
  [163, 162, 165, 255]` — Roblox's **Medium Stone Grey** — to every face that has no picked colour
  and no material. A face that already has a material keeps it un-tinted; a face with a picked
  colour keeps that colour.
- `give_new_part_material` (`builder/plugin/roblox_panels.rs`) now runs that tint on the new part
  even when no default material is set, and clears the inherited material slots instead of copying
  the selected mesh's materials.
- `insert_part` still applies the user's default part material when one is set (Material picker
  with **nothing selected**), so the "set a default material once" workflow still works.

**Verified after the fix (this session):** Insert Part drops an opaque Medium Stone Grey block that
reads clearly against the terrain with no material and no colour step; a second insert is grey too
(it does not copy the first part's look). A face seen against the sky shows no terrain through it.
The remaining dark patches on some faces are the pixel renderer's own shadow at a grazing sun angle
(the same shading the terrain gets), not transparency.

**For the writer:** no "set a default material first" step is needed for legibility any more. The
Material default is now only about *style* (e.g. the Grid look), not visibility.

## Caveats (follow-up additions)
- The default-material setting is per plugin session (it does not persist across editor restarts).
  It only changes the *look* of new parts; they are solid grey either way now.
- `[` / `]` (grid step) and `f` / scroll / orbit need the **3D viewport to hold keyboard/mouse
  focus** — click a part (Select tool) first. A stuck popup (Part ▾, Material/Color) eats clicks;
  dismiss it before driving the ribbon.
- Deselect by clicking the **sky**; clicking terrain can grab PixelWater and open the Pixel World
  workspace (which locks the builder camera until a builder mesh is re-selected).
- **Shift+click** (or a Select-tool **marquee**) is the viewport way to select several parts for a
  boolean / group / align; a **negated** part is a ghost and must be picked in the Scene dock.
- New parts **Insert at the view centre and stack** (each lands on top of the last at the same spot),
  so to spread several out you drag/position them after inserting (there is no scatter-on-insert).

## Scenes created (follow-up)
- None. The follow-up session worked in an unsaved `demo/hex_island.tscn` sandbox; the shipped
  `tutorial/guide/greybox_tower.tscn` from the first pass is unchanged.

## 5. Caveats

## 6. Scenes created

---

# Re-capture (review pass, 2026-11-24)

> The eight clips above were re-recorded on the fixed build (solid Medium Stone Grey new parts) and
> moved into `docs/guide/recipes/09-builder-greybox.json` as replayable items. Every item is
> `fresh_session: true` and is verified with
> `python3 qa/recapture.py docs/guide/recipes/09-builder-greybox.json --only <id> --out-dir <tmp> --display :111 --port 8911`.

**Why the earlier clips were bad:** they were recorded in a session that had started *before* the
grey-part fix was built, so the new parts still rendered dark/see-through, and the tower clips were
framed too close (the camera ended up inside the walls, showing one huge dark face).

**How the camera is placed now:** `docs/guide/recipes/09-frame.py` (a small helper the recipes call
through a `shell` step) selects the anchor part, presses F, then nudges the builder camera with
wheel steps and middle/shift-middle drags until the camera sits at a requested distance, azimuth
and pitch from a world target, with the target panned to the centre of the viewport. That removed
the guesswork of "F plus N wheel steps", which gave a different distance almost every run.

**Clip changes worth knowing for captions/alt text:**

- `10-insert-shapes` — Block, Sphere, Wedge and Cylinder are all solid grey and separated; the Part ▾
  menu openings are in frame (rect starts at the ribbon).
- `18-intersect` / `19-union-separate` — the two operands are 4×2×2 and 2×2×4, so the shared volume
  is a visible 2×2×1 block instead of a sliver; Union/Separate are triggered with
  Ctrl+Shift+G / Ctrl+Shift+U (the ribbon buttons shift position with the selection).
- `40-doorway-02-union` — Negate is clicked on the ribbon (Ctrl+Shift+N is the editor's New
  Inherited Scene, so the shortcut is not usable here); the HUD reads "Negate: 1 part", then Union.
- `21`–`24` — the crop now includes the full viewport *and* the Part inspector, so the HUD is not
  clipped and the Size fields are visible. The camera looks from the sunlit south-west so no wall
  face reads black.
- Known limitation: at the wide harness viewport (896×305, ~128° horizontal FOV) the tower clips
  show the block-out at roughly a quarter of the frame width; the parts are clearly grey and fully
  in frame, but they are not as large as the one-part clips.

**Backfill items** (the other page-9 media) now live in the same recipe file (29 items covering
every remaining media file; a numbered series is one item with several outputs). They were written
from this notes file and the command log. Tested end to end in this pass and matching the shipped
media: `01-toolbar-toggle` (crop not re-checked), `11-part-menu`, `13-dragmode-menu`,
`16-color-palette`, `50-part-inspector`, `60-edit-tab`. The rest (handle stills, picker views,
the tool clips, 30-build-*, 40-doorway stills) are written but **not yet run to completion** —
expect to adjust a drag point or crop when they are next used. `04-roblox-ui-toggle` needs a
session whose layout starts in MST (its own fresh session), because the setup step only turns the
Roblox UI on when it is missing.

## Recipes

Verification pass on `docs/guide/recipes/09-builder-greybox.json` (2026-11-24), every item run
with `python3 qa/recapture.py docs/guide/recipes/09-builder-greybox.json --only <id>
--out-dir /tmp/opencode/ds09-ver* --display :119 --port 8919`; outputs viewed against
`docs/guide/media/09-builder-greybox/`. `docs/guide/media/` was never written to. All 38 items now
set `fresh_session: true` (several backfill items were not self-contained and inherited the
previous item's Box/Box2/Box3 nodes when run in a batch, which changed the Scene-dock rows and the
framing). The shared `setup` selects `BuilderRuin/Ruin` and turns the Roblox UI on; items that need
MST layout (04) or a plain part seed their own state.

### Green (output matches the media's subject, framing and state)

| Item | Result |
|---|---|
| 02-editor-overview | Full 1600x900 editor in Roblox mode. The frame helper's shift+middle pan leaves a transient "n m" measure bubble for ~1 s; a `move` + `wait` clears it. Shipped media shows a slightly different ruin angle but the same overview. |
| 03-ribbon | Crop 283,128,882,192 upscaled 2x to 1764x384 reproduces the shipped media (cross-correlation diff 1.4/255). 03b is the 120x32 tabs crop upscaled 3x (360x96). |
| 12-more-shapes-menu | Part menu (862,274) + More Shapes submenu (1055,478); synthetic hover does not reliably open the submenu, so it is opened with 6x Down + Right and the crop 856,266,365,560 is scaled to the shipped 584x891. |
| 14-group-menu | Group popup (1024,274,179,87); crop 1019,268,190,99 at 2x = 380x198. |
| 51-ribbon-nothing-selected | Same crop as 03-ribbon (283,128,882,192 at 2x); sky click 420,340 clears the selection. |
| 20-24-handles | The box is given the Built-in **Grid** material (media HUD: "Grid applied to 6 faces"); camera dist 3.2 was still too close - dist 4.6 with crop 287,318,633,285 matches the media size. Tool buttons in this layout are 36 px wide from Select 287: Move 335, Scale 385, Rotate 425, Transform 465, Pivot 730 (the old 760 was in a gap). |
| 15-material-picker | Media is a Project-shelf capture of the Ruin's two slots. The picker's shelf is sticky in this build, so the recipe clicks Project (1005,397) explicitly, then Built-in 1096,397, List 1215,362, Grid 1168,362. Crop 885,265,372,520 = the whole popover. The shipped 704x520 media is a larger editor-display-scale capture; content is identical, scale is not. |
| 17-align-panel | Align is ribbon row 2 button 295,310 in this layout (not 640,296). Panel rect 295,310,295,225; crop 285,310,303,244 resized to the shipped 606x472. |
| 52-builder-preferences | Builder menu (291,159,252,349) -> Preferences is item 2: down x2 + Enter (one Down highlights item 1 and Enter opens the UV editor - that was the earlier failure). Verified via `windows` containing "Pixel Builder Preferences"; crop 566,150,472,620. The shipped 637x588 media is a larger-scale capture of the same rows. |
| 04-roblox-ui-toggle | The shared setup leaves Roblox mode on, so the item first toggles it off back to the saved MST layout, then records hovering 650,137 and clicking it on; ribbon visible from ~6 s. Media 900x320, rect 288,100,900,320, poster at 7 s. |
| 11-apply-material | Picker opens on Built-in for a new Box; row 2 (y=480) is Checker, row 1 Grid. Click 1146,479 applies Checker; camera dist 4.6. |
| 12-apply-color | Color 954,236, palette over the part, click 990,420; camera dist 4.6. |
| 13-undo-move | Media's translucent fill is the selection tint of the old part; the current build renders the same state (dark translucent box + blue outline) when Move is active. Drag starts on the X arrow at 780,470 (not 735,470, which is the box centre); record 287,318,696,282 = media size. |
| 14-group-ungroup | Two boxes, Scene-dock multi-select (select Box, Ctrl+click Box2 row 95,443), Group via keyboard (Down+Enter) then Ungroup (Down x3 + Enter). The earlier failure was an `expect select --path Box` placed after the Ctrl+click, which reset the multi-selection. |
| 15-lock | Lock 1060,236; HUD "Locked 1 node", handles vanish, drag does nothing; camera dist 4.6, record 694x428. The media also shows a second teal part the recipe does not create. |
| 17-align | Three boxes; Scene-dock rows are Box 393, Box2 420, Box3 442 (Ctrl+click 420/442). Align panel buttons by pixel: Mode Center 465,372, Axes Y 458,447, Align 438,505 (click-text Y used to hit a ribbon control and turn on Pixel Preview). Crop 287,300,896,302; poster 1.5 s. |
| 06-08-09-snap | `[`/`]` walk the step 1.0 -> 2.0 -> 0.25; the Grid snap checkbox is 588,216 (rect 574,198,28,36), not 761,242. 09 shows it unticked and the 45 deg checkbox still ticked; record 287,196,694,428. |
| 20-size-fields | Box selected, inspector scrolled, Size X field focused and "8" typed; record 287,318,1300,305. The shipped 1090x290 media was captured in a different (snow) scene, so background/framing differ; the field content/state matches. |
| 16-anchor | Box lifted to y=18 then Anchor 1110,236; poster shows the seated part with "Anchored 1 part". The shipped media is a much wider terrain view where the part is all but invisible; same state, different framing. |

### Not green (precise reason)

| Item | Reason |
|---|---|
| 05-select-tool | The media's "click empty space to deselect, then click the part again" half cannot be reproduced in `demo/hex_island.tscn`: the PixelWater plane covers the whole view, so every non-part click (sky at 1000,322, grass at 850,470, the ruin at 800,345) selects PixelWater and switches the 3D view to the Pixel World water workspace. Pruning the editor's fallback workspace by selecting the terrain first does not survive the material-picker/frame-helper steps. The recipe now only re-selects the part; the deselect frame is missing. |
| 25-28-handle-drags | The shipped media is the **pre-fix orange** default part; the current build renders new parts solid Medium Stone Grey, so colour/state cannot match. Under XTEST the face-ball drags also overshoot (a 14 px drag at dist 4.6 doubles the 4 m box), so 26-28 show an oversized part. The Ctrl+Z steps were removed because a missed drag + undo deleted the inserted box, emptying 26-28; with undo it is empty, without it the part grows. Ball/ring positions: +X face ball ~937,470. |
| 30-build-stills | 30-01 (Grid floor slab, dist 5) and 30-02 (floor + walls) now match in subject and use the white Grid material. 30-build-03-final cannot be reproduced: the shipped still is an **orange pre-fix tower** in a different (cloudy sky) hand-built scene, while this item rebuilds only the floor + two walls in `demo/hex_island.tscn`. |
| 40-doorway-stills | 40-doorway-01-negate shows the wall with the magenta negated box ("Negate: 1 part"). The shipped media is the orange pre-fix wall, and in the fresh session the Ctrl+click at 95,421 / Ctrl+Shift+G does not produce the doorway — the union is not applied, so 40-doorway-03-result still shows both parts. Needs the multi-select/union step reworked before this item can be green. |
| 06-07-transform-pivot | 07 works (Edit pivot, rings dragged, mesh stays). 06 shows the Transform gizmo and drags, but the media's "W x H x D" readout comes from a corner-cube *scale* drag; the probes dragged the translate plane/arc instead and the corner-cube point depends on the framing, so the readout is not reliably captured. |

### Recipe-level notes

- The editor window is 1600x900 in these sessions; the ribbon is left-aligned in the ~896 px
  viewport column, so all backfill coordinates from the original notes that assumed a wider ribbon
  (Insert part 930, Group 1035, Align 640,296, Union 357,296, Pivot 760, snap checkbox 761,242)
  were re-measured live with `qa/drive.py ui --root all --filter ...`: Insert part 822..858,
  Part ▾ 862, Align 287,296 (row 2), Group 984 (dropdown ~1032), Lock 1042, Anchor 1100,
  Union 339/278, Negate 459/278.
- The material picker is closed with a click on the Select button (305,236), not Esc: Esc can
  deselect and revert the editor to the previous (Pixel Water) workspace.
- The frame helper's shift+middle pan leaves a transient distance bubble ("6.3 m") in the viewport;
  add `move` + ~2 s `wait` before a still/recording.

## Re-shoot (recipes)

Staged and verified with `qa/recapture.py` on `:124`/`8924` (`--out-dir`), then copied over
`docs/guide/media/09-builder-greybox/`. The current build paints new parts solid Medium Stone
Grey, so every re-shot part is grey (the old media was the pre-fix orange).

| File(s) | Result |
|---|---|
| `05-select-tool.mp4` / `.png` | **Re-shot.** Deselect is now <kbd>Esc</kbd> (mouse over the view) instead of clicking the PixelWater plane, which used to switch the 3D view to the water workspace. Clip: click part → Esc (box outline gone) → click part. Caption updated to say Esc. |
| `06-transform-gizmo.mp4` / `.png` | **Re-shot.** `09-grip.py` grabs the +X/-Z top corner cube and drags it 27×22 px (Size X 4 → 5); the poster is mid-drag and shows the `5.00 × 1.00 × 2.00` readout. The rotation-arc half was dropped from the clip/caption (it depended on framing); Pivot half unchanged. |
| `22-tower-walls.mp4` / `.png`, `23-tower-roof.mp4` / `.png`, `24-tower-final.mp4` / `.png` | **Re-shot.** Perspective VFOV lowered from 70 to 40 through View → Settings (`09-vfov.py`), camera moved in from dist 17/16 to 11.5. The tower now fills most of the viewport height instead of a quarter. Rects kept (1300-wide for 22/23 with the Size fields; 896 for 24). |
| `25-scale-extrude.mp4` / `.png`, `26-scale-symmetric.mp4` / `.png`, `27-move-drag.mp4` / `.png`, `28-rotate-drag.mp4` / `.png` | **Re-shot.** Grey part. A helper (`09-grip.py`) computes the handle centre from the part cage + camera, so the drag lands even when the frame helper's centring drifts: +X ball 80 px → Size 5.0; Ctrl+ball 80 px → 6.0 (centre fixed); +X arrow 80 px → Position −13.0; X ring 60 px → Orientation 45.0°. The record rect now includes the Part inspector, so the numeric readout is in frame (posters show Size 5.0/6.0, Position −13.0, Orientation 45.0). |
| `30-build-03-final.png` | **Replaced.** The old still was an orange tower from another scene. The new still is the same finished grey tower frame as `24-tower-final.png` (floor + two walls + wedge roof, gridded floor), produced by the 24 recipe item and copied to this name. The 30-01/02 stills are unchanged (already green). |
| `40-doorway-01-negate.png` | **Re-shot.** Same recipe (wall 8×4×1 + negated 2×3×1.5 ghost); the wall is now grey instead of orange. Alt text updated. `40-doorway-03-result.png` was left as-is (not in this re-shoot scope). |
| `50-part-inspector.png` | **Re-shot.** Same recipe; the Color swatch is now the build's default grey instead of orange. Alt/caption updated. |

Recipes/helpers: `docs/guide/recipes/09-builder-greybox.json` (items updated; 30-03 now comes from
the 24 item, a separate `30-build-03-final` item was removed), plus `09-vfov.py` (View → Settings
VFOV) and `09-grip.py` (handle-position maths + real drag). Unchanged in this pass:
`07-pivot-move.*`, `30-build-01/02*`, `40-doorway-03-result*`.

Checked but not replaced: `40-doorway-03-result.png` is still the old orange union result. The
current build's Ctrl+click + Ctrl+Shift+G union step does not apply (the re-run leaves the wall and
the magenta ghost side by side — see the known gap in the 40-doorway-stills notes), so re-shooting
it would replace a correct doorway with a wrong non-doorway. It needs the union step fixed before it
can be re-shot. `07-pivot-move.*`, `30-build-01/02*`, the UI stills and the deliberate teal
`12-apply-color` part were left unchanged.

**Update — `40-doorway-03-result.png` re-shot (2026-11-24, `rs2-`, `:124`/`8924`).** The
`40-doorway-stills` recipe item now copies the working selection + Union steps from
`40-doorway-02-union`: `select Box2`, **mouse over the 3D view first** (`move 95 421` for the
Scene-dock row, then `move 650 450` before <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>G</kbd> — without
the second move the shortcut never reached the builder, which is why the old run left the wall and
the ghost side by side). The Union is asserted with `nodes PixelBuilderMesh` containing `Union`,
then <kbd>Esc</kbd> deselects and the camera zooms the same view in (`--dist 7 --elev 16`, same
azimuth/crop 287,318,896,304) so the doorway is large and obvious. New still: grey wall with a
door-shaped hole down to the ground, no selection outline. `40-doorway-01-negate.png` is unchanged
(pixel diff 0). Alt text/caption updated. New item check: "the same grey wall with a door-shaped
hole cut through it, no selection outline".
