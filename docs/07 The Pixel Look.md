# The Pixel Look

This is the part that makes the game look like a pixel-art painting instead of a normal 3D game. The models are still 3D — the addon renders them in a smart, low-resolution way and adds outlines.

![[outline-comparison.mp4]]

## How it works (the short version)

A node called **PixelRenderer** watches the active camera. Every frame it:

1. Renders the 3D world into a small image, for example 426 × 240 pixels.
2. Draws dark outlines where objects overlap the sky or each other.
3. Optionally reduces the colors to a palette.
4. Scales the small image up to your screen without blurring it.

The UI is drawn after that, so text stays sharp.

## The PixelRenderer

Select the `PixelRenderer` node in the Scene tree. The Inspector shows these groups.

![[renderer-inspector.png]]

### The important settings

| Setting | What it does | Start here |
|---|---|---|
| **Enabled** | Master on/off. | On |
| **Mode** | `Anchor` is the full pixel pipeline with outlines and per-object block sizes. `Low Res` only scales the image; it has no outlines. | `Anchor` |
| **Pixel Size** | Screen pixels per low-res pixel. Higher = chunkier. | `3` or `4` for a 720p window |
| **Camera** | Leave empty to follow the active `Camera3D`. | Empty |
| **Preview** (Studio Prelook) | Shows the pixel look inside the editor. This is what the **Pixel Preview** button toggles. | On while working |

> [!tip]
> **Fixed Height** (in the Camera group) overrides **Pixel Size**. If changing Pixel Size does nothing, check that Fixed Height is `0`.

### Camera group

| Setting | What it does |
|---|---|
| **Fixed Height** | Locks the number of low-res rows. `0` = derive from Pixel Size. |
| **Texel Snap** | Stops pixels from shimmering when an orthographic camera moves. |
| **Sub-texel Scroll** | Keeps scrolling smooth while pixels stay locked to the world. |
| **Crisp Edges** | Avoids seams when the upscale scale is not a whole number. |
| **Stabilize Rotation** | Stops the image jumping when you orbit. |

Leave these on unless you are chasing a specific problem.

### Outlines group

| Setting | What it does | Good range |
|---|---|---|
| **Outlines** | Master switch for silhouettes and creases. | On |
| **Background** | Draws outlines where objects meet the sky. | On |
| **Depth Thr.** | How much depth difference counts as an edge. Lower = more outlines. | `0.3–0.5` |
| **Normal Thr.** | How sharp a ridge must be to get a crease highlight. Lower = more creases. | `0.5` |
| **Darken** | How dark the silhouette is when no outline color is set. | `0.5–0.65` |
| **Highlight** | How bright creases are. `0` turns creases off. | `0.2` |
| **Id Edges** | Draws edges where two different outline groups touch. | On |
| **Wide Ids** | Enables the full id pass (needed for per-object ids/colors). | On |
| **Obj. Pixel** | Fallback block size for objects that do not set their own. | `1` |

Outline colours are explained below.

### Color group

This is where the palette lives.

| Setting | What it does |
|---|---|
| **Palette** | A `PixelPalette` resource. |
| **Use Palette** | Turns palette quantization on. |
| **Mix** | Blends between the original colours and the palette. `1` = fully palette. |
| **Dither** | Adds an ordered dot pattern before the palette, which hides banding. |
| **Spread / Matrix** | Dither strength and pattern (`Bayer4` or `Bayer8`). |
| **Saturation / Contrast / Brightness** | Simple colour grading. |

> [!tip]
> A palette is the single biggest “style” decision. A good 32–64 colour palette makes the whole game feel designed. The demo's island palette is a set of value ramps: dark blues, teals, greens, browns, fire colours and skin tones.

### God Rays group

Screen-space light shafts that shine through gaps in the clouds. They need the **God Rays** and **Clouds** features turned on in `Project Settings → marching_squares_terrain/features`.

| Setting | What it does |
|---|---|
| **God Rays** | Master switch. |
| **Beam Tint** | Colour of the shafts. |
| **Ray Alpha** | Brightness of one sheet. |
| **Max Sheets / Ray Depth** | Quality and distance. Lower = cheaper. |
| **Dawn/Dusk** | Keeps long shafts at sunrise and sunset. |

If you do not need them, leave God Rays off. They cost performance.

### Advanced group

