# Troubleshooting

Most “broken addon” moments are one of five things: the wrong node is selected, a feature switch is off, the camera is in the wrong place, the editor is not showing the pixel preview, or the thing genuinely needs a resource it does not have.

Use this page in order.

## The terrain toolbar is missing

**Cause:** Something other than the terrain is selected.

**Fix:** Click `MarchingSquaresTerrain` in the Scene tree. The toolbar appears on the left edge of the 3D view.

If the terrain is selected and the toolbar is still missing, the addon library may not have loaded. Check the Output panel for red errors when opening the project. A missing `libmarching_squares_terrain.so` / `.dll` means the addon was not built for this computer.

## The ground is invisible or full of holes

| Cause | Fix |
|---|---|
| The terrain has no chunks | Select it, choose **Chunks** (`8`), click a green **+** |
| Part of the ground is painted with slot 16 (Void) | Paint it with a real texture, or `Ctrl+Z` |
| The camera is under the terrain or facing away | Select the camera and press `F`, or frame the terrain |
| The storage mode is broken | **Settings → Storage**: switch to `Resource` for a simple scene |
| The terrain is below the water plane and the water is opaque | Raise the camera or lower the water |

## The world is flat gray / not pixel art

1. Is there a `PixelRenderer` in the scene?
2. Is **Pixel Preview** on at the top of the 3D view? (Or is `Studio Prelook` on in the renderer?)
3. Is the project using the **Compatibility** renderer? `Project Settings → Rendering → Renderer` must be `gl_compatibility`.
4. Is the renderer's **Enabled** on?
5. Is **Outlines** on?
6. Is the scene running a second renderer? Only one `PixelRenderer` should be active.

## Outlines are missing or too heavy

| Look | Change |
|---|---|
| No outlines at all | **Outlines** on; **Pixel Preview** on; check the renderer is active |
| Outlines on the sky but not objects | **Background** on |
| Too many lines | Raise **Depth Thr.** and **Normal Thr.** |
| Lines disappear when the camera moves | Keep **Texel Snap**, **Sub-texel Scroll**, **Crisp Edges** on |
| Coloured outlines do nothing | **Wide Ids** on; assign an **Outline Id**; use a `PixelOutlineColors` resource or **Outline Col.** with **Own Color** on |

## The palette does nothing

- Did you tick **Use Palette**?
- Is **Mix** above `0`?
- Is a `PixelPalette` assigned to the renderer?
- Does the palette have colours? An empty palette quantizes to black.

## Textures are missing or the texture panel is empty

- The terrain has no **Tex Assign** resource. In the Inspector, assign `res://addons/MarchingSquaresTerrain/texture_presets/mst_classic.tres`, or choose a **Texture Preset** in the Paint strip.
- If you changed a slot's **Pattern**, it replaced the custom texture with the bundled pattern PNG. That is expected; undo if it was a mistake.
- If a texture looks stretched, adjust the slot's **Scale** (the repeat multiplier) in the slot resource. The panel slider is a convenience; the real value is `Magnify` on the slot resource.

## Foliage paint does nothing

- Is the Foliage tool selected (`5`)?
- Is an item selected in the right-side palette?
- Are you painting on a cell whose ground slot is allowed by the item's **Ground Slots**? Hand-painting ignores most rules, but the palette entry still has to be selected.
- If you are erasing, is **Erase All** on? That removes every prop, including ones you wanted.
- If the palette says “No texture set”, assign a texture preset first.

## Grass is missing

- The texture slot must have **Has grass** ticked. Grass, Meadow, Forest Floor and Moss have it by default.
- **Settings → Grass → Grass** must be on.
- **Min Height** must be below the ground you are looking at. Set it to the water level.
- The **Grass** feature must be on.
- Grass is small. Zoom in or check with **Pixel Preview**.

## Water is missing or looks wrong

| Symptom | Fix |
|---|---|
| No water at all | **Water** feature on; node visible; `Size` > 0 |
| Water is a flat solid colour | The terrain is missing or the water is far above the ground; the shader cannot read depth |
| No foam/shoreline | Same as above: the water needs the terrain height texture |
| No waves | Raise **Subdivisions**; press **Animate** in the editor |
| Waterfalls do not appear | Hex/triangle grid only; add raised **Water Cells**; lower **Min Levels** |
| Water covers the whole island | Lower the water node's Y |

