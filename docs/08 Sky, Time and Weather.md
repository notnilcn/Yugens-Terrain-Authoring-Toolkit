# Sky, Time and Weather

The island demo gets much of its mood from three nodes: **PixelSky** for clouds, **PixelDayNight** for the sun and sky colour, and **PixelWeather** for rain, snow and lightning.

![[day-night-weather.mp4]]

> [!warning]
> Most effects are behind **feature switches**. If a node is in the scene but nothing happens, open **Project Settings → marching_squares_terrain/features** and turn its feature on. Adding a node from the **Pixel World → Add** menu turns the feature on for you; adding it by hand does not.

## PixelSky — clouds and their shadows

**PixelSky** draws the cloud shadows that drift across the ground, and optionally a visible cloud deck in the sky.

Select `PixelSky` and look at the Inspector.

![[sky-inspector.png]]

| Setting | What it does | Good range |
|---|---|---|
| **Sun** | The `DirectionalLight3D` used to project cloud shadows. Empty = the first sun sibling. | Wire it explicitly |
| **Clouds** | Master switch for this node. | On |
| **Coverage** | How much of the sky is covered. | `0.3–0.7` |
| **Softness** | How soft the cloud edges are. | `0.1` |
| **Scale** | Size of the cloud shapes. Smaller = bigger clouds. | `0.015–0.04` |
| **Wind** | How fast the clouds drift (also affects smoke and fire). | `(0.02, 0.01)` |
| **Height** | World height of the cloud plane used for shadows. | `40`, or above your camera |
| **Shadow** | How dark cloud shadows are. | `0.3–0.6` |
| **Bands** | Steps the cloud shading into pixel bands. `0` = smooth. | `2–4` for a pixel look |
| **Cloud Deck → Deck** | Draws a visible cloud layer. | On |
| **Deck Size** | Size of the visible cloud plane. | 2–3× your map |
| **Deck Opacity** | How solid the clouds look. | `0.6–0.9` |
| **Deck Color / Shade Color** | Lit and shaded cloud colours. | Warm white / cool blue-grey |

> [!tip]
> The cloud deck follows the camera, so you can make it larger than the playable area without wasting geometry.
>
> [!warning]
> If your camera is **above** the cloud height, the deck can pass between the camera and the ground and wash the terrain out with cloud colour. The tutorial overview cameras are high, so their scenes raise **Height** above the camera. Either raise the height or turn **Cloud Deck → Deck** off.

## PixelDayNight — sun, sky and time

**PixelDayNight** moves the sun along a daily arc and grades the sky, ambient light, fog and rim light to match the time.

Select `PixelDayNight`.

![[daynight-inspector.png]]

| Setting | What it does |
|---|---|
| **Time** | `0` = midnight, `0.25` = sunrise, `0.5` = noon, `0.75` = sunset. |
| **Day Length** | Real seconds for a full day. `600` = 10 minutes. `1080` = 18 minutes. |
| **Running** | Whether time advances in the game. |
| **Time Scale** | Multiplier. `10` makes the day go by ten times faster. |
| **Override** | Any value `>= 0` freezes the time at that value. Great for screenshots and cutscenes. |
| **Sun** | The `DirectionalLight3D` to move. Empty = first sibling. |
| **Environment** | The `WorldEnvironment` to grade. Empty = first sibling. |
| **Latitude Tilt / Azimuth** | Tweaks the angle of the sun's arc. |
| **Sun Shadows** | Shadows fade out at night. |
| **Sun Energy Scale** | Overall sun brightness. |
| **Ambient Energy Scale** | How bright the shadowed side is. |
| **Apply Fog** | Drives depth fog on the environment. |
| **Fog Density / Night Density** | Day and night fog thickness. |
| **Light Ramp Preset** | Toon lighting bands for terrain and objects: Off, Classic, Soft, Hard, Warm Shadows, Custom. |
| **Rim Strength / Width** | A light edge on silhouettes, which helps characters read against the ground. |

> [!warning]
> **In the editor, time does not advance by itself.** Drag **Time** to see a different time. If you want a frozen time, set **Override** to that value. In the running game, the cycle moves normally.

> [!tip]
> **Pixel World → Lighting Preset** sets up Dawn, Noon, Dusk, Night, Storm or Snow in one click. Start from a preset, then tweak.

