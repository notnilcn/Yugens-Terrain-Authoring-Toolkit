# Sculpting

Sculpting is raising, lowering and flattening the ground. There are four tools for it, and they are meant to be used together: **Brush** makes the shape, **Level** makes flat areas, **Smooth** fixes lumps, and **Bridge** makes ramps.

> [!tip]
> Work from big shapes to small details. Make the mountain, then the cliff, then the path, then smooth. Do not start by smoothing a bump you have not shaped yet.

## Brush

**Brush** is the main tool. The toolbar button says `Brush` and the shortcut is `1`. It has two modes, chosen in the strip at the bottom with **Brush Mode**.

### Mode 1 — Drag (the default)

Think of it as “select, then move”.

1. Click or drag on the ground. White cells appear. This is a **selection**.
2. Release.
3. Press on a white cell and drag up or down. The whole white shape moves.

| While selecting | What happens |
|---|---|
| `Shift` + click | Add cells to the selection |
| `Ctrl` + click | Remove cells from the selection |
| `Alt` + drag | Move only the cell under the cursor |
| Right mouse / `Esc` | Clear the selection |

![[brush-drag.mp4]]

The **Flatten** checkbox (on by default) makes the dragged selection flat at the height you release at. Turn it off if you want to keep the shape's relative bumps while moving it.

### Mode 2 — Raise / Lower (the fast way)

Change **Brush Mode** to `Raise / Lower`. Now you can sculpt like clay:

| Input | Result |
|---|---|
| Hold **left mouse** | Raise the ground |
| Hold **right mouse** | Lower the ground |
| Hold **left + right** | Flatten toward the **Height** setting |
| `Ctrl` + left click | Pick the **Ceiling** from the ground |
| `Ctrl` + middle click | Pick the **Height** |
| `Ctrl` + right click | Pick the **Floor** |
| `Shift` + wheel | Brush size |
| `Ctrl` + wheel | Strength |


The **Ceiling**, **Height** and **Floor** values stop you from going too far. They are training wheels, not limits on the terrain itself. Set them to values that make sense for your map (for example, floor `0` if the sea is at `0`).

### Brush settings

| Setting | What it does | Artist advice |
|---|---|---|
| **Brush Type** | Round, Square, Hexagon, Triangle | Round for hills; Square and Hexagon for blocky shapes. |
| **Size** | Diameter in world units | `Shift` + wheel is faster than the slider. |
| **Aspect** | Stretches the brush along one axis | Use for long ridges. |
| **Rotation** | Rotates the brush | Pairs with Aspect. |
| **Flatten** | Drag mode: keep the selection flat | Leave on for plateaus. |
| **Falloff** | Soft edge | Turn off for hard-edged blocks. |
| **Peak** | Falloff shape: pointy tops vs flat mesas | Negative values make mesas. |
| **Grid Aligned** | Snap to whole cells | Useful for hex and triangle grids. Requires **Falloff off**. |
| **Quick Paint** | Also paint a texture while sculpting | `Base Grass Masked` is handy for keeping grass under new hills. |

> [!warning]
> **Falloff** is shared by several tools. If **Grid Aligned** is grayed out, switch back to Brush, turn **Falloff** off, then return to the tool you were using.

## Level

**Level** flattens toward a chosen height. Use it for plateaus, building foundations, roads and lake beds.

1. Select **Level** (`2`).
2. Set **Height** in the bottom strip, or `Ctrl` + left click the ground to copy the height under the cursor.
3. Drag over the area.

![[level-smooth.mp4]]

> [!tip]
> Level is the fastest way to make a believable cliff top: raise a rough hill with Brush, then Level the top at one height.

## Smooth

**Smooth** relaxes the ground toward the average of its neighbours. It is the “make it look natural” button.

1. Select **Smooth** (`3`).
2. Choose **Smooth Mode**:
   - **Terraced** keeps flat steps. This is the blocky, stair-like look.
   - **Sloped** turns smoothed cells into slanted planes, like a ramp.
3. Drag. **Strength** controls how quickly it changes.

Smooth is also good for cleaning up the seam where two brush strokes meet, and for making a cliff face less jagged.

> [!warning]
> It is possible to over-smooth. If the terrain starts looking like melted ice cream, undo and use a lower **Strength**.

## Bridge

**Bridge** builds a ramp between two points. Use it for paths up a cliff, ramps into a village, or bridges over a gap.

1. Select **Bridge** (`4`).
2. Set **Size** to the width of the ramp.
3. Drag from the **start** point to the **end** point. A white line previews the direction.
4. Release.

![[bridge.mp4]]

**Ease Value** changes the height curve:

| Ease | Shape |
|---|---|
| `1` | Straight line |
| `2` | Slow start, fast finish |
| `0.5` | Fast start, slow finish |
| `-1` | Smooth in and out (the default) |
| `-5` | Very smooth, almost S-shaped |

The bridge uses the brush **Size** as its width. Brush Type and Grid Aligned are shown in the strip but do not affect the bridge.

## A reliable sculpting recipe

1. Block in the overall shape with **Brush** at a large size.
2. Flatten important areas with **Level**.
3. Cut paths and ramps with **Bridge**.
4. Smooth only where it looks rough.
5. Add a second pass of small brush strokes for detail.
6. Save (`Ctrl+S`) and press `F5` to look at it from the player's camera.

> [!tip]
> Look at the scene from the game camera while you sculpt. A hill that looks great from above can hide the path from the player's eye level.

Next: [[04 Painting the Ground]]
