# Making Props in the Builder

Godot has a built-in 3D modeler called the **Pixel Builder**. It is not as powerful as Blender, but it is more than enough for rocks, crates, fences, simple houses and stylised props. You never have to leave the editor.

![[builder-tour.mp4]]

## Opening the builder

1. Add a **PixelBuilderMesh** node to the scene (the **+** button in the Scene tree).
2. Select it.

The builder UI appears:

- A line at the top-left of the 3D view says `Pixel Builder - <mode> mode, <tool>`.
- A **Builder** menu appears in the 3D view's menu bar.
- A strip appears at the bottom with **Shape**, **Smooth**, **Color**, and the shape's size fields.
- A **Materials** panel appears on the right.

![[builder-ui.png]]

> [!tip]
> The **Builder** menu has **Modeler layout**. It swaps the interface for a larger tool palette with an **Add Mesh** grid. Try both and use the one that feels better. The keyboard shortcuts work in both.

## The four modes

| Key | Mode | What you select |
|---|---|---|
| `1` | Object | The whole prop |
| `2` | Face | One flat face |
| `3` | Edge | One edge |
| `4` | Vertex | One corner point |

`H` cycles through the modes if you forget a number.

In **Object** mode, Godot's normal move/rotate/scale gizmos work (`W`, `E`, `R`). In the other modes, the builder takes over the gizmo; use `Shift+W`, `Shift+E`, `Shift+R` for its move/rotate/scale handles.

## Make a simple prop

This is the whole beginner workflow. We will make a coloured stone block.

1. Add a **PixelObject** (optional, but it gives the prop the pixel outline look).
2. Add a **PixelBuilderMesh** as a child of the PixelObject.
3. Select the PixelBuilderMesh.
4. In the bottom strip, set **Shape** to `Box`.
5. Set **W**, **H**, **D** to the size you want (for example `1.2`, `1.0`, `1.2`).
6. Press `C` (Draw shape).
7. Drag on the ground to set the footprint. Move the mouse up to set the height. Click to finish.
8. Press `W` to return to the Select/Move tool. In Object mode, use the normal gizmo to place it.
9. In the strip, click **Color** and pick a stone grey.
10. Press `B` (Paint). Click the faces to paint them. `Shift` + click fills the whole flat area.
11. `Ctrl+S` saves the scene. The mesh data is stored inside the scene.

> [!warning]
> Pressing `C` and drawing creates a **new sibling mesh** next to the one you selected. Hold `Shift` while you click to finish (or stamp) the shape if you want to add it to the currently selected mesh instead. (`Shift+C` opens the Bezier tool, not append.)

> [!warning]
> Do not change **W/H/D** after you have moved individual vertices or faces. Changing the recipe rebuilds the shape and replaces those edits. Use the Object-mode scale gizmo for non-destructive resizing.

## Painting and materials

There are two levels of “colour”:

### Face colour (easy)

The strip's **Color** picker sets a flat colour. The paint tool (`B`) assigns the current colour to faces. This is enough for most stylised props, and it needs no texture files.

- `Shift` + click fills a whole flat area.
- `Ctrl` + click picks the colour/slot under the cursor.
- The eight swatches store colours: click to use, `Ctrl` + click to store the current colour, `Alt` + click to select all faces with that colour.

### Material slots (when you need textures)

The right-hand **Materials** panel can hold material slots. Each slot is a normal Godot material with a texture. Assign a slot to faces with the paint tool.

- **+ Slot** adds an empty material slot.
- **+ Preset** adds Grid, Checker, No Draw, Collider or Trigger.
- Drag a material from the FileSystem onto a face to assign it.
- **No Draw** makes faces invisible but still solid for collision. Useful for the back of a wall.
- **Collider** / **Trigger** mark faces for gameplay.

> [!tip]
> For a first prop, stay with face colours. Textures can come later, when the shape is right.

## Smooth or faceted

The right panel has **Smooth** and **Faceted** buttons. Smooth blends the normals across faces, so a sphere looks round. Faceted keeps each face flat, so a crystal looks sharp.

For pixel art, faceted is often the better choice: it preserves the low-poly, hand-made look.

## Collision

If the player should not walk through the prop:

1. Select the PixelBuilderMesh.
2. Open the **Collision** fold in the Inspector and click **Set Collider**. Or right-click the mesh in the 3D view and choose **Object → Set Collider**.
3. For a trigger (an area that detects the player but does not block), choose **Set Trigger**.

The collider is a simple trimesh built from the prop. For a character or a moving object, you usually want a simpler collision shape instead.

## Saving and reusing

| What you want | How |
|---|---|
| Keep it in this scene | `Ctrl+S`. The data is stored in the scene. |
| Reuse it in other scenes | Select the mesh, open **Contents** in the Inspector, use the resource menu → **Save As…**, and save a `PixelBuilderMeshData` `.tres`. Then set **Contents** to that file on every copy. |
| Reuse the whole node setup | **Builder → Create reusable part…** saves the branch as a `.tscn` and replaces it with an instance. |

> [!warning]
> If two meshes point at the same `.tres`, editing one edits both. Use **Make Unique** on the resource before editing a shared prop, or save a separate copy.

## Editing an existing model

You can convert an imported mesh into editable geometry:

1. Select the `MeshInstance3D` (or the root of a CSG shape).
2. In the Inspector, find the **Editable Mesh** section and click **Make Editable**.
3. The builder takes over, keeping the materials and UVs.

This is destructive: it replaces the original mesh with an editable copy. It also drops rigs and animations, so do not convert a character you still want to animate.

## Useful builder shortcuts

| Key | Action |
|---|---|
| `1` `2` `3` `4` | Object / Face / Edge / Vertex mode |
| `W` | Select / Move tool |
| `C` | Draw shape |
| `Shift+C` | Bezier tool (append = hold Shift while finishing a shape) |
| `B` | Paint material/colour |
| `5` | Vertex paint |
| `E` | Extrude |
| `I` | Inset |
| `Ctrl+B` | Bevel |
| `Ctrl+R` | Insert edge loop |
| `M` | Merge |
| `J` | Connect |
| `F` | Fill hole (Edge/Vertex mode with a selection; otherwise frames) |
| `Ctrl+T` | Triangulate |
| `Ctrl+D` | Duplicate faces |
| `Delete` | Delete selection |
| `Ctrl+A` | Select all |
| `G` / `Shift+G` | Grow / shrink selection |
| `L` / `Alt+L` | Select edge loop / ring |
| `U` | Open the UV editor |
| `[` / `]` | Halve / double the grid step |
| `Esc` | Cancel, leave the tool, clear selection, or go back to Object mode |

## UV editing (when you use textures)

Press `U` to open the **UV editor**. It has **Faces** and **Verts** modes and the usual tools for moving, rotating, scaling, fitting and projecting UVs. This is only needed when a prop uses a texture and the automatic box mapping is not good enough.

For flat-coloured props, ignore UVs entirely.

## Exporting

The **Builder** menu can export:

| Format | Use |
|---|---|
| **OBJ** | Blender or another 3D program (includes an `.mtl` material file) |
| **glTF / GLB** | Game engines and online viewers |
| **STL** | 3D printing |
| **PLY** | Point/scan tools and some DCC apps |
| **Godot mesh (.res)** | A baked, non-editable mesh for the game |

**Bake to MeshInstance3D** turns the editable mesh into a plain mesh. Do this near the end of a project if you do not need to edit the prop anymore; it is faster to load.

Next: [[11 Example Scenes]]
