# Capture notes — page `08-atmosphere-effects` (Weather, Water & Mood)

> **2026-11-23 re-take (session `docs-atmos2`, after an addon fix + QA template refresh).**
> A fix landed that (a) stops lighting presets from leaving **fog switched on** — the sticky
> "mint-green wash" after cycling presets is gone, and **Storm** now renders correctly as a
> dark, foggy downpour; and (b) makes **weather changes apply instantly with Animate OFF**
> instead of freezing half-blended. The fixer also confirmed **Birds / Light Volume / Sun
> Shafts are NOT broken** in the editor (they preview fine — see the corrected facts below).
> All affected media was re-shot in fresh state with a consistent close island framing; the
> stale caveats in §0 and §7 and the wrong per-node notes in §2g/§2i/§2j were corrected.
> Re-taken / new files are listed at the end of §5.

Area C07. Session `docs-atmos`, display `:77`, port `8877`,
scene `res://tutorial/04_world_mood.tscn` (rich: terrain, water, sky, day/night, weather,
fireflies, birds, dust). Source of truth for the writer agent. Everything below was verified
in the running editor and cross-checked against the Rust source under
`addons/MarchingSquaresTerrain/rust/crates/mst/src/effects/`, `.../editor/fire_tool.rs`,
`.../editor/pixel_ui/sections.rs`, `.../fire_section.rs`, `.../presets.rs`.

This page owns the **atmosphere / effect nodes** and their pixel-UI sections:
PixelSky, PixelDayNight, PixelWeather, PixelWater (+ waterfall), PixelFireflies,
PixelParticleField, PixelBirds, PixelSmoke (smoke + fire), PixelLightVolume, PixelSunShafts,
PixelFireSystem (Grass Fire + the fire/ignite tool). Page 07 (another agent) owns the
renderer-level look (pixel size, outlines, palette, god rays, debug) — this page points back
to it with "see page 07". The right-side **Lighting Preset** buttons and **Wire Sun / Env
Refs** button are documented by page 07 (panel owner) but their *effect* on these nodes is
shown here.

---

## 0. INVESTIGATION — the "Animate-only preview refresh" question (NOT A DEFECT)

The page-07 (pixelizer) agent flagged a suspected defect: that the Sky / Day-Night editor
**preview only refreshes with Animate on**, and that lighting-preset globals stick for the
session. My brief asked me to confirm whether this is a real, blocking defect before
capturing. **It is not.** Findings (all with Pixel Preview ON, Animate OFF, scene
`04_world_mood`):

- **Changing a Day/Night field refreshes the viewport immediately, Animate OFF.**
  `time_of_day` 0.72 → 0.50 (noon) via a plain property write (`node.set`, which is exactly
  what the editor UI does) changed **48 %** of viewport pixels; 0.50 → 0.95 (night) changed
  **58 %** (mean 79/channel). Visually confirmed a full dusk→noon→night shift (brightness,
  shadow direction).
- **A REAL slider drag refreshes too.** Dragging the Day/Night **Time** spin-slider with
  Animate OFF changed the viewport **55 %**. (The drag overshot the 0–1 range to 62.8 — the
  known `EditorSpinSlider` synthetic-drag overshoot artifact noted on page 07, not a bug;
  a human dragging slowly stays in range. The point is: the preview updated.)
- The harness `set-prop` is a bare `node.set(prop, value)` with **no** forced redraw
  (`qa/harness/harness.gd` `_cmd_set_prop`), so set-prop and a human edit take the identical
  path. Both refresh.
- **What Animate actually controls** (`clock.rs`): `clock::step()` returns 0 while
  `is_editor() && !animate_in_editor`, so with Animate OFF the shared clock is **frozen** —
  effects do not *advance on their own* (clouds don't drift, Day/Night "Running" does not move
  the sun by itself, rain doesn't fall, birds don't fly). A one-off **edit** still refreshes.
  Runtime (`!is_editor()`) always animates, so everything updates there.
- So the pixelizer agent's "stale after Animate off" impression is just Day/Night **Running**
  on + Animate off = the sun holds still until you Animate or nudge Time. **Editing any field
  still refreshes the still picture immediately.** Neither blocks capture.

**UPDATE (2026-11-23 fix):** the earlier "lighting-preset globals stick for the session"
behaviour (a sticky mint-green wash after cycling Storm/Snow presets) **has been fixed** —
presets no longer leave fog switched on, so each preset now reads correctly even after
cycling through all six, and a preset + Ctrl+Z reverts cleanly (verified: undo-after mean
RGB `[104,146,150]` ≈ undo-before `[103,146,149]`, not the old washed `~190`). Separately,
**weather now applies instantly with Animate OFF** (verified: `set weather = Storm` with
`transition_seconds = 6` and Animate off snapped fully to the dark stormy look in one frame,
no half-blend). Both were re-shot in this state.

**Gotcha for the writer:** tell the reader that **Animate** (3D-view menu bar, page 07) must
be ON to *watch* motion (drifting clouds, falling rain/snow, flying birds, flickering fire,
the day/night cycle running). Editing a field updates the still picture right away either way.

---

## 1. The scene (`04_world_mood.tscn`) and how atmosphere nodes work

A little green island in a wide turquoise lagoon (a hut sits on it). The scene already contains
the atmosphere family wired up:

| Node (Scene tree) | Class | What it does |
|---|---|---|
| WorldEnvironment | WorldEnvironment | holds the Godot Environment the Day/Night node grades |
| Sun | DirectionalLight3D | the sun light the sky / day-night / god-rays follow |
| MarchingSquaresTerrain | MarchingSquaresTerrain | the island + lagoon ground (has grass) |
| PixelWater | PixelWater | the lagoon water (depth bands, foam, waves) |
| Camera | MstOrbitCamera | the game camera (orbits at runtime) |
| PixelRenderer | PixelRenderer | the pixel look (page 07) |
| PixelSky | PixelSky | cloud shadows + overhead cloud deck, publishes sun/cloud globals |
| PixelDayNight | PixelDayNight | time of day, sun arc, sky/ambient grade, fog, rim light |
| PixelWeather | PixelWeather | Clear/Cloudy/Foggy/Rain/Snow/Storm + optional snow |
| Fireflies | PixelFireflies | night swarm |
| Birds | PixelBirds | boids flock |
| Dust | PixelParticleField | dust motes (world-anchored flipbook) |

