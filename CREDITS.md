# Credits & licenses

Marching Squares Terrain (pixel edition) is MIT licensed (see `LICENSE`). It is a Rust rewrite and integration of two
earlier script addons plus code and ideas from the projects below. Reference code is translated over to Rust
(the sources are scripts or C++, so they are translated rather than pasted) wherever their licence permits; add
attribution here when code is ported.

## Code it is derived from

| Source | Author | License | What was taken |
|---|---|---|---|
| [Yūgen's Terrain Authoring Toolkit](https://github.com/ToumaKamijou/Yugens-Terrain-Authoring-Toolkit) (MarchingSquaresTerrain) | Yūgen, forked from Jackachulian; collaborators DanTrz, powertomato, santarl | MIT | The marching-squares cell cases (ported 1:1 to `pt_core::square`), the cell-mode mesher (walls, 45° ramps, smooth bridge planes), triangle/hex/hex-ring grid math, merge modes, brush tools and their UX, ridge/ledge texturing, grass planting rules, the grass blade sprite (`addons/MarchingSquaresTerrain/textures/grass_blade.png`, the original's `resources/plugin_materials/grass_leaf_sprite.png`; the default grass sprite of every grass slot), the ground noise texture (`textures/ground/mst_noise.png`: the original's `grass_terrain_noise.res` / `wall_noise_texture.res` NoiseTexture2D baked to a 128² PNG; used by the importer and the *MST Classic* preset `texture_presets/mst_classic.tres` with the original's default albedos), the node and tool icons (`addons/MarchingSquaresTerrain/editor/icons/`), and the addon/class naming. |
| Level-builder feature set | — | MIT | Level-builder features the builder lacked: dissolve, offset, extra selection, extrude / inset modes, Bezier plane / block, extra shapes, STL / PLY export, document import, live adjust HUD, context menu, paint presets / smoothing groups / palettes, UV extras, and the modeler UI layout (`pt_modeler::{xg_tools, bezier, xg_shapes, xg_export, modeler_meta}` and `mst::builder`), including its mode icons `mode_{vertex,edge,face}.svg` in `editor/icons/builder/modeler/`. |
| poly2tri | Poly2Tri Contributors (Copyright (c) 2009-2018) | BSD-3-Clause (notice at the top of `pt_modeler/src/poly2tri/mod.rs`; redistributions in binary form must reproduce it) | Constrained Delaunay triangulation of poly-shape caps with holes (`pt_modeler::xg_shapes::poly_shape`). |

## Inspiration and direct-port references