| Setting | Leave it alone unless… |
|---|---|
| **Meta Layer** | You have a custom render-layer setup. |
| **Depth Range** | Your scene is much larger than 200 m. |
| **View** | You want a debug view: Normals, Depth, Pixel Size, Outline Ids, Raw Color, Outline Cause, Shafts. |

## The three buttons at the top of the 3D view

| Button | What it does |
|---|---|
| **Pixel Preview** | Turns the editor's 3D view into the pixel look. This is the button you will use constantly. |
| **Animate** | Runs the effect clock in the editor: clouds, water, grass, fire. Turn it off on a slow laptop. |
| **Pixel World** | Add sky, water, weather and effects; apply lighting presets; wire the sun/environment references. |

![[pixel-preview-button.png]]

## Palettes

A **PixelPalette** is a resource with a list of colours. The renderer finds the nearest palette colour for every pixel.

To make one:

1. In the FileSystem, right-click → **Create New → Resource…** → `PixelPalette`.
2. Open it in the Inspector.
3. Fill **Tints** with colours, or assign a **Source Image**. A one-pixel-tall strip of swatches works well as a source image.
4. Assign the palette to the PixelRenderer's **Palette** slot and tick **Use Palette**.

Useful palette settings:

| Setting | Meaning |
|---|---|
| **Quantize Mode** | How “nearest colour” is measured. `Perceptual` is the best default. |
| **Weights** | Only used by `HueWeighted`. |
| **Brightness** | Only used by `Value`. |
| **Mix** | How strongly the palette takes over. |
| **Bake LUT to PNG** | Saves the palette lookup table as an image for reuse. |

> [!warning]
> If the palette is assigned but the scene still looks unchanged, **Use Palette** is probably off, or **Mix** is `0`.

## PixelObject

The terrain is already pixel-native. Other models are not. Wrap them in a **PixelObject** to give them the pixel shader, outlines and block sizes.

```
PixelObject
└── YourModel.glb
```

![[pixel-object-inspector.png]]

| Setting | What it does |
|---|---|
| **Pixel Size** | Block size in low-res texels. `-1` inherits. |
| **Outline** | `-1` inherit, `0` off, `1` on. |
| **Outline Id** | `-1` automatic, `0–253` explicit group. |
| **Own Color** + **Outline Col.** | Give this object its own outline colour. |
| **Convert** | Convert `StandardMaterial3D` to the pixel shader. Leave on. |
| **Bands / Shade Floor / Rim** | Toon shading. |
| **Texel Snap** | Stops small moving objects from shimmering. |
| **Profiles** | Per-part overrides for complex objects. |

> [!tip]
> **Outline Id** is how you get coloured outlines. Give the player id `1`, enemies id `2`, and interactive objects id `3`, then assign colours in a `PixelOutlineColors` resource. The default look is one dark outline, which is fine for a first pass.

## Orthographic vs perspective cameras

The crispest pixel look comes from an **orthographic** camera:

1. Select the `Camera3D`.
2. Set **Projection** to `Orthographic`.
3. Use **Size** to zoom. Smaller Size = closer.

An orthographic camera has no perspective distortion, so the pixel grid stays even. The trade-off is that it looks less “3D”. The demo uses orthographic cameras for the island overview and perspective cameras for the playable character.

If you use a perspective camera, pixel snapping only works on a focus plane. The demo sets a `pixel_snap_distance` metadata value on the player camera to tell the renderer where that plane is. Ask the programmer if you need it.

## Performance

The pixel look is cheap compared to a full-resolution 3D render, but a few settings matter:

- `Pixel Size` 4 is cheaper than 2 (fewer pixels).
- `Mode: Low Res` is much cheaper than `Anchor`, but has no outlines.
- Turn **God Rays** off if you do not need them.
- Turn **Animate** off in the editor when you are not looking at effects.
- `Wide Ids` adds one extra scene pass. Leave it on for coloured outlines, off for the cheapest look.

## Recommended first settings

| Setting | Value |
|---|---|
| Mode | `Anchor` |
| Pixel Size | `3` |
| Camera | empty |
| Preview | on |
| Outlines | on |
| Background | on |
| Depth Thr. | `0.35` |
| Normal Thr. | `0.5` |
| Darken | `0.55` |
| Highlight | `0.22` |
| Id Edges | on |
| Wide Ids | on |
| Obj. Pixel | `1` |
| Palette | the game palette, if there is one |
| Use Palette | on, if a palette is assigned |
| Dither | on |
| God Rays | off until the art needs them |

Next: [[08 Sky, Time and Weather]]