Added during capture via the Pixel World panel "+ Add": **Smoke** (PixelSmoke), **LightVolume**
(PixelLightVolume), **SunShafts** (PixelSunShafts), **FireSystem** (PixelFireSystem, under the
terrain). These were NOT saved to the scene (session only).

**Two ways to add an effect** (both documented on page 07, used here):
- **Pixel World panel → "+ Add"** (right side): inserts the node with its sun / environment /
  camera refs auto-wired, one undo step. Terrain-child families (Grass Fire, Erosion, Wild
  Flowers) add **under the terrain** and need a terrain in the scene (this scene has one, so it
  "just works"; with several terrains you must select one first).
- **Scene dock → Add Child Node** (search the class name, e.g. `PixelSmoke`): the manual Godot
  way; you then wire the refs yourself (or press **Wire Sun / Env Refs** in the panel).

**Selecting an effect node** swaps the bottom attribute strip to that node's section (icon on the
left pixel toolbar, settings in the strip, split into **page tabs** across the top). Fields edit
the same way as page 07: spin-slider drag = one undo step, checkbox, dropdown, colour button;
every change is `Set <Section> <Field>` in Undo History (Ctrl+Z).

**Animate (menu bar, page 07) and the preview** — see §0: an *edit* to any field refreshes the
still picture immediately even with Animate OFF; **Animate ON is only needed to watch motion**
(clouds drift, the day/night clock run, rain/snow fall, birds fly, fire flicker). Fresh editor
sessions start with the clock frozen.

---

## 2. Feature inventory (every control, by node)

Ranges/defaults are from `editor/pixel_ui/sections.rs` (+ `fire_section.rs`). Field tooltips in
the strip follow the page-07 pattern `<Label>\n<Class>.<prop>\nRange a to b (drag, or click to
type)`. Section (toolbar-icon) tooltips are quoted verbatim below.

### 2a. PixelSky — "Sky & Clouds" (one strip, no page tabs) — `30-sky-strip.png`
Section tooltip: *"Sky & Clouds — PixelSky: cloud shadows on the ground and the optional cloud
deck. Its clouds drive the shared cloud field (pt_cloud_* globals) that terrain, grass, water and
god rays read; with several skies, `cloud_camera_scope` picks the one for each camera."*

| Field | prop | Type | Range / default | What it does |
|---|---|---|---|---|
| Clouds | clouds_enabled | bool | on | Master switch for cloud shadows + the shared cloud field. |
| Coverage | coverage | float | 0–1 / 0.45 | How much of the sky is cloudy (more = more/bigger shadow blobs). **Headline knob.** |
| Softness | softness | float | 0–0.5 / 0.12 | Soft vs hard cloud-shadow edges. |
| Scale | cloud_scale | float | 0.0015–0.2 / 0.02 | Cloud pattern size (small = tiny puffs, large = big banks). |
| Wind X | wind (x) | float | −1–1 / 0.02 | Cloud drift speed/direction, east-west. |
| Wind Y | wind (y) | float | −1–1 / 0.011 | Cloud drift, north-south. |
| Height | cloud_height | float | 1–500 / 300 | Height of the cloud deck plane. |
| Shadow | shadow_strength | float | 0–1 / 0.45 | How dark the cloud shadows fall on the ground. |
| Bands | bands | int | 0–8 / 0 | Posterise the cloud shading into N bands (0 = smooth). |
| Deck | deck_enabled | bool | on | Draw the overhead cloud **deck** (the visible cloud sheet in the sky). |
| Deck Size | deck_size | float | 10–4000 / 500 | Size of the deck plane that follows the camera. |
| Opacity | deck_opacity | float | 0–1 / 0.85 | Deck cloud opacity. |
| Color | deck_color | Color | white-ish | Lit cloud colour. |
| Shade | deck_shade_color | Color | lavender-grey | Shaded cloud colour. |

### 2b. PixelDayNight — "Day / Night" — tabs **Time · Sun & Fog · Rim Light**
`10-daynight-time.png`, `11-daynight-sunfog.png`, `12-daynight-rimlight.png`.
Section tooltip: *"Day / Night — PixelDayNight: sun arc, sky/ambient grading, fog, cel light ramp
and rim light over the day."*