| Source | Author | Notes |
|---|---|---|
| 3d-pixel-art-base-project | — | MIT (relicensed by the copyright holder) — pixel-perfect camera/scaler, cel ramps, outlines, foliage and water v1 reference; code may be ported directly over to Rust with attribution. |
| [cyclopsLevelBuilder](https://github.com/blackears/cyclopsLevelBuilder) | Mark McKay | MIT — the builder's mesh-editing vocabulary and tool profile: block / prism / stairs / cylinder / sphere primitives, vertex / edge / face editing, move, rotate, duplicate, clip and intersect, grid and vertex snapping, UV and vertex-colour brushes, material palette dock. Code may be ported directly over to Rust with attribution. |
| "Creating Particle Effects Without Particles", "Pixel Art Rain Shader", "Wind Animations" (videos in `~/g/code_examples/refs/`) | — | Techniques only (no code): prerendered tiling / looping particle flipbooks with corner clones and a one-loop time offset, scrolled under masks (`pt_core::flipbook`, `PixelParticleField`); pixel rain streaks with ground / water splash rings (`rain_splash.gdshader`); travelling wind gusts (`pixel_wind.gdshaderinc`). |
| [3d-pixel-art-in-godot](https://git.sr.ht/~denovodavid/3d-pixel-art-in-godot) | denovodavid | CC0 — texel-snapped camera with sub-texel upscale shift; `PixelRenderer.stabilize_rotation` ports the snap space of `camera_3d_texel_snapped3.gd` (lattice re-anchored when the camera rotates). |
| [godot-3d-pixel-art](https://github.com/astrellon/godot-3d-pixel-art) | Alan Lawrey | MIT — pixel art pipeline ideas; smoke puff column (`place_smoke.gd`) and noise light flicker (`light_flicker.gd`) behind `PixelSmoke` / `puff.gdshader`. |
| godot-3d-pixel-art (robert-k; `refs/robert-k-godot-3d-pixel-art`) | robert-k (repository LICENSE: "Copyright (c) 2021 teebarjunk", MIT) | MIT. Code ported to Rust / this repo's shaders, as follows. The god rays (`godrays/godrays.gd` + `godray.gdshader`) became `PixelSunShafts` (`mst/src/effects/sun_shafts.rs`, `shaders/sun_shafts.gdshader`). The stepped 12 fps animation playback of `scripts/player.gd` became `PixelAnimationStepper` (`mst/src/render/stepper.rs`). The courtyard demo layout (`tests/build_courtyard_demo.gd`) follows `scenes/main.tscn` / `assets/basic_scene.blend`. Its assets are listed under *Addon assets* below. |
| hex_map_godot | David Pruitt (after Jasper Flick's catlikecoding Hex Map tutorials) | MIT — the per-cell shader data texture (one texel per cell, visibility/explored channels, fading transitions, edit mode forces everything visible) behind the fog of war / cell tints (`pt_core::cell_shader_data`, `mst_cells.gdshaderinc`) and the `grid_on` terrain grid overlay. Also deterministic per-cell hash feature placement (scatter "Per Cell" mode, after catlikecoding). Waterfalls (`pt_core::waterfall`, `waterfall.gdshader`): the fall quad cut where it enters the water below (`_triangulate_waterfall_in_water`) and the two-layer scrolling river streaks (`compute_river`), ported from `hex_grid_chunk.gd` / `water.gdshaderinc`. Code may be ported directly over to Rust with attribution. |
| [Terrain3D](https://github.com/TokisanGames/Terrain3D) | Cory Petkovsek, Roope Palmroos et al. | MIT — auto-shader slope rules and the debug views list (slope, heightmap/contours, control/slot ids, region borders, grid); instancer workflow (paint/erase MultiMesh props, scale/rotation/colour jitter, re-snap after sculpting), slope range filter and gradient operation (`pt_core::scatter` / `pt_core::brush`). Code may be ported directly over to Rust with attribution. |
| godot-hex-map | Lary | MIT — selection + paste with 60° tile rotation (`pt_core::clipboard`). Code may be ported directly over to Rust with attribution. |
| *Fire / Rain / Grass Interaction System* and *Wind Animations* reference videos (`~/g/code_examples/refs/`) | — | Look only (no code): fire spreading through grass as blobby yellow / orange / red pixel flames, blackened grass left behind, rain putting it out with blue steam, gust fronts rolling over the grass. Behind `PixelFireSystem` (`pt_core::fire`, `fire_flame.gdshader`, `pixel_fire.gdshaderinc`) and the gust sway in `mst_grass.gdshader`. |
| t3ssel8r's devlogs | t3ssel8r | Depth/normal edge rules for outlines and convex crease highlights. |
| 3DPixelArt_Tutorial | Eduardo Schildt | MIT — single-shader fullscreen-quad variant of the look: `Shaders/edge_detection.gdshader` (depth/normal edge detection, texel-scaled pixelation), the source of the edge and dither vocabulary. Code may be ported directly over to Rust with attribution. |
| "Crafting a Better Shader for Pixel Art Upscaling" | — | The sharp (anti-aliased texel edge) upscale filter. |

## Addon assets

| Asset | Source | License |
|---|---|---|
| `addons/MarchingSquaresTerrain/assets/courtyard/{fountain,pillar,stone_ring,round_tree}.glb` | robert-k's godot-3d-pixel-art `assets/basic_scene.blend` (objects `Fountain`, `Pillar`, `Rocks`, `Tree` + children), exported to glTF with Blender 5.2. Each is moved to the origin, and the Blender materials get robert-k's Godot material albedos (`rock.tres`, `oak_bark.tres`; leaves green). | MIT (reference repository) |
| `addons/MarchingSquaresTerrain/assets/characters/fox/fox.glb` | robert-k's godot-3d-pixel-art `assets/fox/fox.glb`, unmodified (rig, Idle / Run / Jump / Fall / Attack / ToucheGround clips, embedded textures). The original author of the model is not stated in the source repository. | MIT (as shipped in the source repository) |
| `addons/MarchingSquaresTerrain/assets/rocks/stone_*.glb` | robert-k's godot-3d-pixel-art `assets/rocks/`. The file names match Kenney's *Nature Kit* stones (CC0); the base colour was set to robert-k's `stone.tres` albedo. | MIT / CC0 |
| `demo/assets/audio/wind_and_birds_loop.wav` | robert-k's godot-3d-pixel-art `audio/` (courtyard ambience). The original source is not stated in the source repository. | MIT (as shipped in the source repository) |

## Demo assets

| Asset | Author | License |
|---|---|---|
| `demo/assets/characters/*` — [KayKit Character Pack: Adventurers](https://kaylousberg.itch.io/kaykit-adventurers) | Kay Lousberg | CC0 |
| `demo/assets/characters/animations/Rig_Medium_Animations.glb` — `Rig_Medium` clips (Idle, Walking_A, Walking_Backwards, Running_A, Running_Strafe_Left/Right, Jump_Start, Jump_Idle, Jump_Land, Cheer) from `Characters/gltf/Knight.glb` of the official [KayKit Character Pack: Adventures 1.0](https://github.com/KayKit-Game-Assets/KayKit-Character-Pack-Adventures-1.0) repository (www.kaylousberg.com). Pruned to the skeleton + those clips (meshes, materials, textures and other clips removed); otherwise unmodified. | Kay Lousberg | CC0 1.0 (pack `LICENSE.txt`) |
| `demo/assets/models/player.glb`, `enemy.glb` | generated by this project's Blender scripts | same as this project |

## Demo-only dependencies

| Dependency | Author | License | Notes |
|---|---|---|---|
| `addons/phantom_camera_rust/` + `rust/crates/phantom_camera_rust/` — Rust port of [Phantom Camera](https://github.com/ramokz/phantom-camera) 0.11.0.3 | port: this project's author; original: Marcus Skov (ramokz) | GPL-3.0-only (port, `addons/phantom_camera_rust/LICENSE`); original MIT (`LICENSE-PhantomCamera-MIT`) | Third-person camera of the demo player. Separate GDExtension library; the MIT addon never links it. Delete both folders + the autoload to drop it. |

The default ground textures (`addons/MarchingSquaresTerrain/textures/ground/*.png` except `mst_noise.png`) are baked from the addon's own procedural slot patterns (same pixels as the generated fallback); the default palette is generated procedurally by the addon.