The WorldEnvironment must use a **Color** or **ProceduralSky** background for PixelDayNight to grade the sky. Other sky types are left alone.

## PixelWeather — rain, snow and lightning

**PixelWeather** blends between weather types and darkens the sky through the DayNight node.

Select `PixelWeather`, and place the node roughly at ground height near the middle of the map.

![[weather-inspector.png]]

| Setting | What it does |
|---|---|
| **Weather** | Clear, Cloudy, Foggy, Rain, Snow, Storm. |
| **Transition** | How long the change takes, in seconds. |
| **Ground** | How long the ground stays wet or snowy, in seconds. |
| **Strength** | Overall weather intensity. |
| **Presets** | Optional `PixelWeatherPreset` resources for custom weather. |
| **Sun / Environment** | Usually wired by the Pixel World menu. |
| **Rain Style** | `Flipbook` (cheap sheets plus splashes) or `Particles`. |
| **Area** | Size of the rain box around the camera. |
| **Rain Speed / Streak Texels** | How fast and how long the streaks are. |
| **Layers / Layer Spacing / Texel** | Density and size of the flipbook rain. |
| **Splashes / Splash Rate** | Splash rings on the ground and water. |
| **Snow Enabled** | Snow on top of any weather. |
| **Snow Intensity / Flakes / Flake Size** | Snowfall amount and look. |
| **Accum. / Melt** | How fast snow builds up and melts. |
| **Lightning Enabled** | Storm flashes and bolts. |
| **Bolt Color / Flash Energy** | Lightning look. |

### Weather recipes

**Rain**

1. Add `PixelWeather` (use **Pixel World → Add → Weather**).
2. Set **Weather** to `Rain`.
3. Keep **Rain Style** on `Flipbook`.
4. Adjust **Rain Texel** (`0.06–0.10`) and **Layers** (`6–10`) until the rain reads at your camera distance.
5. Set **Splash/s** to `1–3`.

**Snow**

1. Set **Weather** to `Snow`, or leave the weather clear and tick **Snow Enabled**.
2. Set **Intensity** to `0.5–0.8`.
3. Set **Flakes** to `300–800` and **Flake Size** to `2–3`.
4. Set **Accum./min** to `0.3–1.0` so the ground whitens over time.

**Storm**

1. Set **Weather** to `Storm`.
2. Tune **Transition Seconds** to `8–15` so the storm rolls in.
3. If the lightning is too much, lower **Flash Energy** or turn **Lightning Enabled** off.

> [!tip]
> The ground and the sky need to agree. A sunny noon sky with heavy rain looks wrong. PixelWeather darkens the sky through PixelDayNight automatically, but you can also lower **Time** slightly for a gloomier look.

> [!warning]
> Rain and fog are tuned for a camera 20–40 m from the action. If your camera is far away (a 150 m island overview), the default rain fog can ghost the whole island. The tutorial scenes use custom **Rain / Snow / Storm** presets with only a tenth of the fog. For a far camera, duplicate the built-in preset and lower **Fog Density**, or turn the environment fog down.

## Feature switches

In `Project Settings → marching_squares_terrain/features` you will find:

| Feature | Needed for |
|---|---|
| Clouds | Cloud shadows, cloud deck, god rays |
| God Rays | Renderer god rays, dust motes |
| Water | PixelWater |
| Weather | PixelWeather |
| Birds | PixelBirds |
| Fireflies | PixelFireflies |
| Smoke | PixelSmoke |
| Fire | PixelFireSystem |
| Particles | PixelParticleField |
| Light Volumes | PixelLightVolume and fire light |
| Sun Shafts | PixelSunShafts |
| Outlines / Outline Ids | The pixel outline pass |
| Grass | Terrain grass |

If an effect node is in the scene but invisible, check its switch first. This is the number-one cause of “the addon is broken”.

## The Animate button

In the editor, most effects are frozen until you press **Animate** at the top of the 3D view. That is deliberate: it lets the editor idle when you are not looking at motion.

- Clouds drift, water waves, grass sways, fire burns and birds fly only while **Animate** is on.
- In the running game, everything animates normally.
- **PixelDayNight** never advances in the editor even with Animate; scrub **Time**.

Next: [[09 Water, Fire and Atmosphere]]
