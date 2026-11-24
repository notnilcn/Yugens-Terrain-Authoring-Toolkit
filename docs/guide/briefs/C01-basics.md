# Area C01 — pages `01-godot-for-roblox` and `02-first-world`
Session: name `docs-basics`, display `:71`, port `8871`, scene `res://tutorial/01_flat_playground.tscn`.
Media: `docs/guide/media/01-godot-for-roblox/` and `docs/guide/media/02-first-world/`; notes: `docs/guide/notes/01-godot-for-roblox.md` and `docs/guide/notes/02-first-world.md`.

## 01 — Godot for Roblox builders (orientation; the reader has never seen Godot)
- Full-editor orientation shot with highlight boxes + a hotspot tour: main-screen buttons (2D, 3D, Script, Game, Asset Store), Scene dock (≈ Explorer), Inspector (≈ Properties), FileSystem dock (≈ the place assets live; `res://`), bottom panel (Output ≈ Output window), scene tabs, the 3D viewport toolbar rows (incl. Pixel Preview / Animate / Pixel World / Roblox UI buttons — just point at them, other pages cover them), the play buttons top-right.
- 3D navigation, each as a clip: hold right mouse + WASD/QE fly (≈ Roblox camera), middle-drag orbit, Shift+middle pan, wheel zoom, `F` focus selection, the view gizmo (click axes), View menu / Perspective menu basics.
- Nodes & scenes: what a node is, the scene tree, adding a node (`+` button / Ctrl+A "Create New Node" dialog with search), renaming, reparenting by drag, eye toggle (visibility), lock; what a scene (`.tscn`) is; saving (Ctrl+S), opening scenes from FileSystem, scene tabs; instancing a scene (chain icon) ≈ Roblox packages/models.
- Godot's own transform gizmo (select a Node3D; Q/W/E/R select/move/rotate/scale, snapping toggle, Inspector Transform fields) — and that the builder has its own Roblox-style tools (link to page 09).
- Undo/redo, History dock.
- Running: Play project (F5), Play current scene (F6), Stop (F8); the Game tab / game window. Capture the running game window.
- Where the addon lives (FileSystem `addons/MarchingSquaresTerrain`), Project > Project Settings > Plugins showing it enabled.
- Roblox→Godot term map (Part→MeshInstance3D/PixelBuilderMesh, Model→scene/Node3D, Workspace→scene root, Explorer→Scene dock, Properties→Inspector, Toolbox→FileSystem/Asset Store, Terrain Editor→MarchingSquaresTerrain tools…). Verify each claim in the editor.

## 02 — Your first world (end-to-end quick start, ~15 min for a reader)
Build a brand-new scene from scratch and capture every step: Scene > New Scene, a 3D Scene root (Node3D), add a `MarchingSquaresTerrain` node, select it so the terrain toolbar appears, sculpt a hill (Brush), flatten an area (Level), paint grass/rock (Paint), add some foliage (Foliage), then use **Pixel World** to add the pixel look (renderer, sky, day/night, water — whatever the panel offers), toggle Pixel Preview, add a Camera3D framed on the island (or whatever the addon expects for the pixel renderer — investigate), save as `res://tutorial/guide/first_world.tscn`, and press F6 to run it; capture the running game. Keep each step short — deeper pages cover each tool. Copy the scene + data into the repo `tutorial/guide/` per CAPTURE.md.
