# Capture notes — page `10-builder-modeller` (Making & editing meshes in the Builder)

Area C09. Session `docs-modeller`, display `:79`, port `8879`, scene `res://demo/hex_island.tscn`
(builder mesh at `BuilderRuin/Ruin`, a `PixelBuilderMesh`). Editor resized to **1920×1080** so the
builder panels don't squeeze the 3D view. Captured from the real Godot 4.7.2 editor.

This page is the **mesh-modelling** companion to the parts page: it covers editing the actual
polygons of a builder mesh — selection modes, extrude/inset/bevel/loop-cut and the rest, painting,
materials, the UV editor, export, Options, and how the three builder layouts (Roblox UI / Modeler /
MST) differ. The reader's comfort zone is the **Roblox UI → Edit tab**, documented first.

> **Orientation.** The three layouts are chosen by the **Roblox UI** toggle in the 3D-view toolbar
> (top strip, to the right of the viewport tools; rect ~[1373,82,118,44], label "Roblox UI",
> tooltip *"Roblox UI: show the modelling tools as a Roblox Studio-style ribbon with Roblox handles
> and shortcuts. Off: the regular Builder layout."*). Turn it ON → a ribbon with **Model** and
> **Edit** tabs appears under the 3D menu bar. Turn it OFF → the regular Builder layout (which is
> itself either **MST** or **Modeler**, chosen in the Builder menu / Options). All three drive the
> *same* commands; only the surface differs.

---

## 1. Feature inventory

### 1a. Roblox UI — the **Edit** tab ribbon (the mesh-editing tools)

Appears when Roblox UI is ON and you click the **Edit** tab (next to **Model**). Left→right:

**Selection modes** (radio group; one lit at a time; the lit one is blue):

| Button | Shortcut | What it does | Notes |
|---|---|---|---|
| **Object** | `Alt+1` (Roblox) / `1` (other layouts) | Whole meshes. Tooltip: *"Whole meshes (use Godot's gizmo to move them)"* | Default. Only Object-mode actions (Insert part, Lock, Anchor, Pivot, Union…) are enabled here. |
| **Face** | `Alt+2` / `2` | Tooltip: *"Click faces (Shift adds), drag on empty space for a box"* | Enables Extrude, Inset, Merge, Delete, Pivot-to-Sel. |
| **Edge** | `Alt+3` / `3` | Tooltip: *"Pick edges"* | Enables Extrude, Bevel, Loop Cut, Bridge, Delete. |
| **Vertex** | `Alt+4` / `4` | Tooltip: *"Pick vertices"* | Enables Bevel, Merge, Delete. |

`H` cycles the modes (Object → Vertex → Edge → Face, per the shortcut name). In the Roblox layout
the bare `1`/`2`/`3`/`4` keys are the **tools** (Select/Move/Scale/Rotate/Transform), so the modes
move to `Alt+1..4`. In MST/Modeler layouts the bare digits are the modes.

**Mesh operations** (greyed out until the matching mode + a selection exist; the disabled tooltip
appends the reason, e.g. *"Use face selection mode."*):

| Button | Shortcut | Modes | Tooltip (verified) |
|---|---|---|---|
| **Extrude** | `E` | Face / Edge | *"Faces along their normals, open edges outwards, by Dist. Faces: Extrude method in the strip (face normal / vertex normal / individual); edges: As group"* |
| **Inset** | `I` | Face | *"Shrink the selected faces into their own faces by Inset. Inset by: Fraction (towards the centre), Width or Equal (a rim Inset metres wide, mitred corners)"* |
| **Bevel** | `Ctrl+B` | Edge / Vertex | *"The selected edges (Edge mode) or corners (Vertex mode) by Bevel over Segs segments. Amount type Offset / Width / Depth / Percent, Profile (0.5 round), Clamp overlap, Harden normals; connected edge rims inset their corners. 1 segment chamfers, more round the edge"* |
| **Loop Cut** | `Ctrl+R` | Edge | *"Insert edge loop — Cuts loops across the quad ring through the selected edge"* |
| **Bridge** | (no default key) | Edge | *"Join two open (boundary) edges, or two equal open edge runs, with faces"* |
| **Merge** | `M` | Face / Vertex | *"Faces into one n-gon / vertices into one"* |
| **Delete** | `Delete` | Face / Edge / Vertex | *"Remove the selected faces / edges / vertices"* |

**Select ▾** (MenuButton) — selection-growing helpers: **Select Loop** (`L`), **Select Ring**
(`Alt+L`), **Grow** (`G`), **Shrink** (`Shift+G`), **Similar**. (Source: ribbon `RibbonMenu::Select`.)

**Pivot to Sel.** — *"Move the node origin to the centre of the selected elements"* (element modes).

**UV** — `U` — *"Open the UV window of this mesh (manual UVs: move / scale / rotate, planar / box
projection, flip, fit, sew, copy / paste, texel density)"*.

(Media: `01-edit-ribbon.png` — the whole Edit ribbon, Object mode lit, operations greyed.)

### 1b. Roblox UI — the **Model** tab (parts; covered fully on the parts page, summarised here)

Select/Move/Scale/Rotate/Transform tools (`1`–`5`), **Geometric/Surface** drag-mode dropdown, grid
step spin (metres) + rotation-snap spin (°), Pivot/Reset, Insert **Part** ▾ (Box/Sphere/Wedge/
Corner Wedge/Cylinder + **More Shapes**), **Material**, **Color**, **Group** ▾, **Lock** ▾,
**Anchor** ▾, **Align**, and the booleans **Union** (`Ctrl+Shift+G`) / **Intersect**
(`Ctrl+Shift+I`) / **Separate** (`Ctrl+Shift+U`) / **Negate** (`Ctrl+Shift+N`).

---

## 2. Operations — verified behaviour & adjust panels

