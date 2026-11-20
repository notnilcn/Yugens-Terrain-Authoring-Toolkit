# Stamps, Ramps and Copying

The first four tools shape the ground by hand. These tools do bigger jobs: drop a whole mountain, paint a long slope, copy a piece of terrain, or mark an area with colour.

![[stamp.mp4]]

## Stamp

**Stamp** places a height map — a picture where white means high and black means low — onto the terrain. It is the fastest way to add a mountain, a crater or a valley.

Select the terrain, click **Stamp**, and look at the bottom strip.

![[stamp-panel.png]]

### The built-in stamps

| Stamp | Shape |
|---|---|
| **Mountain** | A ridged peak |
| **Crater** | A bowl with a raised rim |
| **Mesa** | A flat-topped plateau with steep sides |
| **Valley** | A winding low channel |
| **Hill** | A smooth dome |

### Placing a stamp

1. Hover over the terrain. An orange wireframe preview follows the cursor.
2. **Click** to stamp once.
3. **Drag** to pin the center, pull out to set the size and direction, then release to stamp.

| Shortcut | Changes |
|---|---|
| `Shift` + wheel | Size |
| `Ctrl` + wheel | Height |
| `Alt` + wheel, or `Z` / `X` | Rotation |

A negative **Height** carves downward. That is how you make a valley with the Mountain stamp, or a hill with the Crater stamp.

### Stamp modes

| Mode | Result |
|---|---|
| **Add** | Adds the stamp's height to the ground. |
| **Max** | Raises low ground up to the stamp, but never lowers high ground. |
| **Min** | Lowers high ground down to the stamp, but never raises low ground. |
| **Replace** | Replaces the area with the stamp, relative to the center height. |
| **Lift** | Raises toward the stamp, clamped by **Ceiling**. |
| **Carve** | Lowers toward the stamp, clamped by **Floor**. |
| **Level** | Pulls the area toward the **Level** height. |
| **Broad** | Builds a landmass from the **Level** height upward. Only raises. |

For most artist work, **Add** and **Carve** are enough. Use **Replace** when you want a clean, deliberate shape.

### Limits

With **Limits** off, a stamp can raise or lower the terrain as far as it likes. Turn **Limits** on to clamp it to the **Ceiling** and **Floor**, and to make **Level** / **Broad** use the **Level** value. This prevents a mountain stamp from punching through the bottom of the world.

### Your own stamp images

1. Make a grayscale image where white is high and black is low. A soft, blurry image works better than a hard-edged one.
2. Save it as `.png`, `.jpg`, `.webp` or `.exr`, at most 256 × 256 pixels.
3. Put it in `addons/MarchingSquaresTerrain/stamps/`.
4. Restart the editor. The stamp library is cached, so new files are not seen until a restart.
5. The file name becomes the menu name, with underscores turned into spaces.

> [!warning]
> Do not name your file `mountain.png`, `crater.png`, `mesa.png`, `valley.png` or `hill.png`. Those names are reserved for the built-in stamps and your file will be skipped.

## Gradient

**Gradient** pulls the terrain toward a straight ramp between two points. It is perfect for a beach that slopes into the sea, or a long road up a hillside.

1. Select **Gradient**.
2. `Ctrl` + left click the **start** point. A green marker appears.
3. `Ctrl` + right click the **end** point. An orange marker appears.
4. Drag with the **left mouse button** to paint the ramp.

- **Strength** controls how hard each stroke pulls.
- `Shift` + wheel changes the brush size.
- `Ctrl` + wheel changes the strength.
- The two points stay where they are until you set new ones. There is no “clear” button; just set a new start and end.

> [!tip]
> Gradient is not a replacement for Bridge. Use Bridge for a narrow ramp with clean edges; use Gradient for a wide, natural slope.

## Select (copy and paste)

**Select** copies a piece of terrain — heights, textures, grass and smooth flags — and pastes it somewhere else. It is how you repeat a cliff, a platform or a decorative patch without sculpting it twice.

1. Select **Select**.
2. Drag to paint the cells you want. `Shift` adds, `Alt` / `Esc` clears.
3. Press `C` (or click **Copy**). The hovered cell is the anchor.
4. Press `V` (or click **Paste**). You are now in paste mode.
5. Move the mouse. A preview shows the destination and whether it will raise (green) or lower (red).
6. **Left click** to paste. **Right click** or `Esc` leaves paste mode.

| Key | In paste mode |
|---|---|
| `Z` / `X` | Rotate by the grid step (60° on hex, 90° on square) |
| `M` | Mirror |
| **Paste Rotation** | The same rotation as a number |
| **Relative Height** | On: paste relative to the target. Off: paste the absolute heights. |

> [!tip]
> Copy a good-looking cliff once, then paste it along a coastline with **Relative Height** on. It saves hours and keeps the style consistent.

## Tint

**Tint** paints a flat colour over cells. It is not a texture; it is a coloured overlay stored with the terrain. Use it for territory borders, biome hints, paths, or marking an area for the level designer.

1. Select **Tint**.
2. Choose one of the ten colours: Red, Orange, Yellow, Green, Teal, Blue, Purple, Pink, White, Black.
3. Set **Opacity**.
4. Drag to paint. Right-drag erases.

Tints are drawn under fog of war and under runtime tints, so they work in the editor and in the game. They are saved in the terrain data.

> [!warning]
> Tint is an overlay, not a replacement for a texture. If you want the ground to actually look like dirt, paint the Dirt texture. Use Tint for information, not for the main look.

Next: [[07 The Pixel Look]]
