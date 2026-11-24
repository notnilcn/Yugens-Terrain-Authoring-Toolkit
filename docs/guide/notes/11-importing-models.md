# Capture notes — page `11-importing-models` (Importing 3D models into Godot)

Area C10. Session `docs-import`, display `:80`, port `8880`. Editor resized to **1920×1080** so the
FileSystem dock, Import dock and the Advanced Import Settings window all have room. Captured from the
real Godot **4.7.2.stable** editor (title bar reads `4.7.2.stable`). Working scene:
`res://tutorial/01_flat_playground.tscn` (a `Playground` scene: `WorldEnvironment`, `Sun`,
`MarchingSquaresTerrain`, `PixelWater`, `Camera`, `PixelRenderer`, `PixelSky`, `PixelDayNight`), plus
a new scene created for instancing imported models.

This page teaches a Roblox-Studio artist how to get a 3D model **file** (a `.glb`, `.obj`, …) into a
Godot project and into a scene — the editor's **import** system, which Roblox has no direct analogue
for (in Studio you use the Toolbox or "Import 3D"; here a model is a **file in the project folder**
that Godot re-processes into a Godot scene every time it changes).

> **Where the real models live.** The addon ships real GLB models under
> `res://addons/MarchingSquaresTerrain/assets/`:
> - `assets/characters/fox/fox.glb` (a rigged, textured fox — several PNG textures beside it)
> - `assets/courtyard/` — `round_tree.glb`, `stone_ring.glb`, `fountain.glb`, `pillar.glb`
> - `assets/rocks/` — ~40 `stone_*.glb` boulders
> These are used as the real import examples. A hand-made `.obj` (written with Python) stands in for a
> "Roblox Studio → Export Selection → .obj" file, and a GLB exported from Godot itself is re-imported.

---

## 0. Key idea for a Roblox artist (read first)

In **Roblox Studio** a mesh lives *inside* your place file; you bring it in with the Toolbox or
"Import 3D", and after that it is just data in the game. In **Godot** a model is a **file that stays
on disk in your project folder** (`res://…/thing.glb`). Godot watches that file and, whenever it
changes, **re-imports** it: it reads the `.glb` and bakes a Godot scene (meshes, materials,
skeleton, animations) into a hidden cache (`.godot/imported/`). You never edit the cache; you change
the **import settings** (the Import dock) or the source file, and Godot rebuilds. A `.glb.import`
text file sits next to every imported model and stores those settings.

So the three things a Godot importer does that Studio does not expose:
1. **Put the file in the project folder** (copy it in, or drag it onto the FileSystem dock).
2. **Set import options** (Import dock + Advanced Import Settings) — how meshes, materials, physics
   and animations are baked.
3. **Instance** the resulting scene into your own scene (drag it from FileSystem into the viewport).

---

## 1. Feature inventory

### 1a. The FileSystem dock (bottom-left) — where files live

The **FileSystem dock** (bottom-left, rect ~[10,599,268,471]) is Godot's project-folder browser.
Everything under `res://` is in your project. A Roblox artist should read `res://` as "the project
folder on disk". Controls, top to bottom:
- **◀ ▶** back / forward (go to previous / next selected folder). Tooltips *"Go to previous selected
  folder/file."* / *"Go to next selected folder/file."*
- **Path field** (`res://…` LineEdit) — the current folder; type a path + Enter to jump there.
- **Split-mode button** (far right, tooltip *"Change Split Mode"*) — toggle the dock between the
  single tree and a tree + file-grid.
- **Filter Files** box — live-filter the listing.
- **"View items as a grid of thumbnails"** button (tooltip verbatim) — thumbnail vs list view.
- The **tree** itself: **Favorites**, then `res://` with every folder. `.glb` files show a little
  **mesh/filmstrip icon**; expanding one (▶) reveals the scene it imports to (meshes, materials,
  animations).
- **Right-click** a folder → **New Folder…**, and files → **Rename / Move / Duplicate / Delete /
  Open in External Program / Copy Path / Show in FileSystem**. (Right-click menu read from the dock;
  New Folder used in the walkthrough to make `res://models/`.)
- Media: `02-filesystem-assets.png` (the dock navigated to `addons/.../assets/courtyard/` showing
  `fountain.glb`, `pillar.glb`, `round_tree.glb`, `stone_ring.glb`).

### 1b. The Import dock (top-left, tab beside **Scene**)

Selecting a model file in the FileSystem dock fills the **Import dock** — a tab **next to Scene** at
the top-left (`09-scene-import-tabs.png`). It shows, in one tall scrolling list, every setting Godot
uses to bake that file into a Godot scene. For a `.glb`/`.gltf`/`.obj`/`.fbx`, **Import As:** is
**Scene** (dropdown, with a **Preset** button for saved presets). Sections and options (all verified
present for `round_tree.glb`; defaults noted):

**Nodes** — `10-import-dock-nodes.png`
| Option | Default | Meaning (beginner) |
|---|---|---|
| Root Type | *(empty = Node3D)* | What node sits at the top of the imported scene |
| Root Name | *(empty = file name)* | Name of that top node |
| Root Script | `<empty>` | Attach a script to the root on import (advanced) |
| **Apply Root Scale** | ✓ On | Multiply the whole model by **Root Scale** on import |
| **Root Scale** | 1.0 | Uniform scale baked into the model — **the fix for "my model is huge/tiny"** |
| Import as Skeleton | ☐ Off | Force the root to be a Skeleton3D (rigged imports) |
| Use Name Suffixes | ✓ On | Honour `-col`, `-noimp`, `-rigid`… suffixes on mesh names (auto-collision etc.) |
| Use Node Types | ✓ On | Let glTF node extensions pick the Godot node type |

