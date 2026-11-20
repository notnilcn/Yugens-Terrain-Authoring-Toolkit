# Cheat Sheet

One page, no explanations. Bookmark this.

## Editor

| Shortcut | Action |
|---|---|
| `F` | Frame selected node |
| Middle-drag | Orbit camera |
| `Shift` + middle-drag | Pan camera |
| Wheel | Zoom |
| `Q` / `W` / `E` / `R` | Select / Move / Rotate / Scale |
| `F5` | Play the project's main scene |
| `F6` | Play the current scene |
| `F8` | Stop |
| `Ctrl+S` | Save |
| `Ctrl+Z` / `Ctrl+Shift+Z` | Undo / redo |
| `F1` | Demo controls hint |
| `F3` | Demo diagnostics overlay |

## Terrain tools

| Key | Tool | What it does |
|---|---|---|
| `1` | Brush | Paint a selection and move it, or raise/lower directly |
| `2` | Level | Flatten to a height |
| `3` | Smooth | Relax the ground |
| `4` | Bridge | Make a ramp between two points |
| `5` | Foliage | Paint grass, trees, rocks, flowers |
| `6` | Paint | Paint one of 16 ground textures |
| `7` | Inspect | Print cell data to Output |
| `8` | Chunks | Add/remove/select chunks |
| `9` | Settings | Grid, grass, display, storage |
| — | Stamp | Drop a height-map mountain/crater/etc. |
| — | Gradient | Pull the ground toward a ramp between two points |
| — | Select | Copy/paste terrain |
| — | Tint | Paint a flat colour overlay |

## Brush modifiers

| Input | Action |
|---|---|
| `Shift` + click | Add to selection |
| `Ctrl` + click | Remove from selection |
| `Alt` + drag | Move only the hovered cell |
| Right click / `Esc` | Clear selection |
| `Shift` + wheel | Brush size |
| `Ctrl` + wheel | Strength |

## Stamp modifiers

| Input | Action |
|---|---|
| `Shift` + wheel | Size |
| `Ctrl` + wheel | Height |
| `Alt` + wheel, `Z` / `X` | Rotation |
| Click | Stamp once |
| Drag | Pin center, pull size/rotation, release to stamp |

## Select / paste

| Input | Action |
|---|---|
| `C` | Copy |
| `V` | Paste mode |
| `Z` / `X` | Rotate paste |
| `M` | Mirror paste |
| Left click | Paste |
| Right click / `Esc` | Leave paste mode |

## Pixel view

| Button | Action |
|---|---|
| **Pixel Preview** | See the pixel look in the editor |
| **Animate** | Animate clouds/water/grass/effects in the editor |
| **Pixel World** | Add effects and lighting presets |

## Builder

| Key | Action |
|---|---|
| `1` `2` `3` `4` | Object / Face / Edge / Vertex |
| `H` | Cycle modes |
| `W` | Select / Move tool |
| `C` | Draw shape (`Shift` + draw appends to the selected mesh) |
| `B` | Paint |
| `5` | Vertex paint |
| `E` / `I` / `Ctrl+B` | Extrude / Inset / Bevel |
| `Ctrl+R` | Insert edge loop |
| `M` / `J` / `F` | Merge / Connect / Fill |
| `Ctrl+T` | Triangulate |
| `Ctrl+D` | Duplicate |
| `Delete` | Delete |
| `G` / `Shift+G` | Grow / shrink selection |
| `L` / `Alt+L` | Edge loop / ring |
| `U` | UV editor |
| `[` / `]` | Grid step |
| `Esc` | Cancel / back out |

## Time of day

| Value | Time |
|---|---|
| `0.00` | Midnight |
| `0.25` | Sunrise |
| `0.50` | Noon |
| `0.75` | Sunset |
| `1.00` | Midnight again |

## Weather

`Clear` · `Cloudy` · `Foggy` · `Rain` · `Snow` · `Storm`

## What node do I add?

| I want… | Add / use |
|---|---|
| A terrain | `MarchingSquaresTerrain` |
| The pixel look | `PixelRenderer` |
| Clouds and cloud shadows | `PixelSky` |
| A day/night cycle | `PixelDayNight` |
| Rain/snow/lightning | `PixelWeather` |
| Water | `PixelWater` |
| Fireflies | `PixelFireflies` |
| Birds | `PixelBirds` |
| Campfire smoke/fire light | `PixelSmoke` (Fire on) + `PixelFireSystem` hearth |
| A lantern | `PixelLightVolume` |
| Light shafts through trees/pillars | `PixelSunShafts` |
| Dust motes/leaves | `PixelParticleField` |
| A spreading grass fire | `PixelFireSystem` under the terrain |
| Natural gullies | `MarchingSquaresErosion` under the terrain |
| Meadow flowers | `MarchingSquaresWildFlowers` under the terrain |
| A prop | `PixelObject` + a model, or `PixelBuilderMesh` |
| A palette | `PixelPalette` resource |
| Coloured outlines | `PixelOutlineColors` resource |

## Feature switches

`Project Settings → marching_squares_terrain/features`

Clouds · God Rays · Outlines · Outline Ids · Water · Weather · Birds · Fireflies · Smoke · Fire · Particles · Light Volumes · Sun Shafts · Grass

If an effect is invisible, check its switch first.

Add nodes from **Pixel World → Add** and the switch turns on automatically.

## Good starting values

| Setting | Value |
|---|---|
| PixelRenderer Pixel Size | `3` (720p) / `4` (1080p) |
| PixelRenderer Mode | `Anchor` |
| Camera | Orthographic, `Size` 18–30 for characters, 90–160 for islands |
| Outlines Depth Thr. | `0.35` |
| Outlines Normal Thr. | `0.5` |
| Outline Darken | `0.55` |
| Brush Size | `15` |
| Brush Strength (Raise/Lower) | `4` |
| Smooth Strength | `0.15` |
| Grass Min Height | Same as the water Y |
| Sun Shadow Max Distance | `80–120` |
| Day Length | `600–1080` seconds |

## Water checklist

1. **Water** feature on.
2. `PixelWater` Y = water line.
3. `Size` covers the map.
4. A terrain exists in the scene.
5. For waterfalls: hex or triangle grid + raised water cells.

## Effects checklist

1. Feature switch on.
2. Node in the scene, near the ground/action.
3. Sun and environment wired (or a sibling exists).
4. **Animate** on in the editor.
5. **Pixel Preview** on for particle fields.
6. Time of day is night for fireflies.
7. Grass exists for fire.
8. God Rays + Clouds on for dust motes.

## Five rules that save time

1. Save before experimenting.
2. If the terrain toolbar is missing, select the terrain.
3. If the pixel look is missing, check Pixel Preview.
4. If an effect is missing, check its feature switch.
5. If a stroke goes wrong, `Ctrl+Z` — every stroke is one undo.
