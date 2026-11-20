# Water, Fire and Atmosphere

This is the layer that makes a scene feel alive: water, birds, fireflies, smoke, fire, dust and light shafts. Most of these are separate nodes you add with **Pixel World → Add**, and most of them are optional.

![[effects.mp4]]

## Water

**PixelWater** is a banded water plane. Any terrain below the plane's Y is flooded.

1. Add `PixelWater` (or **Pixel World → Add → Water**).
2. Move the node up/down so its **Y** is the water line. The terrain does the rest.
3. Set **Size** large enough to cover the play area.

![[water-inspector.png]]

| Setting | What it does | Good range |
|---|---|---|
| **Size** | Width and depth of the plane, in metres, centered on the node. | 2–3× the map |
| **Subdivisions** | Mesh detail for the waves. More = smoother and more expensive. | `64–225` |
| **Stuff** | The water material. Leave it empty to create the default. |
| **Caustics → Strength** | Bright light patterns in the shallows and on nearby terrain. | `0.3–0.6` |
| **Caustics → Scale / Bands / Reach** | Pattern frequency, band count, and how high up the shore the caustics reach. | Defaults |
| **Waterfalls → Terrain** | The terrain the raised water belongs to. Empty = first terrain. |
| **Waterfalls → Water Cells** | Pools and rivers above sea level, one entry per raised cell: `(q, r, surface height)` on hex, `(x, y, surface height)` on triangle. |
| **Waterfalls → Min Levels** | How big a drop must be to become a waterfall sheet. |
| **Waterfalls → Cascade Stuff** | The waterfall material. |

> [!warning]
> **Pools and waterfalls only work on hex and triangle grids.** On a square-grid terrain you get the flat sea/lake plane, but no raised pools or falling water.

> [!tip]
> The water shader reads the terrain's height. If the water is uniformly deep blue and has no shoreline or foam, the terrain is missing or the water is far above/below the ground.

### Getting the water look right

The default water material is deliberately blue and readable. To change it, select the water and open the material (or the shader parameters) in the Inspector. The most useful parameters are:

- **Depth scale / depth bands** — how quickly the water goes from turquoise shallows to deep blue.
- **Foam width** — the white band along the shore.
- **Swell height** — how tall the waves are.
- **Horizon start / end** — blends the far water into the sky colour.

The exact names depend on the shader version; adjust them live while the game is running or with **Animate** on in the editor.

### Waterfalls

1. Use a **hex** or **triangle** terrain.
2. Add `PixelWater` and set **Waterfall → Terrain** to the terrain.
3. Add raised water cells: `(q, r, height)` in **Water Cells**. Height is in terrain-local units.
4. Set **Min Levels** so only real drops get a waterfall sheet. Smaller drops become short cascades.
5. Tune **Cascade Stuff** for the fall colour and foam.

The island demo uses a set of raised cells to make its pond and waterfall. Open `demo/hex_island.tscn` and select `PixelWater` to see the values.

## Fireflies

**PixelFireflies** is a swarm of glowing dots that drift and blink. It fades in at night automatically.

| Setting | What it does | Good range |
|---|---|---|
| **Count** | Number of fireflies. | `50–120` |
| **Extents** | Half-size of the box they roam in. | `(10, 2, 10)` |
| **Tint** | Colour. | Warm yellow |
| **Size** | Sprite size in metres. | `0.1–0.2` |
| **Night Only** | `1` = only at night, `0` = always. | `1` |
| **Wander / Blink Speed** | Movement and blink rate. | Defaults |
| **Energy** | Brightness. | `2–4` |

> [!warning]
> Fireflies are invisible without a **PixelDayNight** node, because they read the night value. They also need the **Fireflies** feature. If the scene is at noon, they will not show.

## Birds

**PixelBirds** is a small flock that circles, flaps, and occasionally lands.