**Meshes** — `11-import-dock-meshes.png`
| Option | Default | Meaning |
|---|---|---|
| Ensure Tangents | ✓ On | Generate tangents (needed for normal maps) |
| **Generate LODs** | ✓ On | Build lower-detail versions for distance (auto performance) |
| **Create Shadow Meshes** | ✓ On | A cheaper mesh used only for shadows |
| **Light Baking** | Static | How the mesh participates in baked lightmaps (Disabled / Static / Static Lightmaps / Dynamic) |
| Force Disable Compression | ☐ Off | Turn off mesh compression (compatibility) |

**Skins** — Use Named Skins ✓ On (keep skin names for rigged meshes).

**Animation** — Import ✓ On, **FPS** 30, Trimming ☐ Off, Remove Immutable Tracks ✓ On,
Import Rest as RESET ☐ Off. (For a static prop these do nothing; they matter for the fox.)

**Import Script** — Path (a `.gd` that post-processes the scene; advanced, usually empty).

**Materials** — `12-import-dock-materials-gltf.png`
| Option | Default | Meaning |
|---|---|---|
| **Extract** | Keep Internal | *Keep Internal* = materials live inside the scene; *Extract* writes each as its own file so you can edit it |
| Extract Format | Text (*.tres) | File type when extracting (Text `.tres` vs Binary `.res`) |
| Extract Path | *(empty)* | Folder to write extracted materials into |

**glTF** (only shows for glTF/GLB)
| Option | Default | Meaning |
|---|---|---|
| Naming Version | Godot 4.5 or later | Node-naming scheme for compatibility |
| Embedded Image Handling | Extract Textures | Pull textures baked inside the `.glb` out into image files |
| Texture Map Mode | Remap to Standard | How glTF texture maps map onto Godot's StandardMaterial3D |

**Bottom buttons** — `13-import-dock-reimport-buttons.png`
- **Advanced…** — opens the **Advanced Import Settings** window (per-mesh / per-material, preview).
- **Reimport** — re-bake the file with the current settings (press after you change any option).

> **Big idea for the reader.** You never edit the baked result directly. You change a setting here
> (or edit the source file), press **Reimport**, and Godot rebuilds. A `.glb.import` text file next
> to the model stores these choices (so they travel with the project and survive version control).

### 1c. The Advanced Import Settings window

Press **Advanced…** in the Import dock, **or double-click the model file** in FileSystem → a big
window opens, titled **"Advanced Import Settings for Scene '<file>'"** (`20-advanced-import-scene.png`).
Layout:
- **Left:** a scene/mesh/material tree.
- **Centre:** a live **3D preview** you can orbit (drag) — `21-advanced-import-preview.mp4`. Top-right
  of the preview are small buttons to spin the preview light / toggle preview sun.
- **Right:** the full Import inspector (same settings as the dock, but with **complete, untruncated
  labels** — the easiest place to read them).
- **Top-left: Actions…** menu (`27-advanced-actions-menu.png`) — **Extract Materials**, **Set
  Animation Save Paths**, **Set Mesh Save Paths** (bulk versions of the per-item options).
- **Bottom: Close** (discard) and **Reimport** (apply).

Three tabs select *what* the left tree shows:

**Scene tab** — the node tree (for `round_tree.glb`: `Scene ▸ Tree ▸ Icosphere…Icosphere_003 ▸
Leaves ▸ Stem`). Selecting a node shows **per-node** import settings on the right:
- A plain group node (e.g. `Tree`): **Node** (Node Type, Script), **Import** (**Skip Import** —
  leave this node out). `23-advanced-node-generate.png`
- A **mesh** node (e.g. `Icosphere`): **Import ▸ Skip Import**; **Generate ▸ Physics** (checkbox),
  **NavMesh** (Disabled/…), **Occluder** (Disabled/…); and **Mesh Instance** (render Layers,
  Visibility Range, Cast Shadow). **This is the per-mesh collision control.**
- Turning **Generate ▸ Physics = On** reveals a **Physics** section (`24-advanced-physics-collision.png`):
  **Body Type** (StaticBody3D / RigidBody3D / Area3D / …), **Shape Type** (Automatic / Trimesh /
  Convex / Box / Sphere / Capsule / …), **Physics Material**, collision **Layer** / **Mask**. This
  is how you make an imported prop solid without hand-adding a collision shape.

**Meshes tab** — a flat list of every mesh. Selecting one shows **per-mesh** settings
(`22-advanced-per-mesh.png`): **Save to File ▸ Enabled** (write this mesh as its own `.res`/`.tres`),
**Generate ▸ Shadow Meshes / Lightmap UV / LODs** (each Default / Enabled / Disabled), **LODs ▸
Normal Merge Angle** (20°).

**Materials tab** — a list of the model's materials (for the tree: **Leaves**, **Wood**;
`25-advanced-materials-list.png`). Selecting one previews it on a sphere (`28-material-preview-sphere.png`)
and shows **Use External ▸ Enabled** (`26-advanced-material-external.png`) — turn on to replace the
baked-in material with an external `.tres`/`.material` file of your own (a Path field appears). This
is the "edit the imported material" path: either **Use External** here, or **Materials ▸ Extract** in
the dock, then edit the resulting material file.