Setup for all of these: a fresh scene with a **PixelBuilderMesh** root, a **Box** part inserted
(Model tab → Part), coloured terracotta (Color palette). On a plain dark scene builder meshes
render as solid shaded faces; in the busy `hex_island` scene they render as pale/translucent
overlays (see Caveats). Pick a selection mode, select something, then run the op. Every op below
committed as exactly **one** undo step (History dock action name in brackets).

### Extrude (`E`) — [Pixel Builder: Extrude]
Face or Edge mode. Pushes the selection along its normal and makes the connecting side walls.
Opens an **Adjust Extrude** panel: **Dist** (spinbox, metres — can be negative to push inward),
**Extrude** method dropdown *(Face normal = as one along the faces' normals · Vertex normal = as
one along averaged vertex normals · Individual = every face along its own normal)*, **Cancel** /
**Done**. (Media: `11-adjust-extrude.png`, before/after `12-extrude-before/after.png`.)

### Inset (`I`) — [Pixel Builder: Inset]
Face mode. Makes a smaller face inside each selected face, with a rim around it.
**Adjust Inset** panel: **Inset** amount (spinbox) *(fraction toward the centre, or rim width in
metres)*, **Inset by** dropdown *(Fraction = corners move toward centre · Width = a rim this many
metres wide · Equal = mitred corners, same width everywhere)*, **Individual** checkbox *(inset every
face on its own; off = connected faces inset as a group)*. (Media `13-adjust-inset.png`.)

### Bevel (`Ctrl+B`) — [Pixel Builder: Bevel]
Edge mode (selected edges) or Vertex mode (corners). Rounds/chamfers the selection.
**Adjust Bevel** panel: **Bevel** amount · **Amount** type dropdown *(Offset = distance from the
edge · Width = of the new face · Depth = of the cut · Percent = 0–100 % of the adjacent edge)* ·
**Segs** 1–32 *(1 = chamfer, more = rounded along the Profile)* · **B profile** *(0.5 circular,
lower = concave, higher = convex/boxy)* · **Clamp** overlap checkbox · **Harden** normals checkbox.
(Media `21-adjust-bevel.png`, and `20-select-loop.png` for the rim selection it worked on.)

### Selection growing (the **Select ▾** menu, and keys)
- **Select Loop** `L` — verified: picking **1** rim edge then `L` selected the whole **4-edge**
  top loop.
- **Select Ring** `Alt+L`, **Grow** `G`, **Shrink** `Shift+G`, **Similar** (menu only).

### Walkthrough performed — "Terracotta crate" (saved as a scene)
1. New scene → root node **PixelBuilderMesh** (Create New Node dialog).
2. Roblox UI **Model** tab → **Part** → a Box appears at the view centre (`10-walk-start.png`).
3. Coloured it terracotta with the **Color** palette (hex `c8743c`).
4. **Face** mode (`Alt+2`) → clicked the **top** face (`sel-top`).
5. **Extrude** `E`, Dist ≈ 1 → a tall box (`12-extrude-after.png`).
6. **Inset** `I` on the top face → a lid rim (`13-inset-after.png`).
7. **Extrude** `E` the inset face with **Dist −0.7** → a hollow recess, an open chest
   (`14-crate-hollow.png`).
8. **Edge** mode (`Alt+3`), clicked a top-rim edge, **Select Loop** `L` → the 4-edge rim
   (`20-select-loop.png`).
9. **Bevel** `Ctrl+B`, Segs 3, Offset 0.22 → a rounded top lip (`15-prop-beveled.png`).
Each step was one undo step. (Painting, materials, UVs, save and export follow below.)

### Verified shortcuts (every one performed in the editor, Roblox layout)

| Keys | Action | Result observed (History action / state) |
|---|---|---|
| `Alt+1/2/3/4` | Object / Face / Edge / Vertex mode | mode label changed each time |
| `H` | Cycle modes | Object → **Vertex** → **Edge** → **Face** → Object (this order) |
| `E` | Extrude | "Pixel Builder: Extrude" + Adjust Extrude panel |
| `I` | Inset | "Pixel Builder: Inset" + Adjust Inset panel |
| `Ctrl+B` | Bevel | "Pixel Builder: Bevel" + Adjust Bevel panel |
| `Ctrl+R` | Loop Cut | "Pixel Builder: Insert edge loop" + Adjust panel (Cuts) |
| `M` | Merge | "Pixel Builder: Merge faces" |
| `J` | Connect | "Pixel Builder: Connect" |
| `Alt+F` | Fill hole (Roblox layout; plain `F` is Zoom-to here) | "Pixel Builder: Fill hole" |
| `Ctrl+T` | Triangulate | "Pixel Builder: Triangulate faces" |
| `Ctrl+D` | Duplicate | "Pixel Builder: Duplicate" |
| `Delete` | Delete | "Pixel Builder: Delete" |
| `G` / `Shift+G` | Grow / Shrink selection | 1 → 5 → 1 faces |
| `L` | Select Loop | 1 → 4 edges (the rim loop) |
| `Alt+L` | Select Ring | 1 → 4 edges (the ring) |
| `[` / `]` | Halve / Double grid step | grid-step spin changed (1.0 → 2.0 m) |
| `Esc` / **Cancel** | Cancel the current op / adjust | adjust panel closes, op discarded |

> **Gotcha — keyboard focus.** Editor 3D shortcuts only fire when the **mouse is over the 3D
> viewport** (standard Godot behaviour). `Alt+3` etc. did nothing until the cursor hovered the view.
> Teach readers to move the mouse into the 3D view before pressing a key.

> **Roblox vs other layouts.** In the Roblox layout the bare digits `1`–`5` are **tools**
> (Select/Move/Scale/Rotate/Transform) and the **modes** are `Alt+1`–`Alt+4`; `F` is **Zoom-to** and
> **Fill** is `Alt+F`. In the **Modeler** and **MST** layouts the bare digits `1`–`4` are the
> **modes**, `5` is vertex paint, and `F` is **Fill hole**. (Source: `shortcuts.rs`
> `DISPLACED_BY_ROBLOX`.)

---

## 3. Painting & colour