| Setting | What it does | Good range |
|---|---|---|
| **Count** | Number of birds. | `5–10` |
| **Radius** | Size of the circle they fly around. | Half the play area |
| **Altitude** | Flight height above the node. | `6–12` |
| **Speed / Flocking** | How fast and how tightly they flock. | Defaults |
| **Size** | Wingspan in metres. | `0.7–1.0` |
| **Min / Max Texels** | Keeps them readable at a distance. | `8–18` |
| **Landing / Land Interval / Perch Seconds** | They land on high terrain and sit for a while. | `30–60` / `10` |
| **Scare Distance** | How close the camera focus gets before they fly off. | `5` |
| **Preview In Editor** | Simulate in the editor. | On |

Birds need the **Birds** feature, a terrain to land on, and a PixelRenderer for the texel size clamp. Without a terrain they “land” in mid-air at the node's height.

## Smoke and campfires

**PixelSmoke** is a plume of soft pixel puffs. With **Fire** on, it also adds flames and a flickering light pool at the base.

| Setting | What it does | Good range |
|---|---|---|
| **Emitting** | Master switch. | On |
| **Rate** | Puffs per second. | `2–7` |
| **Lifetime** | Seconds each puff lives. | `4–9` |
| **Rise Speed / Spread** | How fast and how wide the plume rises. | `0.6–1.2` / `0.1` |
| **Start Size / End Size** | Puff size at birth and death. | `0.1` / `0.4–1.0` |
| **Use Sky Wind / Wind** | Drifts with the PixelSky wind. | On |
| **Detail** | Puff shape detail. | `6` |
| **Fumes Tint / Ash Tint** | Pale smoke and dark soot. | Defaults |
| **Fire** | Adds flames and a light. | On for a campfire |
| **Fire Tint / Light Energy / Light Range** | Flame colour and light. | Warm orange |
| **Flicker** | Light flicker amount. | `0.3` |

For a campfire, put `PixelSmoke` at the fire base, set **Fire** on, and add a `PixelFireSystem` hearth at the same spot (see below). Enable both the **Smoke** and **Light Volumes** features: the fire light is a `PixelLightVolume` inside the smoke node.

## Light volumes

**PixelLightVolume** is a lantern, campfire or crystal light. It draws an omni light plus a banded, dithered pool of light on the ground and a small glowing source orb.

| Setting | What it does | Good range |
|---|---|---|
| **Radius** | Light range. | `3–5` |
| **Lamp Tint** | Light colour. | Warm amber |
| **Energy** | Brightness. | `0.1–0.3` |
| **Flicker** | Flicker amount. `0` = steady lamp. | `0.1–0.3` |
| **Shadows** | Expensive in Compatibility. | Off |
| **Glow → Pool Scale / Pool Energy** | Size and brightness of the ground pool. | `1.2` / `0.4–0.8` |
| **Glow → Source → Radius / Energy** | The little glowing orb. | `0.3–0.6` / `0.6–1.0` |
| **Night Boost** | How much dimmer the glow is by day. | `1` |

Needs the **Light Volumes** feature. Dozens of them are expensive; use a few important lights and let the sky do the rest.

## Sun shafts

**PixelSunShafts** draws light shafts through the gaps in a shadow-casting sun — the classic “light between pillars” look.

| Setting | What it does | Good range |
|---|---|---|
| **Sun** | The directional light. It must cast shadows. | Wire it |
| **Count** | Number of sheets. More = denser and more expensive. | `10–14` |
| **Depth / Size** | Thickness and coverage of the stack. | `15–25` / `100–250` |
| **Beam Tint** | Shaft colour. | Warm white |
| **Alpha** | Per-sheet brightness. | `0.008–0.012` |
| **Start Depth / Fade Height** | Keeps shafts out of the camera's nose and out of the open sky. | `10–20` / `8–15` |

Needs the **Sun Shafts** feature, a shadow-casting sun, and a shadow distance large enough to reach the shafts. This is different from the renderer's **God Rays** (which go through clouds). Use one or the other, not both, unless you know why.

## Dust and particles

**PixelParticleField** draws baked pixel particles on stacked world-anchored sheets: dust motes in sunbeams, falling leaves, pollen, embers, ambient snow.