### 1d. Instancing an imported model into a scene

An imported `.glb` becomes a **scene** you can drop into your own scene (like dragging a Model out of
Roblox's Toolbox). Four ways, all verified:
1. **Drag from FileSystem into the 3D viewport** (`30-drag-into-viewport.mp4`) — the model lands where
   you drop it (snapped onto whatever surface is under the cursor).
2. **Drag from FileSystem onto a node in the Scene tree** (`42-drag-into-tree.mp4`) — it becomes a
   child of that node, at that node's origin.
3. **Scene ▸ Instantiate Child Scene…** (chain-link icon in the Scene-dock toolbar).
4. **Double-click the file → it opens the imported scene** read-only; or **Scene menu ▸ New
   Inherited Scene…** to make an editable scene that *inherits* the model (so edits to the source
   still flow through).

The instanced model appears as a node (e.g. `round_tree`) whose children are the actual
**MeshInstance3D** nodes (`Tree ▸ Stem`, `Leaves`, …). `43-instanced-tree.png` shows the round tree
instanced into a fresh, well-lit 3D scene.

> **Scale note (verified).** The addon's demo terrain scenes are built at a very large unit scale
> (hundreds of units across), so a real-world-size model (a 3 m tree) looks microscopic there — this
> is contrast/scale, not a broken import. Fix it with **Root Scale** in the Import dock (bake a bigger
> scale into the model) or by scaling the instance (below).

### 1e. Scaling, moving and rotating an instance

The imported model is a normal Node3D, so the editor's transform tools apply (Roblox's Move/Scale/
Rotate):
- **W** = Move, **E** = Rotate, **R** = Scale (hover the 3D view first). Drag the coloured gizmo
  handles. `44-scale-gizmo.mp4` drags the **green (Y) scale handle** to make the tree taller (scale
  Y 1.0 → 2.2); dragging a plane/centre handle scales uniformly.
- Or type exact numbers in the **Inspector ▸ Transform ▸ Scale / Position / Rotation** (right dock).
- **Snapping:** hold the transform; toggle snap with the magnet icons in the 3D toolbar.

> **Roblox bridge.** This is exactly Studio's Move/Scale/Rotate tools, but the gizmo arrows/boxes sit
> on the model's own axes and you can also type precise values in the Inspector (Studio's Properties).

### 1f. Supported formats — what works here (verified)

| Format | Import As (default) | Works in this project? | Notes |
|---|---|---|---|
| **glTF `.gltf` / `.glb`** | **Scene** | ✅ Yes — **recommended** | The addon's own models are `.glb`. Imports meshes, materials, textures, skeleton, animations. Full Import dock + Advanced window. |
| **OBJ `.obj`** | **"OBJ as Mesh"** (a Mesh resource) | ✅ Yes | Different importer: options are Generate Tangents / LODs / Shadow / Lightmap UV2, **Scale Mesh**, **Offset Mesh**. Reads a `.mtl` beside it for colours. Dragging it makes a MeshInstance3D. You can switch **Import As** to *OBJ as Scene*. **This is the Roblox-export format.** |
| **FBX `.fbx`** | Scene (via converter) | ⚠️ Only if configured | Godot shows a **"Configure FBX Importer"** dialog: FBX needs the external **FBX2glTF** tool pointed to in **Editor ▸ Editor Settings ▸ FBX**. Until then, FBX won't import — prefer glTF/OBJ. |
| **Blender `.blend`** | Scene | ⚠️ Only if Blender is installed | Needs **Editor Settings ▸ FileSystem ▸ Import ▸ Blender ▸ Enabled** + the path to a Blender install. Not configured here by default. |
| **Collada `.dae`, STL, PLY** | Scene / Mesh | ✅/➖ | `.dae` imports as a Scene; the addon can *export* STL/PLY but importing them is niche. |

> **For a Roblox artist:** export your model from Studio with **Export Selection → .obj**, drop the
> `.obj` (and its `.mtl`) into your Godot `res://models/` folder, and it imports as a mesh. For
> anything richer (rigs, animation, PBR materials) convert to **glTF** first.

## 2. Walkthroughs

### Walkthrough A — Import a GLB and put it in a scene (the core loop)
1. **Find the model.** In the **FileSystem dock** (bottom-left) open
   `res://addons/MarchingSquaresTerrain/assets/courtyard/` → you see `round_tree.glb` etc.
   (`02-filesystem-assets.png`). (In your own project you'd drag a `.glb` from your file manager onto
   this dock, or copy it into the project folder — see Walkthrough C.)
2. **Check import settings.** Click `round_tree.glb` once → click the **Import** tab (top-left, next
   to **Scene**) → the **Import dock** fills in (`10/11/12-import-dock-*.png`). Leave the defaults for
   now.
3. **Peek inside (optional).** Double-click the file (or press **Advanced…**) → the **Advanced Import
   Settings** window opens with a 3-D preview you can orbit (`20-advanced-import-scene.png`,
   `21-advanced-import-preview.mp4`). Look around, then **Close**.
4. **Instance it.** Switch to a 3-D scene. **Drag `round_tree.glb` from FileSystem into the 3-D
   viewport** (`30-drag-into-viewport.mp4`) — a `round_tree` node appears. (Or drag it onto a node in
   the **Scene** tree — `42-drag-into-tree.mp4`.)