- The **Color** button (Model tab) opens a **hexagon colour wheel** + a row of greys/white, a hex
  field, an **Apply** button, and a **"Click object to apply color"** toggle. Picking a swatch sets
  the builder's *active colour*; **Apply** paints the **whole selected mesh** that colour.
  (Media `30-color-palette.png`.)
- **Paint tool** = `B` (status shows *"Paint material"*). With the active colour, clicking a face
  paints. **Important, verified:** on a mesh with **0 material slots** (a plain inserted part),
  painting a face recolours the **entire mesh** — colour is per-mesh until you add material slots.
  To paint **individual** faces you must first give the mesh material slots (Materials panel, in the
  MST/Modeler layout) and assign a material per face. (Media `31-paint-blue/green/red.png` — each
  click turned the whole crate one colour.) *Writer: present this honestly — single-face painting is
  a materials-panel feature, not a one-click thing on a fresh part.*
- **Vertex paint** = `5` (Roblox: `Alt+5`) — Vertex paint tool.
- From the right-click context menu: **Paint colour** and **Assign material** (both in the *Paint*
  section).

## 4. UV editor (`U` / the **UV** button / Inspector → Open UV Editor)

A dedicated **Pixel Builder – UV Editor** window (resizable). (Media `40-uv-editor.png`,
`41-uv-before.png`.) Controls, with verified tooltips:

- **Faces / Verts** — UV selection mode (click / box-select faces or corners; double-click a face
  selects its UV island).
- Projection: **Manual** *(bake the automatic projection into per-corner UVs)* · **Auto** *(back to
  the automatic world/box projection)* · **Planar** *(project as one island along the average
  normal, fit to 0-1)* · **Box** *(each face along its dominant axis, fit to 0-1)* · **Spherical**
  *(longitude/latitude around the centre)*.
- **Flip U / Flip V** (mirror the projection), **Rot 90** (rotate UVs 90° about the selection
  centre), **Fit / Fit U / Fit V** (fit to 0-1, uniform or per-axis), **Scale** (by U/V factors),
  **Frame / Frame Sel** (frame the 0-1 square / the selection).
- **Sew** *(corners sharing a vertex move to their average UV)*, **Copy / Paste** *(copy the first
  selected face's layout; paste onto faces with the same corner count)*, **Texel** *(match texel
  density to the first selected face)*, **Island** *(select the connected UV island)*.
- **Lock 0-1** *(keep moved/scaled islands inside the 0-1 square)*, **Split** *(grab one corner,
  tearing the seam)*, **Snap** + **Step** field, **Tile** *(texture-preview tiling)*.
- **Auto UVs** row: a **Fill** mode dropdown *(Tile = world-density repeats · Fit = longer side spans
  0-1 · Stretch = both sides to 0-1)*, an **anchor** dropdown (No anchor…), **Flip U / Flip V /
  Swap UV / Group** checkboxes *(Group = one projection across all faces)*, **Apply Auto**.
- **Template:** size (1024), **Grid** / **Transparent** checkboxes, **Export UV Template…**.
- **Canvas gestures (from the in-window help):** *Drag = move · Ctrl+drag = scale (Shift = per
  axis) · Alt+drag = rotate · Middle = pan · Wheel = zoom · Double-click = island · Esc = cancel.
  Ctrl+click a face = stitch it to the first selected face · Ctrl+Shift+click = copy that face's
  layout onto it.* (This is the move/scale/rotate the brief asks for.)
- Status line: "N faces (M manual), K corners selected".

## 5. Export (right-click → **Export ▸**, or the backend `debug_export`)

The context menu's **Export** section (all verified present & enabled): **OBJ…**, **glTF…**,
**STL…**, **PLY…**, **Godot Mesh…**, and **Options…**. Both GLB and STL exports **succeeded** when
driven (wrote a 10 KB `.glb` and a 3.5 KB `.stl`).

**Builder export options** dialog (Export ▸ Options…; media `42-export-options.png`). Header:
*"Every option applies to Export OBJ / STL / PLY; glTF and Godot Mesh use selected faces, world
transforms and combine."* Checkboxes: **Selected faces only**, **Apply world transforms**, **Vertex
colours**, **Right-handed (Godot axes)** (on by default), **Combine selected builder meshes**, **STL
binary** (on by default). OK.

## 6. The Options dialog — **Pixel Builder Preferences** (every preference)

(Media `50-preferences.png`.) Opened from the builder; **Reset to Defaults** at the bottom. Fields
(default shown · tooltip):

| Field | Default | What it does |
|---|---|---|
| Vertex size (px) | 6 | Size of the vertex handles in Vertex mode |
| Handle size | 1.06 | Scale of the move/rotate/scale handles (1 = 64 px arms) |
| Grid step | 0.25 | Grid snap step the builder starts with (`[`/`]` halve/double while editing) |
| Rotate snap (°) | 16 | Rotate-handle snap step while grid snapping is on (0 = free) |
| Grid snap on | ✓ | Start with grid snapping enabled |
| Lightmap texel | 0.25 | Texel size for lightmap UV2 unwrap on Bake / glTF export (0 = skip) |
| Shift selects | Layout default | Shift+click/box = Add / Subtract / Toggle (MST toggles, Modeler adds) |
| Box selects | Layout default | Intersect (box touches) vs Complete (fully inside); MST=Complete, Modeler=Intersect |
| Hover highlight | ✓ | Highlight the face/edge/vertex under the cursor |
| Show scene info | ✓ | Vertex/edge/face + selected counts in the viewport |
| Show dimensions | ✗ | Size of the selection's bounding box |
| Adjust panel | ✓ | Show the live Adjust panel after Extrude/Inset/Bevel/… (off = apply at once) |
| Roblox rotate step (°) | 45 | Rotate-snap step of the Roblox ribbon's Snap group |
| Roblox rotate snap on | ✓ | The Roblox ribbon's rotate-snap checkbox |
| Collisions | ✗ | Roblox drag: a dragged part stops at the first part it would overlap |
| Roblox compact ribbon | ✗ | Roblox ribbon shows icons only (labels → tooltips), Model tab fits one row |
| Edges / Selected edges / Hovered edge | colour | Wireframe / selected / hovered edge colours |
| Vertices / Selected vertices / Hovered vertex | colour | Vertex-handle colours |
| Selected faces | colour | Tint of selected faces |