| Setting | What it does |
|---|---|
| **Preset** | Dust Motes, Rain, Snow, Embers, Leaves. Changing it replaces the values below. |
| **Emitting** | Master switch. |
| **Density** | Fraction of particles. |
| **Weather Response** | Always, Fair Weather, Rain, Snow. |
| **Particle Count / Lifetime / FPS** | How many, how long, how fast the flipbook plays. |
| **Velocity / Drift / Sway** | Motion. |
| **Layers / Spacing / Texel Size** | How many sheets, how far apart, and how big a particle texel is. |
| **Focus Radius** | Keeps particles near where the camera looks. |
| **Tint / Color B / Brightness** | Look. |
| **Sunlit / Beam / Shade / Night** | Light response. |

> [!warning]
> **Dust Motes** only appears inside god-ray shafts. It needs **God Rays** and **Clouds** on, and `Beam = 1`.
>
> Particle fields are **hidden in the editor unless Pixel Preview is on**. That is by design; they are not broken.

## Fire

**PixelFireSystem** simulates a grass fire: it spreads with the wind, throws embers, chars the ground, shrinks the grass, is doused by rain, and regrows.

The easiest use is a **permanent campfire**:

1. Add `PixelFireSystem` as a **child of the terrain**.
2. Set **Hearths** to the fire's position in the node's local space (often `(0, 0, 0)`).
3. Set **Hearth Radius** to `0.5–1.0`.
4. Enable the **Fire** and **Light Volumes** features.

For a spreading grass fire:

1. The terrain must have grass on at least one slot, and **Grass** must be enabled.
2. Set **Ignite Tool** on, select the fire node, and click the meadow. `Shift` + click douses.
3. Or call `ignite_here()` from a script.

| Setting | What it does |
|---|---|
| **Simulate** | Freeze the simulation. |
| **Texel Size** | Fire cell size. |
| **Spread Rate / Burn Time** | How fast it spreads and how long it burns. |
| **Ember Rate / Ember Distance** | Spot fires thrown by the wind. |
| **Use Sky Wind / Wind Strength** | Wind-driven spread. |
| **Douse Rate / Wet Rate / Dry Time** | Rain response. |
| **Regrow Delay / Regrow Time** | How the grass recovers. |
| **Hearths / Hearth Radius** | Permanent fires that never spread. |
| **Max Flames / Max Smoke / Max Lights** | Caps. Lower = cheaper. |

> [!warning]
> No grass = no fuel = the fire does nothing. If `ignite` does nothing, check that the terrain's grass is on and that the texture slot under the click has **Has grass**.

## Erosion

**MarchingSquaresErosion** is a child node of the terrain that carves natural gullies with simulated droplets.

1. Select the terrain.
2. Go to **Settings → LOD & Nature** and click **Add** next to Erosion.
3. Select the new erosion node.
4. Set **Iterations** (`20,000–60,000` for a medium map).
5. Press **Erode**. Each press is one undo step.

Start with **Mode = Auto**. On hex and triangle grids it keeps terraced steps; on square grids it smooths. Increase **Pixels Per Cell** for finer channels, and raise **Iterations** for deeper erosion. If it is too strong, lower **Carve Rate** before lowering iterations.

## Wild flowers

**MarchingSquaresWildFlowers** scatters clustered meadow flowers on the terrain.

1. Select the terrain → **Settings → LOD & Nature** → **Add** next to Wild Flowers.
2. Select the new node and set **Water Level** to your sea level.
3. Press **Scatter Wild Flowers**.

Useful settings: **Density**, **Patch Scale** (bigger drifts), **Max Slope**, **Ground Water**, **Shore Wetness**, and **Trees Shade** (fewer flowers under trees). The nine species are editable `PixelScatterItem`s, so you can change their models, colours and sizes after scattering.

## A living-world checklist

1. Terrain, ground textures, grass.
2. Sky and time of day.
3. Water at the right Y.
4. Weather at ground height.
5. Birds, fireflies, dust.
6. One or two light volumes and a campfire.
7. Sun shafts only if the composition needs them.
8. Play it and look from the player camera. Turn off anything you cannot see.

Next: [[10 Making Props in the Builder]]
