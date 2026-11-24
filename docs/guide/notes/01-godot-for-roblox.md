# Capture notes — page `01-godot-for-roblox` (Area C01 — Basics)

Source of truth for the writer. Verified in the **real Godot editor** (session `docs-basics`,
display `:71`, port `8871`, window **1600×900**, Godot **4.7.2.stable**, Mesa/llvmpipe).
Scene used for most shots: `res://tutorial/01_flat_playground.tscn` (root `Playground` →
`WorldEnvironment`, `Sun`, `MarchingSquaresTerrain`, `PixelWater`, `Camera`, `PixelRenderer`,
`PixelSky`, `PixelDayNight`). Cross-checked against `addons/MarchingSquaresTerrain/` where useful.

Audience: a Roblox-Studio builder who has **never opened Godot**. This page is pure orientation —
"where is everything, and how do I move around". The tools themselves are covered on later pages;
here we only *point* at them.

> Important correction for the writer: the MarchingSquaresTerrain addon is a **GDExtension**
> (native Rust), NOT a classic GDScript EditorPlugin. In **Project → Project Settings** the
> **Plugins** tab is EMPTY; the addon shows under the **GDExtension** tab instead (see
> `50-project-settings-plugins.png` and `50b-gdextension.png`). Write it as "GDExtension", not
> "plugin enabled in the Plugins list".

---

## 1. Feature inventory

### 1.1 The editor at a glance (the big regions)

Full-editor orientation with red highlight boxes: `00-orientation.png`. Clean version (hotspot
base): `01-editor-clean.png`. The window has these regions:

| Region | Where | Roblox analogue | Notes |
|---|---|---|---|
| **Main menu bar** | very top-left: `Scene  Project  Debug  Editor  Help` | File/Home menus | `Scene` holds New/Open/Save scene; `Project` holds Project Settings & export. |
| **Main-screen buttons** | top-centre: `2D  3D  Script  Game  Asset Store` | the big mode tabs | Switch the whole centre area between the 2D canvas, the **3D viewport**, the script editor, the running-Game view, and the online Asset Store. We live in **3D**. See `45-main-buttons.png`. |
| **Play buttons** | top-right cluster | Roblox Play / Stop | ▶ Run Project (F5), ⏸ Pause, ⏹ Stop (F8), then Run Current Scene (F6), Run Specific Scene, and a movie/record icon. See `46-play-buttons.png`. |
| **Scene dock** | left, "Scene"/"Import" tabs | **Explorer** | The scene tree: every node in the open scene. This is where you add/rename/reparent/hide/lock nodes. `40-scene-dock.png`. |
| **Inspector** | right, "Inspector"/"Signals" tabs | **Properties** | Shows every property of the **selected** node. Empty when nothing is selected (`41-inspector-empty.png`); fills with the node's properties when you click a node (`42-inspector-mst.png`, `42b-inspector-camera.png`). |
| **FileSystem dock** | lower-left, "FileSystem"/"History" tabs | **Toolbox / the place assets live** | The project's files, rooted at `res://` (= the project folder). Drag files into the scene, double-click scenes to open them. `43-filesystem.png`. |
| **3D viewport** | big centre area | the 3D editing camera | Where you see and build the world. `01-editor-clean.png` centre. |
| **Viewport toolbar (row 1)** | strip under the scene tabs | — | Transform-gizmo mode buttons (Select / Move / Rotate / Scale), selection helpers, snap, Transform & View menus. `47-toolbar-transform.png`. |
| **Viewport toolbar (row 2)** | strip below row 1 | — | **Pixel Preview**, **Animate**, **Pixel World**, **Roblox UI** (addon buttons; other pages cover them). `48-toolbar-pixelrow.png`. |
| **Perspective / View menus** | top-left of the 3D view | camera presets | `Perspective` menu = camera views (Top/Front/Right…, Perspective vs Orthogonal, display modes). `View` menu = viewport layout, gizmos, grid. |
| **View gizmo (nav cube)** | top-right of the 3D view | — | The little coloured X/Y/Z ball; click an axis to snap to that side view. |
| **Bottom panel** | bottom strip: Output / Debugger / Audio / Animation / Shader Editor | **Output window** | `Output` prints engine messages (the Inspect terrain tool prints here, errors show here). `44-output-panel.png`. |
| **Scene tabs** | row above the viewport | multiple open places | Each open scene is a tab; click to switch. A `(*)` after a name means unsaved changes. |