## 7. Context menu (right-click a builder mesh) — full list

`Duplicate · Delete · Group as Model (Ctrl+G) · Group as Folder (Ctrl+Alt+G) · Ungroup (Ctrl+U) ·
Select children · Union (Ctrl+Shift+G) · Intersect (Ctrl+Shift+I) · Negate (Ctrl+Shift+N) · Separate
(Ctrl+Shift+U) · Zoom to · Lock · Make Editable · UV Editor… (U)` and sectioned:
**Build:** Subdivide, Triangulate (Ctrl+T) · **Join:** Weld · **Paint:** Assign material, Paint
colour · **Normals:** Flip normals, Conform normals · **Object:** Center pivot, Freeze transform,
Mirror, Merge objects, Collider, Trigger, Generate lightmap UVs, Create reusable part, Bake to
MeshInstance3D · **Export:** OBJ…, glTF…, STL…, PLY…, Godot Mesh…, Options….
(Note: the right-click gesture in the viewport did not pop this menu in the harness; the entries
were read from the plugin. The same actions are on the ribbon and the Builder menu.)

## 8. Inspector — Part & Editable Mesh (right dock, Roblox layout)

Selecting a builder mesh shows: **Part — Appearance** (Color, Material, Transparency), **Part —
Transform** (Size x/y/z — edits the recipe; Position, Orientation, Pivot Offset), **Part** (Shape,
Locked, Anchored ▸ terrain), **Behavior** (Collision: None/…), **Editable Mesh** (**Mesh:** "N
vertices, M edges, K faces", material-slot count), **Shape** *(with the rebuild warning: "Rebuilding
replaces the element edits. Undo can restore them." + **Edit Shape**)*, **UV & Lightmap** (Open UV
Editor, Generate Lightmap UVs), **Collision** (Current: Detail · Set Collider · Set Trigger).

> **Rebuild warning (brief item).** Changing **Size** (W/H/D) on a mesh you've already edited
> *rebuilds* the box, which **replaces your element edits**; the Inspector's Shape section warns
> "Rebuilding replaces the element edits. Undo can restore them." So edit the box shape (Size)
> **before** extruding/insetting, or be ready to Ctrl+Z.

---

## 9. Smooth vs faceted, collision, and the two other layouts

### Smooth vs faceted (MST/Modeler **Smoothing** panel)
- **Smooth** — smooth (soft) shading for the selected faces. Verified: "Pixel Builder: Smooth".
- **Faceted** — *"flat shading for the selected faces (group 0)"*. Verified: "Pixel Builder: Faceted".
- **New group** — *"the selected faces get the next free group (1-23)"* — a new smoothing group.
- **Tint groups** checkbox — tint faces by smoothing group (each group in its chip colour).
- **Normals** checkbox — show a normal line at every face corner.

### Collision
- Inspector **Collision** section: *Current: Detail*, **Set Collider**, **Set Trigger**; and a
  **Collision: On** checkbox lower down. The context-menu / **+ Preset** also offer **Collider** and
  **Trigger** material presets. (`entity_type` property holds the collision kind.)

### Layout A — **MST** (the classic builder; Roblox UI OFF, setting = MST)
(Media `60-mst-layout.png`, `61-mst-materials-panel.png`, `62-mst-left-toolbar.png`,
`63-mst-preset-menu.png`.) A **left vertical toolbar** + a **right panel** + a **bottom strip**:
- **Left toolbar** — **Mode:** Object `1` · Face `2` · Edge `3` · Vertex `4`. **Tool:** Select/move
  `W` · Draw shape `C` *(drag a footprint on the grid, move up for height)* · Paint `B` · Vertex
  paint `5` · Place on cell `P` · Bezier plane/block `Shift+C`. **Edit:** Subdivide · Triangulate
  `Ctrl+T` · Flip normals · Weld.
- **Right panel — Materials:** **+ Slot** *(add a vertex-colour StandardMaterial3D you can
  replace)* · **+ Preset** *(Grid, Checker, No Draw, Collider, Trigger)* · **Palette** *(save/load
  the slots' materials + swatches as JSON)* · **Sample** *(make the most-used slot/colour/group of
  the selected faces current)*. Then **Smoothing** (above) and **Face UV** (Offset U/V, Scale U/V,
  Rotation, World aligned, Apply / Reset / ↻90°).
- **Bottom strip:** **Slot** number · **Color** swatch row · **Grid** step · **Subdiv** count ·
  **Weld dist** · **Mirror X/Y/Z** + **Mirror copy**.

### Layout B — **Modeler** (the "larger palette")
(Media `70-modeler-toolbar.png`, `71-modeler-add-mesh.png`.) No left toolbar / right panel; instead
a **horizontal top toolbar**: **+ Add Mesh** *(a shape-grid palette: Box, Prism, Sprite, Plane,
Cylinder, Cone, Sphere, Pipe, Torus, Arch, Door, Stairs, Poly Shape, Bezier Plane, Bezier Block,
Icosphere, Ramp, Arch (wall), Hex prism, Hex cluster, Curved stairs, Trim, Wall; Alt-click a shape
to place it at once)*, **Actions** *(a menu bundling Subdivide/Triangulate Faces, Merge Objects,
Flip Faces, Conform Normals, Center Pivot, Freeze Transform, Mirror, Set Collider/Trigger, Generate
Lightmap UVs, Create Reusable Part, UV Editor…, Export Mesh, Boolean, Place on cell, Weld, Assign
material, New blend material, Invert alpha, Bake to MeshInstance3D, Shape parameters, Paint colour)*,
and icon buttons. Operations (Paint etc.) use **floating popups** in the viewport
("Paint — Click or drag over faces… Options / Cancel / Done") rather than a docked strip.