**Time tab** (0–8): Time `time_of_day` (0–1, 0=midnight, 0.5=noon; default 0.72), Override
`time_override` (−1–1, −1 = off/"let it run"), Running `running` (bool, on — advances the clock
when Animate/runtime is live), Day Length `day_length_seconds` (1–3600 / 600), Time Scale
`time_scale` (0–100 / 1), Latitude `latitude_tilt_deg` (0–80 / 35 — how high the sun climbs),
Azimuth `azimuth_deg` (−180–180 / 20 — compass direction of the sun's arc), Shadows `sun_shadows`
(bool). (Inspector also shows **Twilight Stretch** 0.7.)

**Sun & Fog tab** (8–14): Sun Energy `sun_energy_scale` (0–4 / 1), Ambient `ambient_energy_scale`
(0–4 / 1), Fog `apply_fog` (bool), Fog Day `fog_density` (0–0.1 / 0.0025), Fog Night
`fog_night_density` (0–0.1 / 0.0065), Light Ramp `light_ramp_preset` (enum: Off/Classic/Soft/
Hard/Warm Shadows/Custom — the cel-shading light ramp).

**Rim Light tab** (14+): Light Wrap `light_wrap` (−0.9–1), Rim `rim_strength` (0–2), Rim Width
`rim_width` (0–1), Rim Back `rim_backlight` (0–1). Cel light-wrap + rim/backlight glow.

Headline knob: **Time** (the whole scene recolours; series `13-tod-000/025/050/075.png`).

### 2c. PixelWeather — "Weather" — tabs **Weather · Snow · Particles · Rain · Lightning · Colors**
`20-weather-strip.png`, `22`–`26`. Section tooltip: *"Weather — PixelWeather: blends Clear /
Cloudy / Foggy / Rain / Snow / Storm (clouds, light, fog, wet ground, pixel rain, lightning) and
optional snow … API: set_weather("rain")."*

**Weather tab** (0–4): **Weather** `weather` (enum Clear 0 / Cloudy 1 / Foggy 2 / Rain 3 / Snow 4
/ Storm 5), Transition `transition_seconds` (0–120 — how long a weather change takes to blend),
Ground `ground_seconds` (0–600 — how long wet/snow ground builds), Strength `strength` (0–1 / 1).
**Snow tab** (4–15): Snow `snow_enabled` (bool — *extra* falling snow on top of any weather),
Intensity `snow_intensity`, Accum./min `snow_accumulation` (0–10), Melt/min `snow_melt` (0–10),
Max Cover `snow_max_cover` (0–1), Flakes `snow_flakes` (0–4000), Flake Size `flake_texels` (1–3),
Big Flakes `snow_big_flakes` (0–1), Fall Speed `snow_speed` (0.1–10), Sway `snow_sway` (0–3),
Flake Color `snow_color`. **Particles tab** (15–21): Max Drops `max_drops` (0–20000), Area X/Y/Z
`area` (1–500 / 200 / 500), Rain Speed `rain_speed` (1–60), Streak `streak_texels` (1–12).
**Rain tab** (21–28): Rain Style `rain_style` (Flipbook/Particles), Rain Layers `rain_layers`
(1–32), Layer Gap `rain_layer_spacing`, Rain Texel `rain_texel`, Splashes `max_splashes`
(0–8000), Splash/s `splash_rate`, Splash R. `splash_radius` (1–8). **Lightning tab** (28–30):
Lightning `lightning_enabled` (bool), Flash `flash_energy` (0–8). **Colors tab** (30+): Rain
`rain_color`, Bolt `bolt_color`.
Method: `strike_lightning()` fires a bolt on demand (used to capture flashes).
Gallery: `21-weather-clear/cloudy/foggy/rain/snow/storm.png`; clips `42-rain`, `43-snow`,
`44-storm` (flashes at ~5 s & ~11 s).

### 2d. PixelWater — "Water" — tabs **Surface · Waves · Light · Caustics · Foam · Colors · Waterfalls**
`50`–`56`. Section tooltip: *"Water — PixelWater: depth-banded color, foam and waves; raised pools
and waterfalls on hex / triangle terrains (Waterfalls page)."*

- **Surface** (0–6): Size X/Z `size` (1–4000 / 300), Subdiv. `subdivisions` (0–256 / 128),
  Depth `depth_scale` (0.1–32 / 4), Depth Bands `depth_bands` (1–8 / 4), Rim Dither `rim_dither`.
- **Waves** (6–11): Swell H. `swell_height`, Swell Len. `swell_length`, Steepness
  `swell_steepness`, Swell Speed `swell_speed`, Chop H. `wave_height`. (Gerstner swell + chop.)
- **Light** (11–16): Grazing `grazing_strength`, Reflection `reflection_strength`, Refl. Bands
  `reflection_bands` (1–6), Turq. Depth `turquoise_depth`, Sparkle `sparkle` (0–2).
- **Caustics** (16–20): Caustics `caustics_strength`, Caust. Scale, Caust. Bands, Caust. Reach
  (the dancing light pattern on the lagoon floor under the water).
- **Foam** (20–26): Foam Width, Foam Speed, Surge/s `foam_pulse_speed`, Surge Pause
  `foam_dead_time`, Surge `foam_surge`, Bands.
- **Colors** (26–33): Shallow, Deep, Turquoise, Grazing, Sun Side `sky_toward_sun`, Away Side
  `sky_away_from_sun`, Foam (colour buttons).
- **Waterfalls** (33+): Fall Levels `waterfall_min_levels` (1–64), Level H.
  `waterfall_level_height`, Fall Speed, Streaks/m, Streak Len., Base Foam, Spray Rate, Spray Rise,
  Fall Water / Fall Streak / Fall Foam colours. **Note:** raised pools + waterfalls only form on
  **hex / triangle** terrains (the tutorial island is a square-grid terrain, so no waterfall spills
  here; the fields still show). See `demo/triangle_island.tscn` / `demo/hex_island.tscn`.
Clip `41-water-waves.mp4` (swell, foam surge, sparkle around the island shore).

### 2e. PixelFireflies — "Fireflies" (one strip) — `80-fireflies-strip.png`
Section tooltip: *"Fireflies — PixelFireflies: a blinking, wandering swarm that fades in at night."*
Count `count` (0–4096 / 100), Area X/Y/Z `extents`, Seed, Color `tint`, Size `size` (0.01–2),
Wander `wander` (0–4), Blink `blink_speed` (0–10), Glow `energy` (0–8 / 2), Night Only
`night_only` (0–1 / 1 — fades the swarm in as night falls; invisible at noon, as expected).
Clip `46-fireflies.mp4` (time 0.9, glowing specks blink and wander over the island).

### 2f. PixelParticleField — "Particle Field" — tabs **Field · Layers · Flipbook · Look**
`60`–`63`. Section tooltip: *"Particle Field — PixelParticleField: a prerendered, tiling and
looping flipbook of pixel particles on world-anchored layers (dust motes in the sunbeams, rain,
snow, embers, leaves); stays put under ortho and perspective camera moves."*
- **Field** (0–4): Preset `preset` (Dust Motes / Rain / Snow / Embers / Leaves), Emitting
  `emitting` (bool), Density `density` (0–1), Weather `weather_response` (Always / Fair Weather /
  Rain / Snow — ties the field to the weather).
- **Layers** (4–14): Projection (Ground/Upright), Layers (1–64), Spacing, Texel, Extent, Height
  `stature`, Band, Fall `fall_speed`, Wind X/Y. (The world-anchored sheet stack.)
- **Flipbook** (14–26): Tile, Frames, Count `particle_count`, Lifetime, Velocity X/Y, Drift, Sway,
  Fade, Streak, Seed, FPS. (The baked particle animation.)
- **Look** (26+): Color `tint`, Color B `color_b`, Brightness, Sunlit, Beam, Shade, Night.
Clip `51-particle-dust.mp4` (pale motes drift over the island). Dust is subtle in the editor.

### 2g. PixelBirds — "Birds" — tabs **Flock · Steering · Landing · Look**
`70`–`73`. Section tooltip: *"Birds — PixelBirds: a boids flock of 2-frame flapping pixel birds
that circle, land on high ground and scatter when the camera comes near."*
- **Flock** (0–4): Count (1–256 / 7), Radius (1–200), Altitude (0–100), Speed (0.1–30).
- **Steering** (4–11): Neighbours, Separation, Alignment, Cohesion, Steer, Flaps/s, Size.
- **Landing** (11–15): Land Every, Perch, Scare Dist. (birds scatter when the camera comes within
  this distance), Landing (bool).
- **Look** (15+): Outline (bool), In Editor `preview_in_editor` (bool — must be on to draw in the
  editor), Color `tint`.
**Editor preview (CORRECTED 2026-11-23):** birds **do** render in the editor with
`preview_in_editor` on — they draw as **small white birds at minimum size** (the texel size
floors to the smallest step because the editor has no active game camera to scale them up, but
they are clearly visible, not invisible). With **Animate ON** the flock flies/circles the island
and is easy to watch; with **Animate OFF** the birds are drawn but **frozen** (they don't fly).
So the "effectively invisible in the editor" claim was wrong — a birds clip ships (`45-birds.mp4`,
Animate on, zoomed in on the island so the flapping white flock reads against the grass). They are
still a touch larger / fully scaled at runtime (press Play), but the editor preview is fine.

### 2h. PixelSmoke — "Smoke & Fire" — tabs **Emission · Fire Light · Options · Colors**
`100`–`103`. Section tooltip: *"Smoke & Fire — PixelSmoke: tumbling outlined puffs drifting with
the sky wind, optional flames with a flickering light; burst(position, count) for build/destroy
puffs."*
- **Emission** (0–8): Rate (0–30), Lifetime (0.2–30), Rise `rise_speed`, Spread, Start Size,
  End Size, Wind `wind_strength`, Max Puffs (1–1024).
- **Fire Light** (8–12): Detail (4–16), Light `light_energy` (0–16), Light Range, Flicker (0–1).
- **Options** (12–15): Emitting (bool), Sky Wind `use_sky_wind` (bool — drift with the PixelSky
  wind), Outline (bool), **Fire** `fire` (bool — add flames at the base + a flickering light).
- **Colors** (15+): Smoke `fumes_tint`, Soot `ash_tint`, Flame `fire_tint`.
The node spawns at the editor camera's focus point; move it (Inspector Transform → Position) onto
whatever should smoke. Clip `47-smoke-fire.mp4` (plume rises from the island, drifts with wind).

### 2i. PixelLightVolume — "Light Volumes" (one strip) — `120-lightvolume-strip.png`
Section tooltip: *"Light Volumes — PixelLightVolume: stepped point light with a dithered light
pool and a small source orb. Select one to edit it; the first in the scene is used otherwise."*
Radius (0.1–100 / 3), Color `lamp_tint` (warm, default ~`[1.0, 0.72, 0.40]`), Energy (0–16),
Flicker (0–1), Shadows (bool), Glow `glow_enabled` (bool), Pool Scale, Pool Energy, Glow Radius,
Glow Energy, Glow Steps (1–8), **Night Boost** (0–1 / 1). **Editor preview (CORRECTED
2026-11-23):** the lamp orb **and** its dithered light pool **do** render in the editor — it is
**not broken**. The pool only glows at **~35 % strength outside night** (that is what `Night
Boost` does — it scales the pool down in daylight) and ramps to **full at night**, so to see a
clear lantern pool set the Day/Night **Time** to night (≈0.9–0.95) and select the volume. The
default **Energy is low (~0.15)**, so the pool is subtle at defaults; raise Energy / Pool Energy
(the `121-lightvolume-night.png` still and `52-lightvolume-night.mp4` flicker clip used Energy ≈
20, Pool Energy ≈ 2.5, Radius ≈ 14 on a night island, with fireflies adding to the mood). It is
still brighter at runtime, but the editor preview reads fine at night.

### 2j. PixelSunShafts — "Sun Shafts" (one strip) — `130-sunshafts-strip.png`
Section tooltip: *"Sun Shafts — PixelSunShafts: a stack of light sheets lit along the sun, for
visible shafts of light through gaps in the scene. Select one to edit it; the first in the scene
is used otherwise."*
Sheets `count` (1–16 / 12), Depth (0–200 / 18), Size (1–1000 / 200), Follow Cam `follow_camera`
(bool, on), Tint `beam_tint` (yellow), **Alpha / strength** (0–1 / **0.0105**), Start Depth
(0–100 / 14), Shaft Steps (0–8 / 3), Dither `dither_width` (0–1), Edge Fade (0–1), Fade Height
(0–100), Surface Fade (0–10).
(Distinct from the renderer's **God Rays** on page 07; Sun Shafts is a stand-alone sheet stack.)
**Editor preview (CORRECTED 2026-11-23):** Sun Shafts **do** render in the editor — not broken.
Two things keep them subtle at defaults: the **default Alpha is only `0.0105`** (barely visible),
and the shafts are **lit along the sun**, so they read best with a **low sun** (set Day/Night
**Time** to dawn ≈0.25 or dusk ≈0.75) and with the sun roughly toward the camera. Raise **Alpha**
to make the beam read — the `131-sunshafts-raised.png` still used **Alpha = 1.0** at dusk
(`time_of_day` 0.78), where a clear bright shaft streams from the sun across the water; ≈0.2–0.3
already gives a visible shaft. They are still stronger at runtime, but the editor preview is fine.

### 2k. PixelFireSystem — "Grass Fire" — tabs **Fire · Spread · Wind & Rain · Regrowth · Look**
`110`–`114`. Section tooltip: *"Grass Fire — PixelFireSystem: fire spreads through the parent
terrain's grass with the wind, chars the ground and shrinks the blades; rain puts it out with
steam, and burnt grass regrows. Enable Ignite Tool and click the terrain to start a fire in the
editor (Shift + click douses)."* Adds **under the terrain**.
- **Fire** (0–9): **Ignite Tool** `ignite_tool` (bool — turn on, then click the terrain to start a
  fire; **Shift+click** puts it out — this holds back the move gizmo while on, `fire_tool.rs`),
  Ignite Radius (0.1–20 / 0.8), **Ignite Here** (button — start a fire at a default spot), **Put
  Out** (button `extinguish_all`), **Reset** (button `reset_fire`), Simulate (bool, on), In Editor
  `preview_in_editor` (bool, on), Cell Size `texel_size`, Seed.
- **Spread** (9–14): Spread `spread_rate` (0–10), Burn Time (0.1–60), Embers `ember_rate`,
  Ember Dist., Max Step.
- **Wind & Rain** (14–21): Sky Wind (bool), Wind Str., Wind Skew `wind_influence`, Rain
  `rain_override` (−1–1), Douse `douse_rate`, Wet Rate, Wet Block.
- **Regrowth** (21–24): Regrow After `regrow_delay`, Regrow Time, Wet Boost `wet_regrow_boost`.
- **Look** (24+): Flames `flame_rate` (0–40), Flame Size, Smoke `smoke_rate`, Steam, Lights
  (0–8), Light `light_energy`, Glow `glow_intensity`, Core `inner_tint`, Flame `fire_tint`, Rim
  `brim_tint`, Smoke Col. `fumes_tint`, Steam Col. `vapor_tint`.
Clip `48-grass-fire.mp4` (fire spreads across the island grass, charring the ground).

---

## 3. Lighting Presets (right panel) — `140-lighting-presets-panel.png`, gallery `141-preset-*.png`

Below the effect rows the panel has a **Lighting Preset** header and six buttons — **Dawn · Noon
· Dusk · Night · Storm · Snow** (`pixel_ui/presets.rs`). One click sets a whole bundle of values
on Day/Night + Sky + Weather + the renderer's god rays at once (one undo step). Exact values
(from `presets.rs`), useful as recipe starting points:

| Preset | time_of_day | weather | clouds coverage | god rays | mood |
|---|---|---|---|---|---|
| Dawn | 0.27 | Clear | 0.35, deck peach | on, warm, alpha 0.5 | soft pink-gold morning, light fog |
| Noon | 0.50 | Clear | 0.42, no deck | on, bright, alpha 1.0, day cycle | bright midday |
| Dusk | 0.73 | Clear | 0.45, deck orange | on, orange, alpha 0.5 | warm golden-orange evening |
| Night | 0.95 | Clear | 0.30, deck blue | off | cool blue night |
| Storm | 0.45 | **Storm** | 0.85, deck grey, fog | off | dark overcast downpour |
| Snow | 0.42 | **Snow** | 0.62, deck white, fog | off | pale cold whiteout |

Clip `50-lighting-presets.mp4` (Dawn→Noon→Dusk→Night, the world recolours each click).
Also on the panel: **Wire Sun / Env Refs** (fills empty sun/env refs of Sky/Day-Night/Weather/Sun
Shafts) — page 07 owns it.

---

## 4. Walkthroughs (performed in the editor)

### A. Read the atmosphere workspace
1. Open `04_world_mood.tscn`. In the Scene tree click **PixelDayNight** → the left pixel toolbar,
   the Day/Night bottom strip, and the right **Pixel World** panel appear
   (`01-atmosphere-workspace.png`).
2. The strip has tabs **Time / Sun & Fog / Rim Light**; click them (`10/11/12`).

### B. Change the time of day
1. Day/Night → **Time** tab → drag **Time** 0 → 0.25 → 0.5 → 0.75 (or type). The sky, sun angle
   and shadows change from night → dawn → noon → dusk (series `13-tod-*`). *(Animate OFF is fine —
   the picture updates on each change; Animate ON makes the clock run on its own, clip `40`.)*

### C. Pick a weather type
1. Select **PixelWeather** → **Weather** tab → **Weather** dropdown → try each of Clear/Cloudy/
   Foggy/Rain/Snow/Storm (gallery `21-weather-*`). Set **Transition** to 0 for an instant switch.
2. Turn **Animate** ON to see rain fall (`42`), snow fall + settle (`43`), and a storm with
   lightning (`44`). On the **Snow** tab, `Snow` adds extra falling flakes on top of any weather.

### D. Tune the water
1. Select **PixelWater** → tabs Surface/Waves/Light/Caustics/Foam/Colors/Waterfalls (`50`–`56`).
   Animate ON → the lagoon swells, foam surges round the island shore (`41`).

### E. Night life: fireflies
1. Set Day/Night **Time** ≈ 0.9 (night). Select **Fireflies** (`80`). The glowing swarm fades in
   (`46`). At noon they vanish (Night Only = 1).

### F. Add smoke & fire
1. In the Pixel World panel click **Smoke & Fire → + Add** (panel scrolls — see Caveats). A
   **Smoke** node appears (`90-add-before/after.png`). Select it → Options tab → tick **Fire**.
   Move it (Inspector Position) onto the island. Animate ON → a plume rises and drifts (`47`).

### G. Set the grass alight (the fire tool)
1. Pixel World panel → **Grass Fire → + Add** (adds a **FireSystem** under the terrain).
2. Select it → **Fire** tab. Tick **Ignite Tool**, then **click the island grass** in the 3D view
   to start a fire (Shift+click douses); or press **Ignite Here**. Animate ON → it spreads and
   chars the grass (`48`). **Put Out** / **Reset** stop it.

### H. Apply a lighting preset + undo
1. Pixel World panel → scroll to **Lighting Preset** → click **Dusk**: the whole scene turns warm
   orange (`150-undo-before` → `150-undo-dusk`). **Ctrl+Z** reverts it in one step
   (`150-undo-after`; time 0.42 restored, preview reverts — small sticky residue, see Caveats).

---

## 5. Media manifest

| File | Kind | Shows |
|---|---|---|
| 01-hero.png | shot | The lagoon island (dusk) — neutral page header (preset `141-*` shots are more striking). |
| 01-atmosphere-workspace.png | shot (full) | Whole atmosphere workspace: scene tree, pixel toolbar, Day/Night strip, Pixel World panel, Inspector. |
| 10-daynight-time.png / 11-sunfog / 12-rimlight | shots | Day/Night strip tabs. |
| 13-tod-000/025/050/075.png | series "Time" | Time of day 0→0.75 (night→dawn→noon→dusk), same camera. |
| 20-weather-strip.png | shot | Weather strip (Weather tab). |
| 21-weather-clear/cloudy/foggy/rain/snow/storm.png | gallery | Each of the 6 weather types, same camera. |
| 22-snowtab / 23-particles / 24-raintab / 25-lightning / 26-colors.png | shots | The other Weather tabs. |
| 30-sky-strip.png | shot | Sky & Clouds strip (all fields). |
| 40-daynight-cycle.mp4 (+poster) | clip | Day/Night clock running + cloud shadows drifting. |
| 41-water-waves.mp4 (+poster) | clip | Water swell, foam surge, sparkle. |
| 42-rain.mp4 (+poster) | clip | Rain falling + splashes on water/island. |
| 43-snow.mp4 (+poster) | clip | Snowfall whiting out the lagoon. |
| 44-storm.mp4 (+poster) | clip | Storm downpour with lightning flashes (re-shot: dark/foggy, bolts fired via `strike_lightning()`). |
| 45-birds.mp4 (+poster) | clip **(NEW)** | The white boids flock flying/circling the island (Animate on, zoomed in so the birds read). |
| 46-fireflies.mp4 (+poster) | clip | Fireflies blinking/wandering at night. |
| 47-smoke-fire.mp4 (+poster) | clip | Smoke plume rising + drifting from the island. |
| 48-grass-fire.mp4 (+poster) | clip | Grass fire spreading and charring the island. |
| 49-clouds-drift.mp4 (+poster) | clip | Cloud shadows + deck drifting over the lagoon. |
| 50-lighting-presets.mp4 (+poster) | clip | Dawn→Noon→Dusk→Night presets recolour the world. |
| 51-particle-dust.mp4 (+poster) | clip | Dust motes drifting over the island. |
| 50-water-surface … 56-water-waterfalls.png | shots | The 7 Water tabs. |
| 60-particle-field … 63-particle-look.png | shots | The 4 Particle Field tabs. |
| 70-birds-flock … 73-birds-look.png | shots | The 4 Birds tabs. |
| 80-fireflies-strip.png | shot | Fireflies strip. |
| 90-add-before.png / 90-add-after.png | before/after | Pixel World panel "+ Add" → the row flips to "Select" (Smoke & Fire added). |
| 100-smoke-emission … 103-smoke-colors.png | shots | The 4 Smoke & Fire tabs. |
| 110-fire-fire … 114-fire-look.png | shots | The 5 Grass Fire tabs. |
| 120-lightvolume-strip.png | shot | Light Volumes strip. |
| 121-lightvolume-night.png | shot **(NEW)** | A Light Volume glowing at night: lamp orb + warm dithered light pool on the island (Energy ≈20, night, fireflies around). |
| 52-lightvolume-night.mp4 (+poster) | clip **(NEW)** | The lantern pool flickering at night (Flicker ≈0.6, Animate on) with blinking fireflies. |
| 130-sunshafts-strip.png | shot | Sun Shafts strip. |
| 131-sunshafts-raised.png | shot **(NEW)** | A sun shaft reading clearly at dusk with **Alpha raised to 1.0** (default 0.011) — a bright beam streams from the low sun across the water. |
| 140-lighting-presets-panel.png | shot | The Lighting Preset buttons in the panel. |
| 141-preset-dawn/noon/dusk/night/storm/snow.png | gallery | Each lighting preset applied (great mood heroes). |
| 150-undo-before / -dusk / -after.png | sequence | Apply Dusk preset, then Ctrl+Z reverts it (one step). |

**Scrubbers / compares for the writer:** Time-of-day scrubber = `13-tod-000/025/050/075`. Weather
gallery = `21-weather-*`. Preset gallery = `141-preset-*`. Add before/after = `90-add-before/after`.
Undo sequence = `150-undo-*`.

**Re-taken / new in the 2026-11-23 fix session (`docs-atmos2`):**
- **Re-taken (overwritten, fresh state, consistent close island framing):** the six preset shots
  `141-preset-{dawn,noon,dusk,night,storm,snow}.png`; the lighting-preset clip
  `50-lighting-presets.mp4` (+ poster); the weather gallery
  `21-weather-{clear,cloudy,foggy,rain,snow,storm}.png`; weather clips `43-snow.mp4` and
  `44-storm.mp4` (+ posters); the time-of-day scrubber `13-tod-{000,025,050,075}.png` (the old
  ones were washed/pale, cloudy, far top-down); the undo sequence
  `150-undo-{before,dusk,after}.png` (the old before/after showed the mint-green fog bug; the new
  `after` reverts cleanly to match `before`).
- **New:** `45-birds.mp4` (+poster), `121-lightvolume-night.png`, `52-lightvolume-night.mp4`
  (+poster), `131-sunshafts-raised.png`.
- **Left as-is (checked, not bugged):** all UI-strip crops; the clips `40-daynight-cycle`,
  `41-water-waves`, `42-rain`, `46-fireflies`, `47-smoke-fire`, `48-grass-fire`, `49-clouds-drift`,
  `51-particle-dust` (posters + mid-frames clean); `01-hero.png`, `01-atmosphere-workspace.png`.
  (`40-daynight-cycle` still uses the old far top-down, cloudy framing but shows the cycle running,
  not the fog bug, so it was kept.)
- **Framing note:** the re-takes use a single locked camera on the island
  (`camera_position ≈ [110,116,107]` for the gallery; slightly closer/zoomed variants for the
  birds / light-volume / sun-shafts shots), Select-tool with the translate gizmo turned off
  (3D-view **View ▸ View Transform Gizmo**) and the game **Camera** node hidden, so only a small
  sun/moon node icon remains. Panel-present viewport crop is `(339,165)-(1058,515)` = 719×350.

---

## 6. Roblox bridges
- **Day / Night + Lighting presets** ≈ Studio's **Lighting** service (ClockTime, Atmosphere, Sky)
  and the Lighting presets — here one **Dawn/Noon/Dusk/Night** click sets time, sun, clouds, fog
  and god-rays together, undoable.
- **Weather** has no Studio built-in — it's like dropping in one effect that does rain, snow, fog,
  wet ground and lightning as a single dial (the `Weather` dropdown), instead of hand-building a
  ParticleEmitter + Atmosphere combo.
- **Water** ≈ Studio **Terrain Water**, but it's a stylised pixel surface node with its own depth
  bands, foam and caustics rather than voxel water.
- **"+ Add" in the Pixel World panel** ≈ **Insert Object**, but each entry is a whole effect and it
  auto-wires the sun/camera for you (Studio has no auto-wiring).
- **Fireflies / Birds / Smoke / Particle Field** ≈ Roblox **ParticleEmitter / Beam** decorations,
  but preset and pixel-art styled; you tune counts and colours, not emitter rates.
- **Grass Fire ignite tool** ≈ no Studio equivalent — a click-in-the-viewport paint tool that
  starts a spreading simulation on the terrain's grass.
- **Undo** is the same **Ctrl+Z** — every strip/panel/preset change is one history step.

---

## 7. Caveats (verified)

- **Animate vs edits (the big one — see §0).** Editing any Sky/Day-Night/Weather/etc. field
  updates the editor picture **immediately, even with Animate OFF** (verified by set-prop AND a
  real slider drag). **Animate ON is only for watching motion** (clock running, clouds drifting,
  rain/snow falling, birds flying, fire flickering). A fresh editor session starts with the clock
  frozen (Animate off). The page-07 "preview only refreshes with Animate on" suspicion did **not**
  reproduce as a defect and is **not** blocking. Tell the reader: turn Animate on to *watch*; a
  field edit shows up right away either way.
- **Lighting-preset fog stickiness — FIXED (2026-11-23).** The old behaviour (cycling presets,
  especially Storm/Snow, left **fog switched on** and washed the whole lagoon flat mint-green,
  brightness ~190 vs ~115 normal, even after re-applying Noon) **no longer reproduces** after the
  addon fix: presets now turn fog off when they should, so every preset reads correctly even after
  cycling through all six, and a preset + Ctrl+Z reverts cleanly (verified: undo-after ≈
  undo-before, no washed residue). **Storm** now renders as a proper dark, foggy downpour (its
  heavy fog hides a distant island, so the preset gallery was shot from a closer camera where the
  storm reads as a dark foggy silhouette). All preset / weather / tod / undo media was re-shot in
  this fixed state.
- **Weather now applies instantly with Animate OFF — FIXED (2026-11-23).** Changing `weather`
  (dropdown or property) used to freeze half-blended while Animate was off; it now snaps fully to
  the new weather in one frame even with a non-zero `transition_seconds`. (Turn Animate ON only to
  watch rain/snow fall, lightning flash, etc.) The weather gallery `21-weather-*` was captured
  Animate-off and each type is fully applied, not half-blended.
- **Birds, Light Volume, Sun Shafts preview FINE in the editor (CORRECTED 2026-11-23).** The
  earlier "they preview poorly / birds invisible" claim was wrong — all three render in the editor:
  - **Birds** draw as **small white birds at minimum size** (visible, just not scaled up as at
    runtime); they **fly with Animate ON**, freeze with Animate off. Clip `45-birds.mp4` ships.
  - **Light Volume** shows the lamp orb **and** a dithered light pool; the pool glows at **~35 %
    outside night** (`Night Boost`) and **full at night**, and default **Energy is low (~0.15)**,
    so set Time to night and raise Energy to read it. Still `121-lightvolume-night.png` + flicker
    clip `52-lightvolume-night.mp4` ship.
  - **Sun Shafts** render but **default Alpha is only ~0.011** and they are **lit along the sun**
    — raise Alpha and use a low sun (dawn/dusk) to make the beam read. Still
    `131-sunshafts-raised.png` (Alpha 1.0, dusk) ships.
  All three are still stronger at **runtime** (press Play), but the editor preview is usable.
- **The Pixel World panel scrolls.** Its lower rows (Pixel Object, Diagnostics, Erosion, Wild
  Flowers, Grass Fire) and the **Lighting Preset** buttons sit below the visible fold; scroll the
  panel (mouse-wheel over it) to reach and click them. (During capture, clicking a "+ Add" that
  was reported-visible-but-actually-scrolled-off did nothing until I scrolled it into view — this
  is a scroll/visibility quirk, **not** an addon bug: it affects terrain-child and non-terrain
  rows alike.)
- **Water waterfalls need hex / triangle terrains.** The Waterfalls tab/fields show on any water,
  but raised pools + spilling waterfalls only form on hex/triangle terrain, not the square-grid
  tutorial island. Use `demo/triangle_island.tscn` / `demo/hex_island.tscn` to show them.
- **Newly added effect nodes spawn at the editor camera's focus point**, often mid-air (e.g. Smoke
  / Light Volume landed at ~[100,115,183]). Move them (Inspector → Transform → Position) onto
  whatever should emit.
- **Editor is slow (~1–8 fps, llvmpipe).** Wait 1–2 s after clicks that rebuild the strip/panel;
  synthetic spin-slider **drags overshoot** (a Time drag jumped to 62.8; page-07 noted the same) —
  the knob **series** here were set with exact values, not drags. Dragging slowly by hand is fine.
- **The small node-icon gizmo** (a camera/creature sprite + magenta selection box) sits at the
  scene origin and shows in every shot where a point-like effect node is selected; it's harmless
  but a little distracting. Selecting the terrain instead swaps in the sculpt tool's yellow
  bounding box + a hint bar (also not clean). No gizmo-free hero was achievable without toggling
  View → Gizmos; the preset-gallery shots (`141-*`) are the cleanest mood heroes.
- **No new errors** in `editor.log` from any atmosphere edit, add, preset or the fire tool
  (only the known V-Sync / debug-adapter startup noise).

---

## 8. Scenes created
- **None.** All capture used the stock `res://tutorial/04_world_mood.tscn`. The four nodes added
  during capture (Smoke, LightVolume, SunShafts, FireSystem) were **not saved** — they lived only
  in the disposable session copy and were discarded on session restart.

## Re-capture (review pass, 2026-11-24)
- **62-particle-flipbook** re-shot: the strip label was clipped at the top of the old crop. New
  crop starts at y481 (just below the strip's blue focus border) and is 150 px tall so the whole
  tab row, Tile 128 and the Fade/Streak/Seed/FPS row are visible.
- **47-smoke-fire** re-shot: the old plume was a thin wisp. The new clip raises the smoke
  (rate 28, lifetime 10, rise 5, start 1.0, end 9, spread 0.6, wind 1.5, max puffs 256) and
  records the same close island framing with the plume fully in frame; fire stays on.
- **44-storm** poster re-taken at 5.6 s, the brightest lightning flash, so the still shows a
  visible bolt instead of a plain dark frame. Clip unchanged.
- **40-daynight-cycle** caption corrected in the HTML: the clip is the close island view, not
  the old far top-down.
- **56-water-waterfalls**, **22-weather-snowtab**, **61-particle-layers**, **114-fire-look**
  re-shots are good: full tab row included, and the waterfalls values are real on `hex_island`
  (Fall Levels 5, Fall Speed 3.18, …). Captions updated in the HTML.
- **121-lightvolume-night / 52-lightvolume-night** newly captured: close crop of a warm dithered
  pool at Time 0.95, Energy 20, Pool Energy 2.5, Pool Scale 1.5, Radius 14, Flicker 0.6.

### Caveat: Pixel World "+ Add" gives an unwired Light Volume
`Pixel World → Light Volume → + Add` instantiates a bare `PixelLightVolume`: it has **no**
`Light` (OmniLight3D) and **no** `Volume` (MeshInstance3D/glow) children, so the node renders
nothing even at Energy 120 / Pool Energy 10. The shipped rig is
`res://addons/MarchingSquaresTerrain/effects/pixel_light_volume.tscn` (root `PixelLightVolume`
with `light = NodePath("Light")`, `glow_mesh = NodePath("Volume")`). Instancing that scene (or
copying its root and pasting it under the scene root) makes the pool render. Recipe steps for
121/52 do exactly that. Reported as BLOCKED (addon defect) in the capture report.

### Caveat: Particle Field dust is effectively invisible in the editor preview
The Dust node's flipbook draws only a faint, sub-pixel haze with the pixel renderer; at default
framing it reads as a small white puff/blob (in the old `51-particle-dust` poster the visible
white blob is the node's editor icon, not motes). `blackbox` A/B captures show a small image
difference, so the effect is running, but nothing a reader would recognise as dust motes. The
`51-particle-dust` clip is left as-is and its caption says it is subtle. Reported as BLOCKED
(dust preview) in the capture report.

## Recipes
`docs/guide/recipes/08-atmosphere-effects.json` now covers the page: weather gallery + tabs, the
water tabs, particle/birds/smoke/fire tabs, the preset gallery, time-of-day and undo sequences,
the clips, and the Light Volume still+clip. Recipe tests stage to `--out-dir` only, on
`:113`/`8913`.

### Recipe backfill (2026-11-24)
Every media file on the page is now listed by an item in
`docs/guide/recipes/08-atmosphere-effects.json` (32 items, grouped by section: hero/workspace,
the Day/Night and Weather tabs + gallery, TOD series, Water tabs, Particle/Birds tabs, Fireflies,
the + Add pair, Smoke/Fire tabs, Light Volume strip, Sun Shafts strip + raised shaft, the preset
gallery, the undo sequence, and every clip). Tested on `:113`/`8913` — 29/30 of the first pass
ran clean; the only step failure was the Sun Shafts + Add row position (fixed by scrolling the
panel 6 steps and clicking 1227,410; re-ran clean).
Strip crops use the measured section-label y per tab in the 1600x900 layout. The 121/52 Light
Volume item must stay one item: the shipped rig is copied from
`res://addons/MarchingSquaresTerrain/effects/pixel_light_volume.tscn`, and the editor clipboard
does not survive a session restart, so a fresh_session per output loses the paste.

## Re-capture (2026-11-24, display :108 / port 8908)

- **`51-particle-dust.mp4/.png` re-shot** — the fixed particle field now reads with the honest
  settings: `Dust` Field ▸ **Density = 1.0**, Look ▸ **Beam = 0** (Beam 1 only shows motes inside the
  god-ray shafts). Birds and Fireflies hidden so their specks are not mistaken for dust. Filmed on
  `demo/hex_island.tscn` because that scene opens already framed close on the ruin and pond — at
  island distance a dust mote is a single pixel and invisible. Clip 1026×458, 11 s, poster at 7 s:
  pale one-pixel motes drift across the grass and ruin. The default is faint by design (Density 0.2,
  1-px motes).
- **HTML:** the Particle Field section now has a "Dust looks like nothing" tip naming the two settings
  (Beam 0 + Density 1.0) and the second `51` figure's caption matches the close-up. The Misty Dawn
  recipe step now says Beam 0 for motes everywhere instead of "raise Beam".
- **Recipe:** `51-particle-dust` in `docs/guide/recipes/08-atmosphere-effects.json` is green on
  `:118/8918` (fresh session, item scene `demo/hex_island.tscn`, set density/beam, hide
  Birds/Fireflies, record the viewport, poster at 7).