5. **Frame it.** Select the node, hover the 3-D view, press **F** to focus; scroll to zoom. The
   finished instance: `43-instanced-tree.png`.
6. **Scale/move it.** Press **R** (Scale) and drag a handle (`44-scale-gizmo.mp4`), or type values in
   **Inspector ▸ Transform**.

### Walkthrough B — Change an import setting and Reimport
1. Select `round_tree.glb` → **Import** tab.
2. Double-click **Root Scale**, type `2.0`, Enter.
3. Click **Reimport** (bottom of the dock). Godot rebuilds the model and **every instance in every
   scene doubles in size** (`54-reimport-root-scale.mp4`). Set it back to `1.0` + Reimport to undo.
   *(This is how you fix "my imported model is the wrong size" — Root Scale, not scaling each copy.)*

### Walkthrough C — Bring a NEW file into the project + the Roblox OBJ path
1. **Make a models folder.** Right-click `res://` in FileSystem → **Create New ▸ Folder…**
   (`03-create-folder-menu.png`) → name it `models` (`04-create-folder-dialog.png`) → OK.
2. **Copy the file in.** Put your exported file (here a hand-made `roblox_house.obj` + `roblox_house.mtl`,
   standing in for a Studio **Export Selection → .obj**) into the project's `models/` folder. Godot
   detects and imports it; it appears in the dock with a mesh icon (`05-models-folder-imported.png`).
3. **Read its import settings.** Select `roblox_house.obj` → **Import** tab → note it is
   **"OBJ as Mesh"** with **Scale/Offset Mesh** (`51-obj-import-dock.png`).
4. **Instance it.** Drag `roblox_house.obj` into the viewport (`52-obj-instance.mp4`) → a
   `RobloxHouse` MeshInstance3D appears (`53-obj-house-and-tree.png`).

### Walkthrough D — Export a model from Godot and re-import it (round-trip)
1. Open/make a scene with a model (e.g. the tree).
2. **Scene ▸ Export As… ▸ glTF 2.0 Scene…** (`40-scene-menu.png`) → choose
   `res://models/exported_tree.glb` → the **glTF Export Settings** dialog (`50-gltf-export-settings.png`)
   → **OK**. The `.glb` is written **and** auto-imported.
3. It now sits in `res://models/` and can be dragged back into a scene
   (`55-instance-reimported-glb.mp4`) — a full export→import round-trip. (The addon **Builder** can
   likewise export a `.glb`/`.obj`/`.stl`/`.ply` of a mesh you model, which you re-import the same way
   — see page 10.)

### Walkthrough E — Edit an imported model's material (StandardMaterial3D)
1. Select a mesh part of an instance (e.g. `round_tree/Tree/Leaves`).
2. Inspector → expand **Surface Material Override** → set slot **0** to **New StandardMaterial3D**.
3. Click the material → its properties appear (`70-standardmaterial3d-inspector.png`): **Albedo**,
   **Metallic**, **Roughness**, Emission, Normal Map…