### Switching layouts
`debug_set_layout 0=MST 1=Modeler`, or the **Builder** menu / **Roblox UI** toggle for Roblox.
**Verified:** switching MST → Modeler → MST kept the mode, the selection, and the **full undo
history** (action "Rotate UVs 90", `has_undo` still true). The chosen layout is saved in Editor
Settings and survives a restart.

### Painting modifiers (MST/Modeler Paint tool `B`) — full tooltip
*"Click or drag over faces to give them the current material slot and color. **Shift+click fills the
coplanar region, Ctrl+click picks the slot of a face.**"* (Brief's Shift+click flat-fill /
Ctrl+click pick — both confirmed in the tooltip.) **Sample** does the same pick for the whole
selection.

---

## 10. Roblox bridges

- **Studio has no mesh editing.** In Roblox you place Parts and resize/rotate them, and the closest
  thing to "extrude" is the one-sided Scale drag. This addon's **Edit** tab adds real mesh editing
  (faces/edges/vertices, extrude/inset/bevel/loop-cut) that Studio simply doesn't have — closer to
  **Blender**'s edit mode than to Studio.
- **Modes** are like Blender's Tab→vertex/edge/face select; Roblox has nothing equivalent.
- **Union / Negate / Separate** map to Roblox's **Unions / Negate parts** (Model tab), but here the
  result is an editable mesh, not a CSG part.
- **MeshParts:** Studio imports finished meshes from Blender; here you can **Export** the mesh to
  glTF/OBJ/STL and bring it back as a MeshInstance3D, or keep editing it live.
- **Materials/Color:** Roblox sets a Part's Material+Color in Properties; here a plain part is one
  colour until you add **material slots** (then paint per-face), and **+ Preset** gives quick
  Grid/Checker/Collider/Trigger materials.

## 11. Caveats (for the writer)

- **Builder meshes render pale/translucent in the busy `hex_island`/terrain scenes** (bright
  sky/bloom washes a plain part to near-white; selected meshes show a translucent blue overlay). For
  clean modelling shots we built the prop in a **new empty scene** (neutral dark background), where
  parts render as solid shaded faces. Writer: if a reader's part "disappears", it's contrast, not a
  bug — add a colour or work on a plain background.
- **0 material slots ⇒ colour is per-mesh.** A one-click paint on a fresh part recolours the whole
  thing. Per-face colour needs material slots first (Materials panel → **+ Slot** / **+ Preset**).
- **Keyboard shortcuts need the mouse over the 3D view** (standard Godot). Teach this early.
- **In the Roblox layout, `F` is Zoom-to and Fill is `Alt+F`;** the bare digits are tools, modes are
  `Alt+1..4`. In MST/Modeler the digits are the modes and `F` is Fill.
- **Changing Size (W/H/D) rebuilds the mesh and replaces element edits** ("Undo can restore them").
  Shape the box before editing it.
- Cosmetic: the editor emits harmless `draw_rect() "width" … has no effect` warnings
  (Godot `canvas_item.cpp`, not the addon). No `mst`/`PixelBuilder` ERROR or panic in the log.
- The in-engine `capture` screenshot occasionally hung after a layout switch; `shot` (full-screen)
  always worked — not a user-facing issue.
- The right-click context menu did not pop in the harness; its entries were read from the plugin and
  are also reachable from the ribbon / Builder menu / Modeler **Actions**.

## 12. Scenes created

- **`res://tutorial/guide/modelled_prop.tscn`** — the "terracotta/painted crate": a `PixelBuilderMesh`
  root with a **Box** child (36 verts, 68 edges, 34 faces) modelled by extrude → inset → recess →
  bevelled rim, then painted. Self-contained (embedded `PixelBuilderMeshData` sub-resources + a
  packed mesh blob; no external `.res`/`.tres`). Copied into the repo at
  **`tutorial/guide/modelled_prop.tscn`**.
- **`res://tutorial/guide/modelled_prop.glb`** — the same prop exported as GLB (10 KB) into the
  session project (export verified: GLB + STL both wrote successfully).

## 13. Media manifest

| File | Kind | Shows |
|---|---|---|
| 01-edit-ribbon.png | shot/tour | The whole Edit-tab ribbon (Object lit, ops greyed in Object mode) |
| 02-mode-face/edge/vertex.png | series | The same ribbon with Face / Edge / Vertex mode lit (mode series) |
| 03-edit-ribbon-face.png | shot | Edit ribbon with Face mode + a face selected → Extrude/Inset/Delete/Pivot enabled |
| 04-roblox-model-tab.png | shot | The Roblox **Model** tab ribbon (tools, Geometric, grid/rot steps, parts/booleans) |
| 10-walk-start.png | shot | Walkthrough start: the terracotta Box part |
| 11-adjust-extrude.png | shot | The **Adjust Extrude** panel (Dist, method, Done) |
| 12-extrude-before.png / 12-extrude-after.png | before/after | Box before vs after extruding the top face up |
| 13-adjust-inset.png | shot | The **Adjust Inset** panel (Inset, Inset by, Individual) |
| 13-inset-after.png | shot | Top face after inset (rim) |
| 14-crate-hollow.png | shot | After extruding the inset face down (hollow/recess) |
| 15-prop-beveled.png | shot | After bevelling the rim (rounded lip) |
| 16-crate-painted.png | shot | The finished crate (clean, centred) |
| 20-select-loop.png | shot | The top rim edge-loop selected (Select Loop `L`) |
| 21-adjust-bevel.png | shot | The **Adjust Bevel** panel (amount, type, Segs, profile, clamp, harden) |
| 22-loop-cut.png | shot | Loop Cut on a side edge + the **Cuts** adjust |
| 30-color-palette.png | shot | The Color palette (hex wheel, hex field, Apply, "click object" toggle) |
| 31-paint-blue/green/red.png | series | Painting recolours the whole 0-slot mesh (blue/green/red) |
| 40-uv-editor.png | shot/tour | The full UV editor window (all tools + canvas + help text) |
| 41-uv-before.png | shot | The UV canvas (unwrapped box faces) |
| 42-export-options.png | shot | The Builder export options dialog (6 checkboxes) |
| 50-preferences.png | shot/tour | The Pixel Builder Preferences (Options) dialog, every field |
| 60-mst-layout.png | shot | The whole MST layout (left toolbar + right panel + bottom strip) |
| 61-mst-materials-panel.png | shot | The MST Materials/Smoothing/Face-UV panel |
| 62-mst-left-toolbar.png | shot | The MST left toolbar (Mode / Tool / Edit sections) |
| 63-mst-preset-menu.png | shot | The **+ Preset** menu: Grid / Checker / No Draw / Collider / Trigger |
| 70-modeler-toolbar.png | shot | The Modeler top toolbar (+ Add Mesh, Actions, icons) |
| 71-modeler-add-mesh.png | shot/tour | The Modeler **Add Mesh** shape grid (23 shapes) |