## The terrain disappears when it rains

This is almost always the **cloud deck**, not the rain. If the camera is above the deck's **Height**, the semi-transparent deck sits between the camera and the ground. In heavy weather its coverage and opacity rise, and it washes the island out while the outlines still show through.

**Fix:** select `PixelSky` and either turn **Cloud Deck → Deck** off, or raise **Height** above the camera. The tutorial overview scenes raise the cloud height above their cameras for this reason. Cloud shadows still work either way.

If the island is visible but looks like a ghost, the **weather fog** is too thick for the camera distance. The tutorial 04 scene uses custom Rain/Snow/Storm presets with a tenth of the fog. For a far camera, lower the preset's **Fog Density**, or reduce the environment fog.

## Weather does nothing

- **Weather** feature on.
- **Weather** is not `Clear`.
- **Strength** > `0`.
- The node is near the ground, not under it or high in the sky.
- In the editor, weather changes are frozen unless **Animate** is on. In the game they work.
- If the sky does not darken, there is no `PixelDayNight`, or no `WorldEnvironment` for it to grade.

## Fireflies are invisible

- **Fireflies** feature on.
- A `PixelDayNight` exists.
- **Time** is night (`0.85–1.0`).
- **Night Only** is `1` and **Energy** is above `0`.
- The camera is inside the fireflies' **Extents** box? If not, move the node.

## Fire will not ignite

- The terrain has grass on the cell (slot **Has grass**, **Grass** on, above **Min Height**).
- The **Fire** feature is on.
- **Simulate** is on.
- **Ignite Tool** is on, and you clicked the terrain with the fire node selected. Or use **Hearths** for a permanent fire.
- The terrain has been edited since the fire node was added: call `refresh_fuel()` from a script, or re-add the node.

## Dust motes are invisible

- **Particles** feature on.
- **God Rays** and **Clouds** features on.
- The renderer's **God Rays** is on.
- **Beam** is `1`.
- **Pixel Preview** is on in the editor (particle fields are hidden without it).
- **Density** is above `0`.

## The builder UI does not appear

- A `PixelBuilderMesh` (or a `MeshInstance3D`) must be selected.
- The builder does not open for `PixelObject`, terrain, water or lights.
- If the bottom strip appears but the toolbar buttons do nothing, use the keyboard shortcuts (`1–4`, `C`, `B`, `5`, `W`, `E`, `I`, `Ctrl+B`, `Ctrl+R`). Some toolbar buttons are broken in older builds; report it.

## An imported model has no pixel look

- Is it under a `PixelObject`?
- Is **Convert** on?
- Are its materials `StandardMaterial3D`? Custom shader materials are kept only if they use the addon's pixel shader include.
- Does the model use a texture? Normal/roughness/metallic maps are not converted; only albedo and emission survive.
- Is **Pixel Preview** on in the editor?

## The editor is slow

1. Turn **Animate** off.
2. Turn **Pixel Preview** off while sculpting large areas.
3. Raise **PixelRenderer → Pixel Size** (fewer pixels).
4. Turn **God Rays** off.
5. Use **LOD Distance** on very large terrains.
6. Close other scenes and the game window.
7. On a laptop, make sure Godot is using the discrete GPU if it has one.

## The console is full of red errors

A few red errors on startup can be harmless. Errors that repeat every frame are not.

The most common one in older builds is:

```
Cannot connect to 'pressed': the provided callable is not valid:
'PixelBuilderPlugin::on_action'
```

That is a known bug in the builder toolbar; it was fixed in the current build. If you still see it, tell the programmer which build you are on.

When reporting a problem, include:

1. The scene you were in.
2. The exact red text from the Output panel (screenshot is fine).
3. A screenshot of the 3D view and the Scene tree.
4. What you clicked just before it happened.
5. Whether it happens in the editor, in the running game, or both.

## Reset a scene without losing everything

- `Ctrl+Z` is your friend. Every terrain stroke is one step.
- If a node is badly broken, delete it and add a fresh one from **Pixel World → Add**.
- If a whole scene is messy, open the original example and **Save Scene As…** a new copy.
- Never delete or overwrite the files in `demo/` while experimenting.

## Ask for help

If you are stuck for more than fifteen minutes, stop and ask. Include the scene name, a screenshot, and the Output text. It is much faster than fighting a tool that is actually doing what it was told.

Back to [[00 Start Here]]