**Hotspot-tour positions** (centre as % of `01-editor-clean.png`, 1600×900) for the writer's
`.hotspots` component:
- Main-screen buttons (point at 3D): **42%, 2.4%** — "Switch the centre area: 2D, 3D, Script, Game, Asset Store."
- Play buttons: **93%, 2.4%** — "Run (F5), Stop (F8), Run Current Scene (F6)."
- Scene tabs: **32%, 6.6%** — "Each open scene is a tab; `(*)` = unsaved."
- Viewport toolbar row 1: **34%, 10.7%** — "Move / Rotate / Scale tools, snap, Transform & View menus."
- Viewport toolbar row 2: **31%, 15.2%** — "Addon buttons: Pixel Preview, Animate, Pixel World, Roblox UI."
- Perspective menu: **22%, 21.2%** — "Camera presets (Top/Front/…), Perspective vs Orthogonal."
- Scene dock: **8.9%, 28.6%** — "The scene tree ≈ Roblox Explorer."
- View gizmo: **79%, 23.8%** — "Click an axis to snap to a side view."
- 3D viewport: **50%, 43.3%** — "Your 3D editing camera."
- Inspector: **91%, 44.4%** — "Selected node's properties ≈ Roblox Properties."
- FileSystem: **8.9%, 76.2%** — "Your project's files, under `res://` ≈ Toolbox/assets."
- Output panel: **50%, 84.4%** — "Engine messages & errors ≈ Output window."

### 1.2 Moving the 3D camera (navigation)

Verified gestures (all performed with the mouse over the 3D viewport):

| Gesture | Action | Roblox analogue | Clip |
|---|---|---|---|
| **Hold RMB + W/A/S/D** (+ Q down / E up) | Fly / free-look like a first-person camera. Move the mouse to look while RMB held. | Right-drag + WASD fly in Studio | `13-fly.mp4` |
| **Middle-mouse drag** | Orbit (tumble) around the view pivot. | Right-drag orbit | `10-orbit.mp4` |
| **Shift + Middle-mouse drag** | Pan (slide the view sideways/up-down). | Middle-drag pan | `11-pan.mp4` |
| **Mouse wheel** | Zoom in / out (dolly). A "… m" distance ruler flashes on the left while zooming. | Scroll zoom | `12-zoom.mp4` |
| **`F`** | Focus the selected node — snaps the camera to frame it. | — (no exact equal) | `14-focus-f.mp4` |
| **Click the nav-cube axes** | Snap to Top/Front/Right/… views. | — | `15-viewgizmo.mp4` |
| **`Perspective` menu** | Pick a named view, toggle Perspective/Orthogonal, change display mode. | — | `17-perspective-menu.mp4` |
| **`View` menu** | Viewport count/layout, Gizmos, View Origin, View Grid. | — | `16-view-menu.mp4` |

`Perspective` menu full item list (verified, `17-perspective-menu.mp4`): Top View (Kp 7),
Bottom View (Alt+Kp 7), Left View (Alt+Kp 3), Right View (Kp 3), Front View (Kp 1), Rear View
(Alt+Kp 1), Switch Perspective/Orthogonal View (Kp 5), Perspective, Orthogonal, Auto Orthogonal
Enabled, Lock View Rotation, Display Normal / Wireframe / Overdraw / Lighting / Unshaded,
Display Advanced…, View Environment, View Gizmos, View Transform Gizmo, View Grid.

`View` menu items (verified, `16-view-menu.mp4`): 1/2/2-alt/3/3-alt/4 Viewports (Ctrl+1…Ctrl+4),
Gizmos ▸, View Origin, View Grid (`#`), Preview Translation ▸, Settings…

> Caveat: the clips were driven by injected input, so the mouse-look part of RMB-fly is small; the
> writer should describe it in words. Snapping to an axis via the nav-cube switches the view to
> *Orthogonal* ("Right Orthogonal [auto]"); press Kp 5 or pick **Perspective** in the Perspective
> menu to go back to a 3D perspective view.