### Hotspot tour for `01-edit-ribbon.png` (crop = full ribbon, 1425×109; % of crop)
Buttons left→right, centres (approx, as % width): Object 3%, Face 9%, Edge 15%, Vertex 22%,
Extrude 30%, Inset 37%, Bevel 43%, Loop Cut 50%, Bridge 57%, Merge 63%, Delete 69%, Select 78%,
Pivot to Sel. 89%, UV 98% (all at ~45% height).

---

## 14. Clips (video) — added by the video pass

26 short MP4 clips (3–12 s each, H.264, browser-playable) with poster PNGs live alongside the
stills in `docs/guide/media/10-builder-modeller/`. They were captured in a **fresh empty scene**
(neutral dark background) with a **2.5 m terracotta cube** (`PixelBuilderMesh` → a Box part,
Size 2.5³, hex `c8743c`) so every operation reads clearly against the mesh — the pale/translucent
look of the busy `hex_island` scene (see Caveats) is avoided. Most single-operation clips were
**undone afterwards** so each starts from the pristine cube. Modes/selection/operations are shown
in the **Roblox UI → Edit tab**; painting, the material preset and smooth/faceted are shown in the
**MST layout** (where the Materials/Smoothing panels live). Each clip has a matching `.png` poster
(same basename).

| Clip (mp4 + png poster) | Kind | Demonstrates (step / §) |
|---|---|---|
| clip-01-modes | clip | **Edit tab** — clicking **Object / Face / Edge / Vertex** (the 1–4 selection modes); the ribbon button lights, the status line and the mesh handles change. (§1a) |
| clip-02-mode-cycle-h | clip | Pressing **H** over the 3D view **cycles** the modes (Object→Vertex→Edge→Face); ribbon follows. Mouse must be over the 3D view. (§2) |
| clip-03-select-face | clip | **Face mode** — click a face to select; **Shift-click** adds more (top + two side faces light up). (§1a Face) |
| clip-04-select-edge | clip | **Edge mode** — click an edge; Shift-click adds (three top edges). (§1a Edge) |
| clip-05-select-vertex | clip | **Vertex mode** — click corner vertices; Shift-add (white vertex handles). (§1a Vertex) |
| clip-06-move-w | clip | **Move tool (W)** — drag the **green Y handle** to lift the selected top face; live `(0, 1, 0)` readout, box stretches taller. (§9 tool) |
| clip-07-extrude-e | clip | **Extrude (E)** on the top face → pushes it up + the **Adjust Extrude** panel (Dist / method / Done). (Walkthrough step 5; §2 Extrude) |
| clip-08-inset-i | clip | **Inset (I)** on the top face → inner rim + **Adjust Inset** panel (Inset / Inset by / Individual). (Walkthrough step 6; §2 Inset) |
| clip-09-bevel | clip | **Bevel (Ctrl+B)** on the top **edge-loop** (selected with L) → chamfered rim + **Adjust Bevel** panel (amount / Segs / profile / Clamp / Harden). (Walkthrough step 9; §2 Bevel) |
| clip-10-loopcut | clip | **Loop Cut (Ctrl+R)** → inserts an edge loop around the quad ring + the **Cuts** adjust. (§2 Loop Cut) |
| clip-11-triangulate | clip | **Triangulate (Ctrl+T)** → the selected top quad splits into two triangles (6→7 faces). (§7 Build) |
| clip-12-merge | clip | **Merge (M)** → two adjacent selected faces become one n-gon (6→5 faces). (§1a Merge) |
| clip-13-connect | clip | **Connect (J)** → a new edge joins two selected vertices across the top face (12→13 edges). (§2) |
| clip-14-fill | clip | **Fill** → with the open-top boundary loop selected, **Alt+F** (Roblox layout) closes the hole (5→6 faces) + Adjust Fill-hole panel. (§2) |
| clip-15-duplicate-delete | clip | **Duplicate (Ctrl+D)** the selected face then **Delete** it — the status face-count runs **6 → 7 → 6** (crop includes the status line). (§2) |
| clip-16-grow-select | clip | **Grow selection (G)** → one face grows to five (top + four sides); status `selected: 1 → 5 faces`. (§1a Select ▾) |
| clip-17-uv-editor | clip/tour | **UV editor** window (U) — Faces/Verts toggle, projection buttons, **Rot 90**, canvas **zoom / pan / Frame**; the on-canvas gesture legend reads *Drag = move · Ctrl+drag = scale · Alt+drag = rotate*. (§4) |
| clip-18-material-preset | clip | **MST Materials panel** — **+ Preset → Grid** adds a material slot (preview sphere) and textures the mesh. (§9 Layout A Materials) |
| clip-19-paint-face | clip | **Paint tool (B)** — paint one face **blue**, another **green** (per-face, because a material slot exists); **Ctrl+click** picks a face's colour (the Color chip updates). (§3) |
| clip-20-vertex-paint | clip | **Vertex paint (5)** — the **soft radial brush** over the surface + its bottom-strip controls (Apply = Brush, Channel = RGBA, Radius, Strength). (§3) |
| clip-21-smooth-faceted | clip | **Smoothing panel** — toggling **Smooth** (soft/gradient shading) vs **Faceted** (flat) on the selected faces. (§9 Smoothing) |
| clip-22-crate-extrude | clip | **Crate walkthrough 1/4** — Face mode, select top, **Extrude up** → the tall crate body. |
| clip-23-crate-inset | clip | **Crate 2/4** — **Inset** the top face → the lid rim (Adjust Inset panel). |
| clip-24-crate-recess | clip | **Crate 3/4** — **Extrude the inset face down** (negative Dist, typed −1.4) → hollows the open crate. |
| clip-25-crate-bevel | clip | **Crate 4/4** — Edge mode, select the **outer top rim loop (L)**, **Bevel (Ctrl+B)** → rounded lip; finished open crate (24 v / 44 e / 22 f). |
| clip-26-edit-tab | clip/tour | **The Roblox UI Edit tab in use** — a face selected; the mouse pans across the ribbon (Extrude/Inset/Delete enabled, Bevel/Loop Cut/Bridge/Merge greyed for Face mode), then clicks the mode buttons. (§1a) |

