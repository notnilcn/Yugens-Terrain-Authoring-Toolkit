# Meet Godot

Godot is the program that opens the game project. You do not have to learn everything in it. This note only covers the parts you will touch every day.

![[editor-overview.png]]

## The five parts of the window

| Area | Where | What it is for |
|---|---|---|
| **Scene tree** | Top-left | Click a node to select it. The terrain, camera, sun and props are all here. |
| **FileSystem** | Bottom-left | Your project's files: scenes, textures, models, scripts. Double-click a scene to open it. |
| **3D view** | Center | The world. This is where you sculpt and place things. |
| **Inspector** | Right | Settings for the selected node. If you click the terrain, this is where the terrain settings live. |
| **Output** | Bottom | Messages. Red = error. You can ignore the usual startup messages; red errors that repeat every second are worth reporting. |

There are also tabs across the top of the 3D view: **2D**, **3D**, **Script**, **Game**, **Asset Store**. Stay in **3D** most of the time.

## Moving the camera

These work in the editor's 3D view without selecting a tool:

| Action | Control |
|---|---|
| Orbit around the point you are looking at | Hold **middle mouse button** and drag |
| Pan sideways / up | Hold **Shift + middle mouse button** and drag |
| Zoom in and out | **Mouse wheel** |
| Frame the selected node | `F` |
| Top / front / side view | Click the **Perspective** word at the top-left of the 3D view |

> [!tip]
> If the camera feels stuck, press `F` with the terrain selected. It will jump to the terrain.

## Selecting things

Click a node in the Scene tree. The Inspector on the right changes to show that node's settings. You can also click things directly in the 3D view, but it is easy to grab the wrong thing, so the Scene tree is safer.

When `MarchingSquaresTerrain` is selected, a vertical toolbar appears on the left edge of the 3D view. That toolbar is the terrain editor. It only appears for the terrain.

![[terrain-toolbar.png]]

## Adding a node

1. Click the **+** at the top-left of the Scene tree.
2. Type the name of the node you want (for example `Camera3D` or `PixelRenderer`).
3. Select it and click **Create**.

Most of the nodes you need already exist in the example scenes. You will only add nodes for new effects or a new scene.

## Moving, rotating and scaling

The four tools at the top of the 3D view:

| Key | Tool | What it does |
|---|---|---|
| `Q` | Select | Click things without moving them |
| `W` | Move | Drag the colored arrows |
| `E` | Rotate | Drag the colored rings |
| `R` | Scale | Drag the colored boxes |

> [!warning]
> While a terrain tool is active, the normal move/rotate/scale gizmos are hidden. That is intentional: the terrain tool owns the mouse. To move the terrain itself, press `Q` or click another tool first, or edit its **Transform** in the Inspector.

## Playing the game

| Action | Control |
|---|---|
| Play the current scene | `F5` (or the ▶ button at top-right) |
| Stop playing | `F8` |
| Play only the scene you are editing | `F6` |

In the demo, use `W A S D` to move, `Shift` to walk, `Space` to jump, drag with the mouse to orbit, and the wheel to zoom.

> [!tip]
> The game runs in a separate window. If you do not see it, check the taskbar; it sometimes opens behind the editor.

## Saving and undo

| Action | Control |
|---|---|
| Save the scene | `Ctrl+S` |
| Undo | `Ctrl+Z` |
| Redo | `Ctrl+Shift+Z` |

Every terrain stroke is one undo step. A whole foliage stroke is one step. You can experiment freely.

## The three addon buttons in the 3D toolbar

At the top of the 3D view, next to `Transform` and `View`, the addon adds:

| Button | What it does |
|---|---|
| **Pixel Preview** | Shows the pixel-art look inside the editor, following your editor camera. Turn it on while you work so you see what the player will see. |
| **Animate** | Makes clouds, water, grass and effects move in the editor. Leave it off if the editor feels slow. |
| **Pixel World** | A menu for adding sky, water, weather and other effects, plus ready-made lighting presets. |

![[pixel-preview-button.png]]

> [!tip]
> If the world suddenly looks flat and gray, check **Pixel Preview** first. If it is off, you are seeing the raw materials, not the final look.

## What not to touch

- `addons/MarchingSquaresTerrain/` — the addon itself.
- `project.godot` — the project settings, unless an adult or the programmer asks you to.
- The files in `demo/` — these are the showcase scenes. You can open and play them, but make your own copies before editing.

Next: [[02 Your First Terrain]]