### 1.3 Nodes & scenes (the Scene dock)

- **Node** = one object/thing in the scene (a light, a camera, a terrain, a mesh). Everything is a
  node. ≈ a Roblox Instance (Part, Model, etc.).
- **Scene** = a tree of nodes saved as a `.tscn` file. The top node is the **scene root** (here,
  `Playground`). ≈ a Roblox place / a Model you can reuse.
- **`res://`** = the project folder root (seen in the FileSystem dock).

| Control | Where | What it does | Media |
|---|---|---|---|
| **Add Child Node** (`+`) | Scene dock top-left; shortcut **Ctrl+A** | Opens the **Create New Node** dialog: a searchable list of every node type (incl. the addon's `MarchingSquaresTerrain`, `MarchingSquaresErosion`, `MarchingSquaresWildFlowers`, `MstSavePresetDialog`), a Description panel, Favourites & Recent, Cancel/Create. | `18-add-node.mp4`, `30-create-node-dialog.png` |
| **Instance Child Scene** (chain/link icon) | next to `+` | Drops an existing saved scene in as a child (an *instance*) ≈ inserting a Roblox Model/Package. | — (mentioned) |
| **Rename** | double-click a node, or select + **F2** | Edit the node's name inline. Names must be unique among siblings. | `19-rename-node.mp4` |
| **Reparent** | drag a node onto another node | Makes it a child of the drop target (indented under it). ≈ dragging in the Explorer. | `20-reparent.mp4` |
| **Visibility (eye)** | eye icon at the right of each 3D node's row | Hide/show that node in the viewport. | `21-eye-toggle.mp4` |
| **Lock** | toolbar padlock ("Lock selected node, preventing selection…"), or **Ctrl+L** / unlock **Ctrl+Shift+L** | Stops the node being picked in the viewport; a padlock badge appears on the row. | `22-lock-node.mp4` |
| **Scene tabs** | tab row above the viewport | Switch between open scenes; `(*)` = unsaved. | `23-scene-tabs.mp4` |
| **New Scene / Open Scene / Save** | `Scene` menu, or **Ctrl+S** to save, double-click a `.tscn` in FileSystem to open | Create/open/save scenes. | (page 02 shows New Scene) |

> Verified quirk (eye toggle): clicking the eye on the **MarchingSquaresTerrain** node does *not*
> visibly remove the terrain from the 3D view, because the pixel renderer draws the terrain surface
> through its own pipeline. For a clear hide/show demo we toggle the **Sun** instead — the whole
> island darkens and brightens (`21-eye-toggle.mp4`). Note this honestly for readers.

### 1.4 Godot's own transform gizmo (Move / Rotate / Scale)

Select any 3D node (Node3D family) and a coloured gizmo appears at it. Mode is chosen in the
viewport toolbar row 1 or by key:

| Key | Mode | Gizmo | Clip |
|---|---|---|---|
| **Q** | Select (no transform handles) | plain cursor | — |
| **W** | **Move** | 3 arrows (red X, green Y, blue Z) + plane squares | `24-gizmo-move.mp4` |
| **E** | **Rotate** | 3 coloured rings | `25-gizmo-rotate.mp4` |
| **R** | **Scale** | 3 axis handles with box ends | `26-gizmo-scale.mp4` |

- Drag an axis handle to transform along that axis; the exact numbers also live in the Inspector's
  **Transform** section (`52-transform-fields.png`: Position / Rotation / Scale x-y-z, Rotation Edit
  Mode = Euler, Rotation Order = YXZ).
- There is a **snap** toggle in the toolbar (magnet-style icon) — snapping transforms to fixed
  increments, like Roblox's stud/degree snapping.
- Bridge: this is Godot's *generic* gizmo. The addon **also** gives the terrain its own
  Roblox-style builder tools; and turning on **Roblox UI** adds a Roblox-Studio-style
  **Model/Edit ribbon** with Move/Scale/Rotate + snap fields — see §1.6 and page 09.

### 1.5 Undo / redo & the History dock

- **Ctrl+Z** = Undo, **Ctrl+Shift+Z** = Redo (also `Edit`-style entries in the main menu). Clip
  `27-undo-redo.mp4`.
- The **History** tab (shares the lower-left dock with FileSystem) lists every action with
  **Scene** / **Global** filter checkboxes; the newest is at the top, "The Beginning" at the
  bottom. `51-history-dock.png` shows a real history (Translate, Lock/Unlock Selected, Toggle
  Visible…, Reparent Node, Rename Node, Create Node, The Beginning).

### 1.6 Running the project

- **F5** = Run Project (the main scene). **F6** = **Run Current Scene** (what we use). **F8** =
  Stop. The ▶/⏸/⏹ buttons are the same, top-right.
- Running opens a **separate game window** (titled "Marching Squares Terrain (DEBUG)"), shows the
  Godot boot splash briefly, then the game. Running `01_flat_playground` shows a pixel-art island
  (grass plateau + a soft central mound, teal pixel-water with foam edges) — `28-run-game.png`,
  clip `28-run-game.mp4`.
- While running, the Scene dock gains a **Remote / Local** toggle (inspect the live tree); Stop
  returns to editing. No errors were produced by running/stopping (`--- Debugging process
  stopped ---`).

### 1.7 The "Roblox UI" toggle (the reader's comfort zone)

The **Roblox UI** button (viewport toolbar row 2) switches the addon's builder into a
**Roblox-Studio-style ribbon**. With a `MarchingSquaresTerrain` (or builder) node selected and
Roblox UI **on**, a ribbon appears above the viewport with **Model** / **Edit** tabs and a row of
tools: Select, **Move**, **Scale**, **Rotate**, a pivot tool, a **Geometric** group with
**move-snap "1.0 m"** and **rotate-snap "45°"** fields + checkboxes, then snap/duplicate/mirror/
anchor icons — exactly like Roblox's Model tab. Media: `55-roblox-ui-ribbon.png`, before/after
`56-robloxui-off.png` / `57-robloxui-on.png`, clip `29-robloxui-toggle.mp4`.

Page 01 only points at the button; the ribbon's tools are covered on the builder pages (09, 10).
This page should reassure the reader: "the Roblox-style ribbon exists; flip it on if the raw Godot
toolbar feels unfamiliar."

### 1.8 Where the addon lives

- **FileSystem → `addons/`** contains `MarchingSquaresTerrain/` (and `phantom_camera_rust/`).
  `49-addon-filesystem.png`.
- **Project → Project Settings → GDExtension** lists it as
  `res://addons/MarchingSquaresTerrain/marching_squares_terrain.gdextension` (Min Version 4.7),
  alongside `phantom_camera_rust` and `res://demo/mst_demo.gdextension`. `50b-gdextension.png`.
- The **Plugins** tab is empty (it's not that kind of plugin). `50-project-settings-plugins.png`.

---

## 2. Roblox → Godot term map (verified in the editor)

| Roblox Studio | Godot / this addon | Verified? |
|---|---|---|
| Explorer | **Scene dock** (scene tree) | yes — `40-scene-dock.png` |
| Properties | **Inspector** | yes — `42-inspector-mst.png` |
| Workspace / the place | the **scene** and its **root node** (`.tscn`) | yes |
| Instance (Part, Model…) | **node** | yes |
| Part | a `MeshInstance3D` / the addon's `PixelBuilderMesh` (builder pages) | partially (builder on page 09) |
| Model / grouping | a `Node3D` holding children, or a saved **scene** you instance | yes — reparent `20-reparent.mp4` |
| Inserting a Model/Package | **Instance Child Scene** (chain icon) | surface seen; chain icon present |
| Toolbox / where assets live | **FileSystem** dock (`res://`) + **Asset Store** | yes — `43-filesystem.png` |
| Terrain Editor | select a **MarchingSquaresTerrain** node → the terrain toolbar (page 03) | yes (toolbar appears on select) |
| Move / Scale / Rotate tools | Godot gizmo **W/E/R**, or the **Roblox UI** ribbon's Move/Scale/Rotate | yes — §1.4, §1.7 |
| Stud/degree snapping | gizmo **snap** toggle; ribbon "1.0 m" / "45°" fields | yes — `55-roblox-ui-ribbon.png` |
| Output window | **Output** bottom panel | yes — `44-output-panel.png` |
| Play / Stop | **F5/F6** run, **F8** stop; ▶⏸⏹ top-right | yes — `28-run-game.*` |

---

## 3. Walkthroughs (actually performed)

**W1 — Take the tour (no changes).** Open `01_flat_playground`. Point out each region using the
hotspot list in §1.1. Orbit (middle-drag), pan (Shift+middle), zoom (wheel), fly (RMB+WASD),
press `F` on a node to focus. Snap to a side with the nav-cube, then back to Perspective (Kp 5).
Media: `10`–`17`.

**W2 — Nodes.** `+` (Ctrl+A) → search "Camera3D" → Create (adds `Camera3D`). Select it, F2,
rename to `PlayerCamera`. Drag it onto `MarchingSquaresTerrain` to reparent. Toggle the Sun's eye
to see hide/show. Open the **History** tab to see the trail. Media: `18`–`23`, `51`.

**W3 — Move things.** Select `PlayerCamera`. Press **W** and drag the red arrow (move), **E** and
drag a ring (rotate), **R** and drag a box handle (scale); each is one Undo step — **Ctrl+Z**.
Media: `24`–`27`, `52`.

**W4 — Run it.** Mouse over the 3D view, press **F6**. Wait for the boot splash, see the pixel
island in the game window. Press **F8** to stop. Media: `28`.

**W5 — Feel at home.** Select the terrain, click **Roblox UI**: the Roblox-style Model/Edit ribbon
appears. Media: `29`, `55`–`57`.

---

## 4. Media manifest

Clips (`.mp4`, each has a matching `.png` poster):

| File | Kind | Shows |
|---|---|---|
| `10-orbit.mp4` | clip | Middle-drag orbits the 3D camera. |
| `11-pan.mp4` | clip | Shift+middle-drag pans the view. |
| `12-zoom.mp4` | clip | Wheel zooms in/out (distance ruler flashes). |
| `13-fly.mp4` | clip | RMB + WASD fly/free-look. |
| `14-focus-f.mp4` | clip | `F` snaps the camera to frame the selected terrain. |
| `15-viewgizmo.mp4` | clip | Clicking nav-cube axes snaps to side views. |
| `16-view-menu.mp4` | clip | The **View** menu open (viewports, gizmos, grid). |
| `17-perspective-menu.mp4` | clip | The **Perspective** menu open (view presets, ortho). |
| `18-add-node.mp4` | clip | `+` → Create New Node → search Camera3D → Create. |
| `19-rename-node.mp4` | clip | F2 rename Camera3D → PlayerCamera. |
| `20-reparent.mp4` | clip | Drag PlayerCamera under MarchingSquaresTerrain. |
| `21-eye-toggle.mp4` | clip | Toggling the Sun's eye darkens/brightens the island. |
| `22-lock-node.mp4` | clip | Lock button → padlock badge, then unlock. |
| `23-scene-tabs.mp4` | clip | Switch to `hex_island` scene and back. |
| `24-gizmo-move.mp4` | clip | **W** move gizmo, drag red X axis. |
| `25-gizmo-rotate.mp4` | clip | **E** rotate gizmo, drag a ring. |
| `26-gizmo-scale.mp4` | clip | **R** scale gizmo, drag a box handle. |
| `27-undo-redo.mp4` | clip | Move, Ctrl+Z (undo), Ctrl+Shift+Z (redo). |
| `28-run-game.mp4` | clip | The running game window (pixel island). |
| `29-robloxui-toggle.mp4` | clip | Roblox UI ribbon appears/disappears. |

Stills:

| File | Kind | Shows |
|---|---|---|
| `00-orientation.png` | shot (boxed) | Full editor with red boxes on all major regions. |
| `01-editor-clean.png` | shot | Clean full editor — **hotspot-tour base image**. |
| `30-create-node-dialog.png` | shot | Create New Node dialog (search, matches incl. addon nodes, description). |
| `40-scene-dock.png` | shot | Scene dock (tree ≈ Explorer). |
| `41-inspector-empty.png` | shot | Inspector with nothing selected. |
| `42-inspector-mst.png` | shot | Inspector showing MarchingSquaresTerrain properties. |
| `42b-inspector-camera.png` | shot | Inspector showing a Camera3D (with Camera Preview). |
| `43-filesystem.png` | shot | FileSystem dock at `res://`. |
| `44-output-panel.png` | shot | Output / bottom panel. |
| `45-main-buttons.png` | shot (2×) | 2D / 3D / Script / Game / Asset Store. |
| `46-play-buttons.png` | shot (2×) | Run / Pause / Stop / Run-scene buttons. |
| `47-toolbar-transform.png` | shot (2×) | Viewport toolbar row 1 (gizmo modes, Transform/View). |
| `48-toolbar-pixelrow.png` | shot (2×) | Viewport toolbar row 2 (Pixel Preview/Animate/Pixel World/Roblox UI). |
| `49-addon-filesystem.png` | shot | `addons/MarchingSquaresTerrain` in the FileSystem. |
| `50-project-settings-plugins.png` | shot | Project Settings → Plugins (empty). |
| `50b-gdextension.png` | shot | Project Settings → GDExtension (addon listed). |
| `51-history-dock.png` | shot | History dock with a real action trail. |
| `52-transform-fields.png` | shot | Inspector Transform (Position/Rotation/Scale). |
| `54-hex-island-scene.png` | shot | The rich `hex_island` scene (example of a scene). |
| `55-roblox-ui-ribbon.png` | shot | The Roblox-style Model/Edit ribbon. |
| `56-robloxui-off.png` / `57-robloxui-on.png` | before/after | Toolbar area without / with the Roblox UI ribbon. |

Before/after compare pair: `56-robloxui-off.png` ↔ `57-robloxui-on.png` (same crop). Hotspot tour:
`01-editor-clean.png` + the % positions in §1.1.

---

## 5. Roblox bridges (callout seeds)

- "The **Scene dock** is your **Explorer**; the **Inspector** is **Properties**."
- "`res://` in the **FileSystem** is your project folder — think **Toolbox**, but it's your own files."
- "A **scene** (`.tscn`) is like a Roblox place or a reusable Model; the top node is the Workspace."
- "**W/E/R** are Move/Rotate/Scale — or flip on **Roblox UI** for a Model-tab-style ribbon with
  snap boxes (1.0 m, 45°)."
- "Hiding a node's **eye** ≈ toggling an Instance's visibility; **Lock** ≈ the Lock in Studio."
- "**F6** is Play; **F8** is Stop."

## 6. Caveats

- `01_flat_playground`'s terrain is small and flat; the beautiful full-screen "pixel island" in the
  orientation shots is the PixelRenderer composite seen from the scene's saved camera. Reopening a
  scene does **not** restore that saved camera (the editor keeps the last camera you left), so
  framing has to be redone by hand — not a reader issue, just a capture note.
- Snapping a view via the nav-cube switches to **Orthogonal**; press **Kp 5** / pick Perspective to
  return to 3D.
- Eye-toggle on the terrain node doesn't visibly hide the pixel-rendered surface (renderer-driven);
  demo uses the Sun instead. State this honestly.
- The addon is a **GDExtension**, so the **Plugins** tab is empty — document GDExtension instead.
- The only `ERROR:` in the session log came from a QA driver mistake (calling `get_parent` with an
  argument), **not** from the addon. No addon errors, no crashes.

## 7. Scenes created

None. This page only navigates the existing `01_flat_playground` (and peeks at `hex_island`). The
session copy accumulated throwaway edits (added/renamed/reparented a Camera3D, gizmo nudges) but was
**not saved**; the repo scenes are untouched.

---

## Re-capture (review pass) — 2026-11-24

Re-shot in a fresh session (`recap-a`, display `:81`, port `8881`, window 1600×900,
scene `res://tutorial/01_flat_playground.tscn`). The current harness no longer prints the
`MST QA harness listening …` line, so the Output panel is clean; the empty `qa_tmp/` folder was
removed from the session project so it no longer shows in the FileSystem dock.

- **16-view-menu.png** — FIXED. Now shows the **View** menu fully open and *not* cut off at the
  right edge (the `Ctrl+1 – Physical`, `Ctrl+2 – Physical`, … shortcut column is complete), with
  the **View** toolbar button highlighted. Crop includes the viewport-toolbar rows for context.
- **17-perspective-menu.png** — FIXED. Previously duplicated the View menu; now shows the full
  **Perspective** (view-mode) menu: Top/Bottom/Left/Right/Front/Rear View, Switch Perspective/
  Orthogonal, Perspective•Orthogonal•Auto Orthogonal, Lock View Rotation, Display Normal/Wireframe/
  Overdraw/Lighting/Unshaded, Display Advanced…, View Environment/Gizmos/Transform Gizmo/Grid/
  Information/Frame Time, Half Resolution, Audio Listener, Enable Doppler, Cinematic Preview,
  Focus Origin, Focus Selection, Align Transform/Rotation with View. All 39 items visible, no
  cutoff. (Both matching .mp4 clips were already correct; only the posters were wrong.)
- **00-orientation.png** — FIXED. Re-shot the full editor with the same eight red region boxes
  (main-screen buttons, play buttons, Scene dock, FileSystem dock, Inspector, 3D viewport, Output
  panel, viewport-toolbar rows). Output panel empty, no `qa_tmp`, nothing selected. The 3D view is
  reframed to a clean 3/4 top-down of the flat-playground island (hotspot %-positions in §1.1 still
  apply — panel layout is unchanged).
- **01-editor-clean.png** — FIXED. Same clean full editor (the hotspot-tour base), empty Output,
  empty Inspector (nothing selected), no `qa_tmp`, island framed centre.
- **44-output-panel.png** — FIXED. No more harness line. The new session's raw startup output
  contains only benign `Failed to start … Already in use` port-contention noise (other parallel
  editor sessions hold 6005/6006), which reads like errors, so instead the panel shows three
  genuine **Inspect-tool** messages (`Marching Squares Terrain: … height …, floor 1 'Grass', wall 1
  'Grass', grass 255, smooth false`) — exactly the kind of engine/tool message the caption
  describes ("the Inspect terrain tool prints here"). Cropped to the log + filter row + tab bar.
- **18-add-node.png** — FIXED. Poster now shows the **Create New Node** dialog searching
  `Camera3D` (Camera3D highlighted, description + Create button), consistent with the clip. Output
  behind is clean (no harness line); `qa_tmp` gone.

Could not fix / left as-is: nothing outstanding on this page. Other 01 stills that show the
FileSystem dock (`43-filesystem.png`, `49-addon-filesystem.png`) still contain an empty `qa_tmp/`
folder but were **not** flagged by the review and do not show the harness line; they were left
untouched to stay in scope.


---

## Recipes

Replayable recipes live in `docs/guide/recipes/01-godot-for-roblox.json` (44 items). Every item was run to green with
`python3 qa/recapture.py docs/guide/recipes/01-godot-for-roblox.json --out-dir <scratch> --display :116 --port 8916`;
outputs were compared against `docs/guide/media/01-godot-for-roblox/`. The recipe setup writes a helper
(`/tmp/opencode/ds_mst_ui.py`) for label-addressed clicks, projected world-point clicks/crops and movie recording.

| Item | Kind | Outputs | Status |
|---|---|---|---|
| `01-editor-clean` | still | `01-editor-clean.png` | green |
| `00-orientation` | still | `00-orientation.png` | green |
| `16-view-menu` | still | `16-view-menu.png` | green |
| `17-perspective-menu` | still | `17-perspective-menu.png` | green |
| `18-add-node` | still | `18-add-node.png` | green |
| `44-output-panel` | still | `44-output-panel.png` | green |
| `10-orbit` | clip | `10-orbit.mp4`, `10-orbit.png` | green |
| `11-pan` | clip | `11-pan.mp4`, `11-pan.png` | green |
| `12-zoom` | clip | `12-zoom.mp4`, `12-zoom.png` | green |
| `13-fly` | clip | `13-fly.mp4`, `13-fly.png` | green |
| `14-focus-f` | clip | `14-focus-f.mp4`, `14-focus-f.png` | green |
| `15-viewgizmo` | clip | `15-viewgizmo.mp4`, `15-viewgizmo.png` | green |
| `16-view-menu-clip` | clip | `16-view-menu.mp4` | green |
| `17-perspective-menu-clip` | clip | `17-perspective-menu.mp4` | green |
| `18-add-node-clip` | clip | `18-add-node.mp4` | green |
| `19-rename-node` | clip | `19-rename-node.mp4`, `19-rename-node.png` | green |
| `20-reparent` | clip | `20-reparent.mp4`, `20-reparent.png` | green |
| `21-eye-toggle` | clip | `21-eye-toggle.mp4`, `21-eye-toggle.png` | green |
| `22-lock-node` | clip | `22-lock-node.mp4`, `22-lock-node.png` | green |
| `23-scene-tabs` | clip | `23-scene-tabs.mp4`, `23-scene-tabs.png` | green |
| `24-gizmo-move` | clip | `24-gizmo-move.mp4`, `24-gizmo-move.png` | green |
| `25-gizmo-rotate` | clip | `25-gizmo-rotate.mp4`, `25-gizmo-rotate.png` | green |
| `26-gizmo-scale` | clip | `26-gizmo-scale.mp4`, `26-gizmo-scale.png` | green |
| `27-undo-redo` | clip | `27-undo-redo.mp4`, `27-undo-redo.png` | green |
| `28-run-game` | clip | `28-run-game.mp4`, `28-run-game.png` | green |
| `29-robloxui-toggle` | clip | `29-robloxui-toggle.mp4`, `29-robloxui-toggle.png` | green |
| `30-create-node-dialog` | still | `30-create-node-dialog.png` | green |
| `40-scene-dock` | still | `40-scene-dock.png` | green |
| `41-inspector-empty` | still | `41-inspector-empty.png` | green |
| `42-inspector-mst` | still | `42-inspector-mst.png` | green |
| `42b-inspector-camera` | still | `42b-inspector-camera.png` | green |
| `43-filesystem` | still | `43-filesystem.png` | green |
| `45-main-buttons` | still | `45-main-buttons.png` | green |
| `46-play-buttons` | still | `46-play-buttons.png` | green |
| `47-toolbar-transform` | still | `47-toolbar-transform.png` | green |
| `48-toolbar-pixelrow` | still | `48-toolbar-pixelrow.png` | green |
| `49-addon-filesystem` | still | `49-addon-filesystem.png` | green |
| `50-project-settings-plugins` | still | `50-project-settings-plugins.png` | green |
| `50b-gdextension` | still | `50b-gdextension.png` | green |
| `51-history-dock` | still | `51-history-dock.png` | green |
| `52-transform-fields` | still | `52-transform-fields.png` | green |
| `54-hex-island-scene` | still | `54-hex-island-scene.png` | green |
| `55-roblox-ui-ribbon` | still | `55-roblox-ui-ribbon.png` | green |
| `56-57-robloxui-pair` | series | `56-robloxui-off.png`, `57-robloxui-on.png` | green |

**Verification notes.** The six original items (`00-orientation`, `01-editor-clean`, `16-view-menu`, `17-perspective-menu`,
`18-add-node`, `44-output-panel`) reproduce the reviewed stills. The camera clips (`10`–`15`) use the same real gestures
(middle-drag, Shift+middle, wheel, RMB look, `F`, nav-cube clicks) and the same 3D framing as the media; injected input
makes the motion amplitude slightly smaller than the hand-driven originals. `19`, `20`, `22` and `24`–`27` create their
Camera3D through the real Create New Node dialog (+ → `Camera3D` → Enter); the reparent item uses `Node.reparent` because
the Scene-tree drag row target is not exposed to the harness. `50`/`50b` open Project Settings from the Project menu and
click the tab by pixel (the tabs are a TabBar without per-tab Controls). `49-addon-filesystem` shows the FileSystem dock
with the `addons` folder but does not expand the tree to the addon files; `43-filesystem`/`49` therefore still show the
empty `qa_tmp` folder present in the original media (the setup removes it from fresh session copies, so new runs show a
clean tree — the difference is confined to those two dock shots).