### Capture notes for the video pass (for the writer / future agents)
- **`qa/record.sh`** region grabs were used (`start --rect X,Y,W,H --fps 12` → drive with `drive.py`
  → `stop` → `poster`). The in-engine `capture` still occasionally **hangs after a layout switch**
  (as the stills pass found); `shot` (full-screen) and `record.sh` were used instead.
- **Edge / vertex / UV picking needs pixel-precise clicks.** Real clicks were aimed with
  `drive.py project --point '[x,y,z]'` / `project --path Box` (face/edge/vertex window coords) and
  verified with `plugin-call debug_hover` + `debug_hover_state` (which returns the hovered
  `edge`/`face`/`vert` and the `N verts / M edges / K faces | selected: …` line). On the default
  camera the cube's edges are only a few px wide, so eyeballed coordinates miss.
- **Per-face painting confirmed to need a material slot first** (matches the existing Caveat): on a
  0-slot part the paint tool recolours the whole mesh; after **+ Preset → Grid** (1 slot) each
  face paints independently (clip-19).
- **UV island drag** (move/scale/rotate by dragging) needs a *manual* (baked) face **and** corner
  selection; on a plain untextured 0-1 square the live drag doesn't read on screen, so clip-17
  documents the editor UI + the gesture legend (which names move/scale/rotate) and shows
  zoom/pan/Rot 90 instead of a drag. No defect — just a capture limitation on an untextured face.
- **Object-level gizmo drags** (dragging the red/green/blue arrows of a whole part) were unreliable
  to hit with blind coordinates; the duplicate+delete clip (15) therefore proves the op via the
  **status face-count** (6→7→6) rather than a visible lift. Face-level handle drags (clip-06) do
  read clearly.
- **Self-inflicted log line (NOT a defect):** `ERROR: Can't save PNG at path: '/dev/null'` appears
  once in `editor.log` — it was a `drive.py capture /dev/null` probe the video agent ran, not an
  addon error. No `mst`/`PixelBuilder` ERROR or panic from the operations themselves.
- **No scene was saved/overwritten.** The existing `tutorial/guide/modelled_prop.tscn` (from the
  stills pass) was left untouched; the video pass built its own throwaway crate in the session
  project only.

---

## Re-capture (review pass, 2026-11-24, display :104 / port 8904)