4. Expand **Albedo**, click **Color**, pick a colour (`71-material-albedo.mp4`,
   `72-albedo-color-changed.png`) → that mesh part recolours.
   *(glTF models are split into several mesh parts, so an override colours one part. To edit the
   model's **own** baked material instead, use **Materials ▸ Extract** in the Import dock, or
   **Use External** in the Advanced window's Materials tab, then edit the extracted `.tres`.)*

### Walkthrough F — Turn an imported mesh into an editable Builder mesh (addon-specific)
1. Select an imported **MeshInstance3D** (e.g. the OBJ `RobloxHouse`). The **Pixel Builder** toolbar
   appears, status: *"Mesh / CSG selected: Convert makes it an editable builder mesh."*
2. Click **Convert** (or Inspector ▸ **Make Editable**) → the **Convert to builder mesh** dialog
   (`60-convert-to-builder-dialog.png`): **Merge triangles into** Triangles/Quads/**N-gons**, **Keep
   materials**, **Keep smoothing**, **Smoothing angle**.
3. **Convert** (`61-convert-to-builder.mp4`) → the mesh becomes an editable `PixelBuilderMesh` you can
   extrude/inset/bevel (page 10). This is the bridge from *imported file* to *editable builder mesh*.
   (Source: `modeler_import.rs` / `builder/plugin/xg_io.rs::convert_source`.)

## 3. Troubleshooting (brief's list — verified paths)

- **Model too big / too small.** Use **Import dock ▸ Root Scale** (glTF) or **Scale Mesh** (OBJ), then
  **Reimport** — bakes a size into the file so every instance is right (Walkthrough B). The addon's
  terrain demos use very large world units, so a real-size model looks tiny there — scale the model up
  or the instance up.
- **Missing textures / model looks untextured.** Textures must sit **next to the model file** (glTF can
  also embed them). The fox shows the pattern: `fox.glb` with `fox_Diffuse.png`, `fox_Normal.png`… in
  the same folder (`80-textures-next-to-model.png`). If you move the `.glb` without its textures,
  albedo goes flat/white. For OBJ, the `.mtl` must be beside the `.obj`.
- **Model renders dark / faces look inside-out.** Faces with reversed winding/normals shade black.
  Fixes: in the **Builder** (after Convert) use **Flip normals** / **Conform normals**; or re-export
  from the source app with consistent normals. (Our OBJ logged a harmless Godot note *"OBJ: Ambient
  light for material 'Walls' is ignored in PBR"* — that's the `.mtl` `Ka` being dropped, expected, not
  an error.)
- **Wrong up-axis (model lies on its side).** glTF is **Y-up**; models from Z-up tools can come in
  rotated. Fix by rotating the instance −90° on X, or set a **Root Type** and rotate on import; many
  exporters have a "Y-up" option — prefer that.

## 4. Roblox bridges

- **Studio Toolbox / "Import 3D" → Godot's FileSystem + Import dock.** In Studio a mesh lives inside
  your place; in Godot it's a **file on disk** that Godot re-processes. You manage it in the FileSystem
  dock and tune it in the Import dock — there's no in-app "Toolbox", you put files in the folder.
- **Export Selection → .obj** is the direct bridge: Studio exports an `.obj` (+`.mtl`); drop it in
  `res://models/` and it imports as a mesh (Walkthrough C). For rigs/animation/PBR, convert to glTF.
- **Studio Properties (Material/Color) → StandardMaterial3D.** Albedo = Color, plus Metallic/Roughness
  and texture slots. Editing is the same idea as Studio's Properties, in the Inspector.
- **MeshParts → instanced scenes.** A Studio MeshPart is one mesh; a Godot import is a whole little
  scene you instance (and can convert to an editable Builder mesh, which Studio can't do).
- **Scale/Move/Rotate** gizmos are the same tools as Studio's, but you can also type exact Transform
  numbers in the Inspector, and bake a size with **Root Scale** on import.

## 5. Caveats (for the writer)

- **Headless-session import quirk (capture-only, NOT a user bug):** in this automated Xvfb editor,
  files **copied** into the project with `cp` were not auto-imported until a *full* rescan happened
  (triggered here by a **Scene ▸ Export As… glTF**, which rescans). On a real desktop Godot, the editor
  rescans on window focus and imports new files immediately. The writer should present importing as
  automatic (which it is for users); this note is only so a future capture agent knows the workaround.
- **glTF models are split into multiple MeshInstance3D parts** (the tree = Stem + several Icosphere
  canopy pieces + Leaves). A material override recolours **one** part; to recolour the whole thing use
  the Builder's paint/colour after Convert, or edit each part.
- **OBJ imports as a *Mesh*, not a *Scene*,** by default ("OBJ as Mesh"); dragging it still makes a
  MeshInstance3D. glTF imports as a *Scene*. Say this clearly — the Import dock looks different for the
  two.
- **A plain OBJ's `.mtl` ambient (`Ka`) is ignored** (Godot is PBR); colours come from `Kd`. This is a
  one-line Output warning, not an error.
- **Self-inflicted log lines (NOT defects):** one `ERROR: Node3D::rotate_y: Cannot convert argument 1
  from Array to float` came from a `drive.py call` the capture used to pose the fox (a harness
  type-conversion, not the addon). No `mst`/`PixelBuilder` ERROR or panic occurred during any import,
  export, Reimport, or Convert.
- **Editor was run at 1920×1080** so the Advanced Import window and Import dock have room; at 1600×900
  the dock labels truncate (Root Scale, etc.) — the Advanced window shows full labels.

## 6. Scenes created

- **`res://tutorial/guide/imported_models.tscn`** — the deliverable scene for this page: a `Node3D`
  root named **ImportedModels** with two **instanced imported models** — `round_tree` (from
  `addons/.../courtyard/round_tree.glb`) and `fox` (from `addons/.../characters/fox/fox.glb`, a
  textured rigged character, moved to ~(2.6, 0, 1.6)). It references **only addon assets that exist in
  the repo**, so it is portable. Copied into the repo at **`tutorial/guide/imported_models.tscn`**.
  `01-imported-models-hero.png` is a render of it.
- **Session-only test files (NOT copied to the repo, per brief):** `res://models/roblox_house.obj`
  (+`.mtl`) — the simulated Roblox OBJ; `res://models/exported_tree.glb` — the Godot-exported GLB. Both
  live only in the session project `/tmp/opencode/qa/sessions/docs-import/project/models/`.

## 7. Media manifest

49 files (39 PNG + **10 MP4 clips**), captured at 1920×1080 from Godot 4.7.2.

### Clips (MP4 + matching PNG poster)
| Clip | Kind | Demonstrates |
|---|---|---|
| 21-advanced-import-preview | clip | Orbiting the 3-D preview in the **Advanced Import Settings** window. |
| 30-drag-into-viewport | clip | **Dragging `round_tree.glb` from FileSystem into the 3-D viewport** → it instances. |
| 42-drag-into-tree | clip | Dragging a model onto a node in the **Scene tree** (becomes a child at origin). |
| 44-scale-gizmo | clip | **Scale tool (R)** — dragging the green Y handle makes the tree taller (scale Y 1→2.2). |
| 52-obj-instance | clip | Dragging the **Roblox `roblox_house.obj`** into the viewport → a `RobloxHouse` MeshInstance3D. |
| 54-reimport-root-scale | clip | Changing **Root Scale 1→2** + **Reimport** → every instance doubles live (the import loop). |
| 55-instance-reimported-glb | clip | Instancing `exported_tree.glb` — the **Godot-exported GLB re-imported** (round-trip). |
| 61-convert-to-builder | clip | **Convert to builder mesh** — the OBJ mesh becomes an editable `PixelBuilderMesh`. |
| 71-material-albedo | clip | Editing a **StandardMaterial3D Albedo colour** (type hex) → the leaf mesh recolours. |
| 82-scene-orbit | clip | Orbiting the finished scene: the GLB tree + the textured fox character. |

### Stills
| File | Kind | Shows |
|---|---|---|
| 01-imported-models-hero | shot | The deliverable scene: round tree + textured fox, instanced & lit. |
| 02-filesystem-assets | shot | FileSystem dock at `assets/courtyard/` (the `.glb` models). |
| 03-create-folder-menu | shot | Right-click **Create New ▸ Folder/Scene/Script/…** submenu. |
| 04-create-folder-dialog | shot | The **Create Folder** dialog (Base path res://, Name field). |
| 05-models-folder-imported | shot | `res://models/` with the imported `exported_tree.glb` + `roblox_house.obj`. |
| 09-scene-import-tabs | shot | The **Scene / Import** tab switcher (Import tab active). |
| 10-import-dock-nodes | shot/tour | Import dock — Import As + **Nodes** section (Root Scale etc.). |
| 11-import-dock-meshes | shot | Import dock — **Meshes / Skins** (LODs, Shadow, Light Baking). |
| 12-import-dock-materials-gltf | shot | Import dock — **Animation / Materials / glTF** sections. |
| 13-import-dock-reimport-buttons | shot | Import dock with the **Advanced… / Reimport** buttons. |
| 20-advanced-import-scene | shot | The **Advanced Import Settings** window (tree + 3-D preview + inspector). |
| 21-advanced-import-preview (poster) | shot | Poster for the orbit clip. |
| 22-advanced-per-mesh | shot | **Meshes tab** — per-mesh Save to File / Generate / LODs. |
| 23-advanced-node-generate | shot | **Scene tab** node — **Generate: Physics / NavMesh / Occluder**. |
| 24-advanced-physics-collision | shot | Physics On → **Body Type / Shape Type / Layer / Mask** (collision generation). |
| 25-advanced-materials-list | shot | **Materials tab** — the model's materials (Leaves, Wood). |
| 26-advanced-material-external | shot | Per-material **Use External** toggle. |
| 27-advanced-actions-menu | shot | **Actions…** — Extract Materials / Set Animation·Mesh Save Paths. |
| 28-material-preview-sphere | shot | A material previewed on a sphere in the Advanced window. |
| 40-scene-menu | shot | **Scene** menu: New Scene, New Inherited Scene…, Export As…. |
| 41-create-root-node | shot | New empty scene — **Create Root Node** (2D/3D/UI/Other). |
| 43-instanced-tree | shot | The GLB tree instanced into a clean lit scene (hero). |
| 50-gltf-export-settings | shot | **glTF Export Settings** (Copyright, Bake FPS, Image Format, Root Node Mode). |
| 51-obj-import-dock | shot | The **OBJ** import dock ("OBJ as Mesh": Scale/Offset Mesh). |
| 53-obj-house-and-tree | shot | The imported OBJ house beside the GLB tree. |
| 60-convert-to-builder-dialog | shot | **Convert to builder mesh** dialog (Merge/Keep materials/smoothing). |
| 70-standardmaterial3d-inspector | shot/tour | The **StandardMaterial3D** inspector (Albedo/Metallic/Roughness/…). |
| 72-albedo-color-changed | shot | Albedo Color set to orange (preview sphere + field). |
| 80-textures-next-to-model | shot | `fox.glb` with its **textures beside it** (fox_Diffuse.png, …). |
| 81-fox-textured | shot | The imported, textured fox character close up. |
| *(poster PNGs 30/42/44/52/54/55/61/82)* | poster | First-frame posters for the clips above. |

### Hotspot tour for `10/11/12-import-dock-*` (Import dock, left→right is top→bottom)
The Import dock is one vertical list; sections in order: **Import As** (top) → **Nodes** (Root
Type/Name/Script, Apply Root Scale, **Root Scale**, Import as Skeleton, Use Name Suffixes, Use Node
Types) → **Meshes** (Ensure Tangents, Generate LODs, Create Shadow Meshes, Light Baking, Force
Disable Compression) → **Skins** → **Animation** (Import, FPS, Trimming, Remove Immutable Tracks,
Import Rest as RESET) → **Import Script** → **Materials** (Extract, Extract Format, Extract Path) →
**glTF** (Naming Version, Embedded Image Handling, Texture Map Mode) → **Advanced… / Reimport**
buttons.

## Re-capture (review pass)

Session `guide-11` on display :105 / port 8905 (1920x1080 editor; same project copy as the
original pass). Re-shot in place:

- `81-fox-textured.png` — the fox is now framed clear of the tree trunk (select `fox`, F, orbit
  slightly, zoom in; crop 760x560 from the 3D viewport). The caption/alt no longer say "hidden by
  the trunk".
- `82-scene-orbit.mp4` (+ poster) — orbits the textured fox with the GLB round tree at the edge.
  The old caption mentioned an OBJ house, which is not in `imported_models.tscn`; caption corrected.
- `30-drag-into-viewport`, `42-drag-into-tree`, `52-obj-instance`, `55-instance-reimported-glb` —
  re-recorded at 1640x1000 (0,42) with a cleared Output panel. No harness/QA log lines are printed
  by current sessions; the remaining bottom-panel text is ordinary editor output.
- `05-models-folder-imported`, `52`/`55` posters — no `qa_tmp` folder in the session project.
  `res://models/` was rebuilt for the pass (exported_tree.glb exported from round_tree.glb through
  Scene → Export As → glTF 2.0 Scene) so the dock shows the same two files.
- `03-create-folder-menu.png`, `40-scene-menu.png`, `50-gltf-export-settings.png` — re-cropped with
  the full submenu / full menu / the dialog's Cancel+OK row (OK was cut off before). 03 and 40 are
  2x/1.7x NEAREST upscales; 50 is 1.6x.

## Recipes

`docs/guide/recipes/11-importing-models.json` — 18 items covering every media file on the page
(numbered series grouped; UI stills grouped in output lists). Tested on :115/8915:
`02-04-filesystem`, `30-drag-into-viewport`, `50-export-settings`, `80-82-fox` all pass.
Caveats recorded in the items: `05-models` writes the two sample files into the session project
and relies on the editor's rescan; `51`/`52`/`55`/`60` require those files (run 05 first);
the Advanced-window pixel coordinates and 54's Root Scale field are approximate.

### Test pass 2026-11-24 — display :120 / port 8920

Every item was run with `python3 qa/recapture.py docs/guide/recipes/11-importing-models.json
--only <id> --out-dir /tmp/... --display :120 --port 8920` (scratch dirs under
`/tmp/opencode/qa/recipe-tests/`, never `docs/guide/media/`). Green = steps run, outputs exist and
match the media's subject/framing/state.

| Item | Status | Notes |
|---|---|---|
| 01-hero | green | Pixel Preview must be OFF (it hides the editor grid/axes and the preview sun); F on `fox`, 3 wheel steps out, a level middle-drag up 140 px, a slight wheel-in and an empty-ground click reproduce the media framing (a little wider than the media). |
| 02-04-filesystem | green | (previously verified) |
| 05-models | green | The fixture shell now uses `${MST_QA_STATE:-/tmp/opencode/qa}` (the runner does not export MST_QA_STATE). Expand the `models` row with the arrow at (31,786); crop [8,770,268,82] ×2 gives the media's two imported files. |
| 09-13-import-dock | green | Media crops are the dock only: 09 = [0,40,285,35] ×3; 10-13 = [0,112,285,450] (file-name header excluded). 10-12 are ×2 (11/12 are 412 px tall), 13 is 1x. Shot order 10 → 13 (4 notches) → 11 (6) → 12 (14) because 13 needs a smaller scroll than 11. |
| 20-28-advanced | green | `window-resize 1560 864 --title "Advanced Import Settings for Scene 'round_tree.glb'"` then crop [192,80,1560,900] matches the media window exactly. Tabs at y=173 (Scene 247 / Meshes 304 / Materials 389); Scene-tree rows Scene 243 / Tree 266; the mesh row round_tree_Cylinder at 314; Materials rows Leaves 204 / Wood 223; Generate>Physics checkbox at 1622,299. Selecting Leaves turns the preview into the sphere. 22-24/26 crop [1443,143,315,770] ×2, 25 crop [200,144,310,770] ×2. 28 is a little tighter than the media sphere. |
| 30-drag-into-viewport | green | (previously verified) |
| 40-41-43-scene-menu | 40 green, 41 NOT green, 43 green | The glb "Open Anyway" prompt is a real window at (696,484); click 956,571 accepts it (click-text resolves that dialog's rect at the wrong offset). **41 cannot be green in this build:** Godot 4.7.2 here opens Scene > New Scene straight into an empty scene and shows the Create Root Node chooser as an inline Scene-dock menu (the "+" at 57,66), not the centred dialog captured in the media. The recipe captures the inline menu as evidence, and the output does not match the media dialog. 43 is the imported glb scene tab: select `round_tree/Tree`, F, 3 wheel steps out, level middle-drag, empty-ground click, Pixel Preview off. |
| 42-drag-into-tree | green (state) | The drop creates the `round_tree.glb` child row and the poster shows it; the session's default 3D camera is closer than the media, so the poster framing differs (subject/state correct). |
| 44-scale-gizmo | NOT green | The media shows the green Y scale handle dragged up (tree grows). Under synthetic input the drag grabs the wrong handle / the box centre: the status readout shows `Scaling: (0.000, 0.790, 0.000)` and the tree shrinks; the poster is also off-centre. The handle position after F+3 wheel steps is not stable enough for a fixed pixel drag. Needs a manual capture (or a gizmo-aware drag). |
| 50-export-settings | green | (previously verified) |
| 51-obj-import-dock | NOT green | The FileSystem path box and the OBJ row do not reproduce at the recipe's coordinates: the dock ends with "Select a resource file in the filesystem…" and the path box still shows `res://addons/`. The file rows/filter box sit at different y than the media pass (filter box at the bottom, y≈866). Needs the FileSystem rows re-measured per layout. |
| 52-obj-instance | NOT green | Same navigation failure; the poster shows the tree/scene, not the dragged OBJ. |
| 53-obj-house-and-tree | NOT green | Same navigation failure; no OBJ house in the scene. |
| 54-reimport-root-scale | NOT green | The Root Scale field coordinates are stale: the click at (150,200) opens the Scene dock's Create New Node search and the "2.0" text goes into it (evidence: scratch poster shows the Create New Node dialog with `No results for "2.0"`). Needs the Import dock's Root Scale field rect re-measured with `ui`. |
| 55-instance-reimported-glb | NOT green | Same FileSystem navigation/filter-coordinate failure as 51/52. |
| 60-61-convert | NOT green | Same FileSystem navigation failure; the OBJ is never dragged into the scene, so Convert has nothing to act on. |
| 70-72-material | NOT green | After selecting `round_tree/Tree/Leaves` the inspector shows the MeshInstance3D with **Surface Material Override collapsed**; the media needs it expanded with a StandardMaterial3D and the Albedo colour changed. `click-text "StandardMaterial3D"` does not open it, and the click at (1680,700) does not land on the Albedo field. Needs the override expanded/created first and the field rect measured. |
| 80-82-fox | green | (previously verified; Pixel Preview ON is correct for these media) |

Recipe-level changes made during this pass: 05's fixture env var; fixtures repeated at the start of
51/52/53/55/60 so each is replayable alone; 09-13 crops/scrolls; 20-28 window resize and crops; 40-43
Open Anyway click and glb-tab flow; 01/44 Pixel Preview toggles and camera. Everything else kept as
written.

### Test pass 2026-11-24 (DeepSeek, display :120 / port 8920)

Every item was run with `python3 qa/recapture.py docs/guide/recipes/11-importing-models.json
--only <id> --out-dir /tmp/opencode/qa/recipe-tests/<dir> --display :120 --port 8920`.
Green = steps run, outputs exist and match the media's subject/framing/state.

| Item | Status | Notes |
|---|---|---|
| 01, 02-04, 09-13, 20-28, 30, 40, 42, 43, 50, 80-82 | green | unchanged; re-checked where the item shares a session with a changed item. |
| 05-models | green | `prepare-models.py` writes the fixture, reloads the project (this headless session never rescans copied files), moves the re-created window back to 0,0 and cancels the restored-glb prompt. The path field navigates to `res://models/`; crop [8,770,268,82] x2. |
| 41-create-root-node | green (media re-shot) | This editor build shows **Create Root Node** inline in the Scene dock, so `docs/guide/media/11-importing-models/41-create-root-node.png` was re-shot from that inline menu (crop [0,82,290,214] x2, 580x428) and its caption/alt now describe the inline list. |
| 44-scale-gizmo | green | New `gizmo-y-drag.py` projects the node origin, screenshots the editor and finds the green +Y handle square (threshold g>=180, b<=20), then drags it up 175 px (scale Y 2.32). Framing is set by `09-frame.py --target 0 1 0 --dist 17 --elev 4`; record [370,132,760,660]. |
| 51-obj-import-dock | green | `drag-model.py --index 2 --click` switches the FileSystem dock to split view, navigates to `res://models/` and clicks the OBJ row computed from the live `FileSystemList` rect; expect on the Import dock's `roblox_house.obj` Label; crop [4,78,300,500] x1.9. |
| 52-obj-instance | green | `drag-model.py --index 2 --to 700,380` drags the OBJ from the split list; the poster is at 9.5 s so it shows the "Dropping a Mesh file…" tooltip and the instancing readout. |
| 53-obj-house-and-tree | green | split-list drag, Pixel Preview OFF (asserted), `09-frame.py --target 0 1 1 --dist 22 --elev 12`, empty click to clear the gizmo; crop [385,260,700,400]. |
| 55-instance-reimported-glb | green | split-list drag of `exported_tree.glb` (index 1); poster at 9.5 s. |
| 60-61-convert | green | split-list drag of the OBJ; the Pixel Builder **Convert** button is an untitled Button whose tooltip is "Convert", so it is clicked by path suffix (`/Convert`) via click-control.py. The confirm dialog is accepted with Enter (its default button). |
| 54-reimport-root-scale | green | The Import dock is activated, then the split list selects `round_tree.glb` from the courtyard folder (row 3 after `..`); `round_tree` is selected in the scene and framed with `09-frame.py --target 0 2 0 --dist 22 --elev 12`; record [0,42,1630,720] so the Import dock is in frame. `set-import-float.py` finds the Root Scale `EditorPropertyFloat` at run time; Reimport doubles the instance live. |
| 70-72-material | green | `make-material.py` expands the collapsed Surface Material Override section, opens the picker, creates StandardMaterial3D (Down, Down, Enter) and clicks the picker pencil, producing the media 70 state. The recipe frames the whole tree with `09-frame.py` **without clicking the viewport** (a left click would deselect Leaves and collapse the material). `expand-albedo.py` then finds the four collapsed material sections, opens Albedo, and clicks its Color property; `set-color-hex.py` types e07a2d. 71's poster shows the Albedo section with the tree; 72 matches the media's slot + orange preview + Albedo Color. |

New helpers (all under `docs/guide/recipes/_helpers/`): `prepare-models.py`, `click-window-button.py`,
`click-control.py`, `click-dialog.py`, `shot-window.py`, `set-import-float.py`, `set-color-hex.py`,
`gizmo-y-drag.py`, `window-home.py`, `drag-model.py`, `export-progress.py`, `shot-game.py`,
`make-material.py`, `expand-albedo.py`. `MST_QA_STATE` is read with the `/tmp/opencode/qa`
default. Session names used for tests: `v1113b-*` (exploration) and the runner's
`recap-11-importing-models`; all were stopped and their dirs removed.
