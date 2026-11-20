# Painting the Ground

Sculpting decides the shape. Painting decides what the ground is made of: grass, sand, rock, snow, paths. The terrain has **16 texture slots**. You paint with one slot at a time, and you can paint the top of the ground or the vertical walls separately.

![[painting-textures.mp4]]

## The two places to paint from

Select the terrain, then click **Paint** (`6`) in the toolbar.

1. **The bottom strip** has the brush settings and a **Material** dropdown.
2. **The right-side panel** shows all 16 slots with a preview and their settings.

![[paint-strip.png]]

![[texture-panel.png]]

Click a slot's name in the right panel to make it the active material. The strip's **Material** dropdown is the same choice.

## The 16 slots

| Slot | Default name | Grass? | Typical use |
|---|---|---|---|
| 1 | Grass | Yes | Meadows and lawns |
| 2 | Meadow | Yes | Lighter, flowery grass |
| 3 | Forest Floor | Yes | Dark ground under trees |
| 4 | Sand | No | Beaches and deserts |
| 5 | Dirt | No | Worn paths and fields |
| 6 | Cliff | No | Vertical rock faces |
| 7 | Dark Rock | No | Volcanic ground and caves |
| 8 | Snow | No | Mountain tops |
| 9 | Gravel | No | Roads and riverbanks |
| 10 | Clay | No | Dry, cracked ground |
| 11 | Moss | Yes | Wet stones and old ruins |
| 12 | Path | No | Stone or packed paths |
| 13 | Stone Tiles | No | Plazas and floors |
| 14 | Ash | No | Burnt ground |
| 15 | Ice | No | Frozen water |
| 16 | Void | No | **Cuts a hole in the terrain** |

> [!warning]
> Slot 16, **Void**, does not paint a texture. It removes the terrain surface, leaving a hole. That is useful for cave entrances, but it can also make the ground disappear if you use it by accident. `Ctrl+Z` brings it back.

## Painting

1. Choose a slot.
2. Drag on the ground. The brush paints with the selected material.
3. `Ctrl` + left click picks the material under the cursor. This is the fastest way to match an existing area.
4. `Shift` + wheel changes the brush size.
5. Turn **Falloff** off if you want a hard, crisp edge. With Falloff on, only the center of the brush reaches the 0.5 weight needed to paint, so the edge is soft.

### Painting cliffs and walls

The top of the ground and the vertical faces are separate. Tick **Paint Walls** to paint the vertical faces instead of the top. A cliff usually needs a different texture (Cliff or Dark Rock) on its wall than on its grassy top.

### Restricting by slope

**Slope Min** and **Slope Max** limit painting to ground within a steepness range. For example, set `Slope Min = 35` and paint with Cliff to put rock only on steep faces, without painting the flat meadow.

## Grass

Grass does not come from the Paint tool. It comes from the terrain's **Grass** setting, and it only grows on slots with **Has grass** ticked.

- In the texture panel, the **Has grass** checkbox and the grass color belong to each slot.
- **Grass sprite** is the little leaf image used on that slot. Leave it empty to use the terrain's default grass sprite.
- The terrain's **Settings → Grass** page controls the blade size, how many blades per cell, the wind animation, and the minimum height (set this to your water level so grass does not grow underwater).

If you paint a slot with no grass, the grass in that area disappears. That is how you make dirt paths through a meadow.

## Texture presets

A **texture preset** is a saved set of all 16 slots. It is the fastest way to give a new terrain a complete look.

1. Select **Paint**.
2. At the bottom strip, open **Texture Preset** and choose a preset.
3. The terrain gets a **copy** of the preset. Editing the terrain's slots will not change the preset file.

The addon ships with **MST Classic**. The demo scenes use their own richer sets, which are embedded in the demo scenes rather than saved as preset files.

### Saving your own preset

1. Set up the 16 slots the way you like them.
2. Scroll to the bottom of the right-side texture panel.
3. Click **Export Texture Preset**.
4. Give it a name. It is saved as a `.tres` file in `addons/MarchingSquaresTerrain/texture_presets/` and appears in the dropdown next time.

> [!warning]
> Presets are stored inside the addon folder. That is the addon's data, not your scene. Do not rename or delete files there unless you mean to remove the preset from every project.

## Auto Texture Rules (optional, advanced)

At the bottom of the Paint panel there is a section called **Auto Texture Rules**. It paints the *unpainted* ground automatically:

- **Rock** on slopes steeper than a threshold.
- **Shore** sand near the water level.
- **Snow** above a height line.
- **Cliff lip** along the top edge of cliffs.
- A **filler** texture mixed in by noise to break up repetition.

Turn on **Auto Texture** and set:

| Field | Meaning |
|---|---|
| **Unpainted** | The slot that the rules are allowed to replace. Leave this as Grass if grass is your base. |
| **Noise** | How rough the edges of the rules are. |
| **Rock / Rock Slope** | The rock slot and the angle where it starts. 35–45° is a good start. |
| **Shore / Water Level / Shore Height** | Sand around the waterline. Set **Water Level** to the same Y as your water plane. |
| **Snow / Snow Height** | The snow line and how far it wanders. |
| **Cliff Lip / Lip Width** | A band of dirt or rock along the top of a cliff. |

Anything you paint by hand wins over the rules. The rules only fill the areas you have not painted.

> [!tip]
> Auto rules are a great first pass, but do not rely on them for Hero areas. Hand-paint the places the player will stand and look at.

## A painting order that works

1. Paint the whole terrain with Grass.
2. Paint Sand around the water and Path where people walk.
3. Paint Cliff on the vertical walls.
4. Paint Snow on the highest ground.
5. Add small patches of Dirt, Gravel or Moss for variety.
6. Turn on Auto Texture Rules for a second pass, then hand-fix the important spots.

Next: [[05 Plants, Trees and Props]]