- **Verified good, left as-is:** `61-mst-materials-panel.png` (the whole MST right panel, Materials → Smoothing → Face UV, 2x).
- **Re-shot (crop now 900x556 from the viewport's top-left, so the full status line is inside):**
  `10-walk-start`, `12-extrude-before/after`, `13-inset-after`, `14-crate-hollow`, `15-prop-beveled`,
  `16-crate-painted`. The crate is rebuilt live from an empty scene (Ctrl+N → Other Node → `PixelBuilderMesh`
  → Roblox UI → Part → Color `c8743c`) and then extruded up 1.0, inset 0.2 (Fraction), extruded −0.7, rim
  loop + Bevel Segs 3 / 0.22, painted `33cc33`. Newly inserted parts are now solid Medium Stone Grey (the addon
  no longer copies the selected mesh's material), so the walkthrough colours the part explicitly; faces created
  by Extrude/Inset come out grey, which the new stills show.
- **`21-adjust-bevel.png`** re-cropped from a full-screen shot taken with the panel live (Bevel 0.22 / Offset /
  Segs 3 / B profile 0.51 / Clamp / summary `Bevel · 3 segments · Profile 0.50 · Offset 0.220`).
- **`clip-24-crate-recess` re-recorded** (7.3 s): Dist typed as **−0.72**, the face sinks, panel + status visible.
  Poster at 4.5 s.
- **`clip-25-crate-bevel` re-recorded** (9.8 s): rim loop, Bevel 0.22, Segs 3 — the whole Adjust Bevel panel and
  the status line (`3 segments · Profile 0.50 · Offset 0.220`) are in frame. Poster at 6 s.
- **Technique notes:** the Adjust panels are driven with `plugin-call debug_action_adjust <Action id>` /
  `debug_adjust_field <Field id> <value> <flush>` / `debug_adjust_end false` (Action 8 Extrude, 9 Inset, 10 Bevel,
  37 Select Loop, 2 ModeEdge; Field 9 Distance, 10 Inset, 11 Bevel, 13 BevelSegs). Pixel clicks into the Roblox
  ribbon are unavoidable for Part/Color (they are custom-drawn, not Controls).
- **Recipe:** `docs/guide/recipes/10-builder-modeller.json` — `crate-walkthrough-stills` and `crate-clips` replay
  the above and were verified with `qa/recapture.py` on :114/8914. The remaining items are backfill placeholders
  (see each item's `notes`).

## Recipes (2026-11-24, display :118 / port 8918)

Status of `docs/guide/recipes/10-builder-modeller.json` (session 1920×1080):

| Item | Outputs | State |
|---|---|---|
| `crate-walkthrough-stills` | 10/12/13/14/15/16/20/21 | **green** (verified earlier and replays) |
| `crate-clips` | clip-22..25 | **green** (verified earlier and replays) |
| `panels-and-dialogs` | 11, 13-adjust, 22-loop-cut, 30-color-palette, 42-export-options, 50-preferences | **placeholder** — opening each dialog needs pixel coordinates into the custom-drawn ribbon (Part/Color/Export) plus the Adjust panels, and the 1920 window layout differs from the pass that shipped the stills; not re-validated this pass |
| `shortcut-clips` | clip-01..16, clip-26 | **placeholder** — 17 clips on one session; the recording/replay pairs are 3–12 s each and an editor pause stretches them, so they need the original one-at-a-time session; not re-validated |
| `uv-editor` | 40, 41, clip-17 | **placeholder** — needs the UV window position for the 1920 layout; not re-validated |
| `painting` | 31-paint-blue/green/red, clip-18..21 | **placeholder** — the per-face paint needs a material slot first (+Preset) and MST panel coordinates; not re-validated |
| `mst-layout` | 60, 61, 62, 63 | **green** on :118/8918 — modelled_prop + `debug_set_layout 0` + F, crops measured at 1920 (60 = (0,80,1920,870) → 1500×680; 61 = 2× (1366,128,258,528); 62 = 2× (310,160,48,680); 63 = crop after +Preset at (1438,198)) |
| `modeler-layout` | 70, 71 | **green** on :118/8918 — `debug_set_layout 1`, toolbar crop (287,104,1216,44), + Add Mesh at (990,125), palette crop (958,150,420,505) |
| `ribbon` | 01, 02×3, 03, 04 | **green** on :118/8918 — modelled_prop + `debug_set_layout 2`, Edit tab (373,140), mode buttons at (396/470/554,185), face clicked at (900,450), Model tab (325,140); crops (287,130,1216,109) include the Model/Edit tab row |

Notes for the next pass: `debug_set_layout n`, `debug_action(token_id)` / `debug_act(name)`,
`debug_action_adjust`, `debug_adjust_field`, `debug_preferences`, `debug_export_options`,
`debug_modeler_open` and `debug_uv_*` exist on `PixelBuilderPlugin` and drive the ops without pixel
clicks; `debug_shortcuts` returns the action names (mode_object/face/edge/vertex, extrude, inset,
bevel, insert_loop, merge, connect, fill, triangulate, duplicate, delete, grow, shrink,
roblox_fill, …). The ribbon band at 1920 is x287 w1425, y≈120 h109 (the shipped crops are exactly
that size).

## Recipes (2026-11-24, v1012, display :121 / port 8921)

`docs/guide/recipes/10-builder-modeller.json`, page session 1920x1080. Everything below was
replayed from a cold session with `python3 qa/recapture.py ... --only <id> --out-dir <scratch>
--display :121 --port 8921` and the outputs compared with `docs/guide/media/10-builder-modeller/`.

| Item | Outputs | State |
|---|---|---|
| `crate-walkthrough-stills` | 10/12/13/14/15/16/20/21 | green (unchanged) |
| `crate-clips` | clip-22..25 | green (unchanged) |
| `panels-and-dialogs` | 11, 13-adjust, 22-loop-cut, 30-color-palette, 42-export-options, 50-preferences | **green** — all six panels are now opened through the debug API instead of pixel clicks into the custom-drawn ribbon: `debug_action 1` (ModeFace), `debug_action_adjust 8/9/35` (Extrude / Inset / Insert edge loop), `debug_action 80` (ExportOptions), `debug_preferences`. Values go through `debug_adjust_field` (field 9 = Dist, 10 = Inset). The two Adjust panels are cropped at the live `PixelBuilderAdjustPanel` rect (queried from `ui` in a shell step, so a panel that grows a row cannot clip the crop) and 2x upscaled; the colour popup (932,246,289,381), export dialog (700,368,518,320) and preferences (728,212,468,656) use fixed rects measured on this layout. |
| `uv-editor` | 40, 41, clip-17 | **green** — window opened with `debug_action 46` (same code as the U button); it is a `Window` at (550,230) size 820x620, so 40 is that rect and 41 is the canvas band (562,460,805,285). Wheel-out 6 notches fits the box's islands. **Gotcha:** a freshly opened UV window ignores every click/wheel until its title bar is clicked — the clip therefore starts with a click at (700,242), then selects the square island (828,558) and presses Rot 90. |
| `painting-stills` (new, split out of `painting`) | 31-paint-blue/green/red | **green** — same 0-slot Box; each colour set through the colour-chip popup (hex at 1048,600, Apply at 1188,600), then `debug_action 6` (ToolPaint) and one click on the mesh recolours the whole part; shots are 700x420 crops at (600,300). |
| `painting` (now only the clips) | clip-18..21 | **still not green** — the four MST-layout clips need the Materials panel (+ Preset popup is a `PopupMenu`, so its rows must be clicked by rect from `drive.py popups`), and clip-19 additionally needs two per-face paint clicks after a preset slot exists. Not finished this pass; the shipped media is unchanged and still valid. |
| `shortcut-clips` | clip-01..16, clip-26 | **still not green** — 17 record/poster pairs in one item: any single drifted step fails the whole item, and the selection clips need per-clip pixel picks (edge/vertex picking is only a few px wide on this camera). A next pass should split it into 2-4 items (modes+selects / build ops / paint+layout) and drive the ops with `debug_action`/`debug_select_faces`/`debug_select_edges`/`debug_select_verts` so no pixel picks are needed. |
| `mst-layout`, `modeler-layout`, `ribbon` | 60-71, 01-04 | green (unchanged) |

Useful for the next pass: the Adjust panel rect is not fixed — it grows upward with its field rows
(Extrude 119 px tall at y 638, Inset 117 px at y 670), so always query `PixelBuilderAdjustPanel`.
