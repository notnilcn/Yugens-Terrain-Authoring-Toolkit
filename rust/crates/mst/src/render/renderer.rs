//! `PixelRenderer` — 3D pixel art for the Compatibility renderer.
//!
//! Pipeline (all plain SubViewports + canvas shaders, no compositor or compute):
//!
//! ```text
//! ResolveViewport (2D, low-res + 1 texel margin)          <- renders after its children
//!   ├─ ColorViewport  (3D, low-res + margins) Camera3D: scene color
//!   ├─ MetaViewport   (3D, same size)       Camera3D: metadata layer only
//!   ├─ IdViewport     (3D, same size)       Camera3D: id pass (outline ids 0-255, palette/dither)
//!   └─ ColorRect  pixel_resolve.gdshader   (blocks, outlines, rays, dither, palette)
//! CanvasLayer(-100) ColorRect pixel_upscale.gdshader   (sub-texel shift, sharp upscale)
//! ```
//!
//! The cameras copy the game's active camera every frame. For orthographic cameras the
//! position is snapped to the texel lattice and the remainder is applied in the upscale,
//! so pixels never swim while the view still scrolls smoothly. Perspective cameras with a
//! `pixel_snap_distance` meta get the same on their focus plane (exact there, close to it for
//! low-FOV cameras).

use godot::classes::camera_3d::{KeepAspect, ProjectionType};
use godot::classes::control::LayoutPreset;
use godot::classes::environment::{AmbientSource, BgMode, ReflectionSource, ToneMapper};
use godot::classes::viewport::{Msaa, ScreenSpaceAa};
use godot::classes::sub_viewport::UpdateMode;
use godot::classes::scene_tree::GroupCallFlags;
use godot::classes::{Camera3D, CanvasLayer, ColorRect, Environment, INode, ImageTexture, Node, ShaderMaterial, SubViewport};
use godot::prelude::*;

use super::editor_preview::EditorHost;
use super::object;
use super::outline_palette::{self, PixelOutlinePalette};
use super::palette::PixelPalette;
use crate::globals;
use crate::util::*;

const MARGIN: i32 = 4;
/// Layers the 3D editor reserves for itself (Node3DEditor: MISC_TOOL 24, GIZMO_GRID 25,
/// GIZMO_EDIT 26, GIZMO_BASE 27 + viewport index); never drawn by the editor preview.
const EDITOR_ONLY_LAYERS: u32 = 0xff00_0000;
// The meta / id cameras must never see the editor's layers either (the origin axis lines sit on
// them and would be outlined in the preview): their masks are the meta layer (1–20) + the id bit.
const _: () = assert!(EDITOR_ONLY_LAYERS & (1 << pt_core::meta::ID_CAMERA_BIT) == 0);

struct Pipeline {
    root: Gd<Node>,
    resolve_vp: Gd<SubViewport>,
    color_vp: Gd<SubViewport>,
    meta_vp: Gd<SubViewport>,
    id_vp: Gd<SubViewport>,
    color_cam: Gd<Camera3D>,
    meta_cam: Gd<Camera3D>,
    id_cam: Gd<Camera3D>,
    resolve_mat: Gd<ShaderMaterial>,
    upscale_mat: Gd<ShaderMaterial>,
    #[allow(dead_code)] // kept for ownership clarity; freed with `root`
    /// Game: full-screen layer behind the UI. Editor preview: `None` (the rect sits in the editor).
    display: Option<Gd<CanvasLayer>>,
    display_rect: Gd<ColorRect>,
    /// Editor preview only: the editor 3D viewport being mirrored.
    host: Option<EditorHost>,
    /// Whether the viewports currently render (editor preview pauses while the 3D screen is hidden).
    rendering: bool,
}

#[derive(GodotClass)]
#[class(tool, base = Node)]
pub struct PixelRenderer {
    #[export]
    #[var(set = set_enabled)]
    enabled: bool,
    /// Pipeline mode (port of the original `PixelizerManager3D.mode`). Anchor: the full
    /// pipeline (metadata + id passes; per-object pixel sizes, outlines, creases). Low Res:
    /// color pass only, upscaled (no metadata passes, so no outlines or per-object blocks);
    /// dither, palette and grading still apply.
    #[export(enum = (Anchor = 0, LowRes = 1))]
    mode: i32,
    /// Screen pixels per low-res texel.
    #[export(range = (1.0, 16.0, 1.0))]
    pixel_size: i32,
    /// Fixed low-res height in texels (0 = derive from `pixel_size`).
    #[export(range = (0.0, 1080.0, 1.0))]
    fixed_height: i32,
    /// Camera to follow (empty = the viewport's current Camera3D, re-resolved every frame).
    #[export]
    camera: Option<Gd<Camera3D>>,
    /// Snap orthographic cameras to the texel lattice (stable pixels). Perspective cameras are
    /// snapped too when they carry a `pixel_snap_distance` meta (the lattice on that plane).
    #[export]
    snap_camera: bool,
    /// Apply the snapped-away remainder in the upscale (smooth scrolling).
    #[export]
    smooth_scroll: bool,
    /// Anti-aliased texel edges for non-integer scales.
    #[export]
    sharp_upscale: bool,
    /// Render layer reserved for the metadata camera (pixelized geometry is put on it).
    #[export(range = (1.0, 20.0, 1.0))]
    #[var(set = set_meta_layer)]
    meta_layer: i32,
    /// View depth (m) covered by the metadata depth encoding.
    #[export(range = (1.0, 4000.0, 1.0, or_greater))]
    depth_range: f32,
    /// Re-anchor the snap lattice on the camera whenever it rotates (snap space of
    /// 3d-pixel-art-in-godot's `Camera3DTexelSnapped3`): an orbiting camera keeps its subject at
    /// the same sub-texel phase instead of jumping up to half a texel per frame, and translation
    /// stays world-locked between rotations. Off = lattice anchored at the world origin.
    #[export_group(name = "Stabilize", prefix = "stabilize_")]
    #[export]
    stabilize_rotation: bool,
    /// Editor only: show the pixelated image in the editor's 3D viewport, following the editor
    /// camera (also toggled by the "Pixel Preview" button in the 3D viewport toolbar).
    #[export_group(name = "Editor", prefix = "editor_")]
    #[export]
    #[var(set = set_editor_preview)]
    editor_preview: bool,

    #[export_group(name = "Outlines")]
    #[export]
    outlines: bool,
    /// Draw silhouettes against the sky/background too.
    #[export_subgroup(name = "Outline", prefix = "outline_")]
    #[export]
    outline_background: bool,
    /// Depth step (m) that counts as a silhouette edge.
    #[export(range = (0.01, 10.0, 0.01))]
    depth_threshold: f32,
    /// Crease sensitivity (lower = more creases highlighted).
    #[export(range = (0.0, 2.0, 0.01))]
    normal_threshold: f32,
    /// How much silhouette pixels darken their own color (for outline ids without a color).
    #[export(range = (0.0, 1.0, 0.01))]
    outline_darken: f32,
    /// How much crease pixels brighten.
    #[export(range = (0.0, 1.0, 0.01))]
    highlight: f32,
    /// Legacy outline colors for ids 0-7 (used where `outline_palette` has no color).
    /// Alpha > 0 tints the outline towards the color.
    #[export]
    outline_colors: PackedColorArray,
    /// Per-id exterior/interior outline colors (ids 0-253; 254 hover, 255 selection).
    #[export]
    outline_palette: Option<Gd<PixelOutlinePalette>>,
    /// Draw the palette's interior row where surfaces with different outline ids touch.
    #[export]
    id_edges: bool,
    /// Depth steps (m) below this between parallel vertical faces of the same object are wall
    /// folds (hex cliff corners), not silhouettes. 0 = outline every depth step.
    #[export(range = (0.0, 20.0, 0.1))]
    fold_limit: f32,
    /// Walls seen exactly edge-on (square terrain under an axis-aligned camera: a straight
    /// vertical depth step onto lower ground of the same object) draw as a ledge (lip highlight)
    /// instead of a silhouette, so they don't pop between a hard line and a thin face as the
    /// camera turns.
    #[export]
    ledge_walls: bool,
    /// Render the id pass (one extra low-res scene pass): outline ids 0-255, hover/selection
    /// highlights and per-object palette/dither toggles. Off = ids 0-7 only.
    #[export]
    extended_ids: bool,

    #[export_group(name = "Object Defaults")]
    #[export_subgroup(name = "Object", prefix = "object_")]
    /// Pixel size of PixelObjects whose chain is all "inherit" (-1).
    #[export(range = (1.0, 4.0, 1.0))]
    object_pixel_size: i32,
    /// First outline id handed out by automatic allocation (PixelObject `outline_id` -2, or
    /// a top-level PixelObject left at inherit). Ids cycle through `auto_id_first`..=253.
    #[export(range = (1.0, 253.0, 1.0))]
    auto_id_first: i32,
    /// Outline of PixelObjects whose chain is all "inherit".
    #[export]
    object_outline: bool,

    #[export_group(name = "Focus")]
    /// Keep `focus_target`'s subtree highlighted with the selection outline (id 255), so the
    /// subject always reads (port of the original `auto_focus`). Runtime only.
    #[export]
    auto_focus: bool,
    #[export]
    focus_target: Option<Gd<Node3D>>,

    #[export_group(name = "Color")]
    #[export]
    palette: Option<Gd<PixelPalette>>,
    #[export]
    palette_enabled: bool,
    #[export(range = (0.0, 1.0, 0.01))]
    palette_mix: f32,
    #[export]
    dither_enabled: bool,
    #[export(range = (0.0, 0.5, 0.001))]
    dither_spread: f32,
    #[export(enum = (Bayer4 = 4, Bayer8 = 8))]
    dither_size: i32,
    #[export(range = (0.0, 2.0, 0.01))]
    saturation: f32,
    #[export(range = (0.0, 2.0, 0.01))]
    contrast: f32,
    #[export(range = (-1.0, 1.0, 0.01))]
    brightness: f32,

    #[export_group(name = "God Rays")]
    /// Stable3D light-sheet god rays (port of `StablePixelArtGodRays.ush`, MIT): world-anchored
    /// sheets containing the sun beam axis stack up the sunlight let through the PixelSky
    /// cloud gaps. Defaults are Stable3D's `MI_StablePixelArtGodRays_Default` (cm -> m).
    #[export]
    god_rays: bool,
    /// RayColor (used unless `ray_use_sun_color`).
    #[export_subgroup(name = "Ray", prefix = "ray_")]
    #[export]
    ray_color: Color,
    /// GodRayUseSunColor: tint with the sun's hue instead of `ray_color`.
    #[export]
    ray_use_sun_color: bool,
    /// Multiplier on Stable3D's default ray strength: 1 = the look of Stable3D's
    /// `MI_StablePixelArtGodRays_Default` in this renderer (visible, subtle shafts; a light
    /// haze at noon). The LDR calibration of Stable3D's HDR RayStrength is built in.
    #[export(range = (0.0, 4.0, 0.01, or_greater))]
    ray_strength: f32,
    /// Most light sheets one view ray crosses (Stable3D: 24).
    #[export(range = (2.0, 48.0, 1.0))]
    ray_samples: i32,
    /// MaxRayLength (m).
    #[export]
    ray_max_distance: f32,
    /// Extra quantisation of the sheet stack into this many bands (1 = off, as Stable3D).
    #[export(range = (1.0, 16.0, 1.0))]
    ray_bands: i32,
    /// SheetSpacing (m): distance between the world-anchored light sheets.
    #[export_subgroup(name = "Sheets", prefix = "ray_sheet_")]
    #[export(range = (0.05, 50.0, 0.01, or_greater))]
    ray_sheet_spacing: f32,
    /// SheetQuantum: light added per fully lit sheet.
    #[export(range = (0.0, 10.0, 0.01, or_greater))]
    ray_sheet_quantum: f32,
    /// SheetMaxStack: cap of the stacked light.
    #[export(range = (0.0, 10.0, 0.01, or_greater))]
    ray_sheet_max_stack: f32,
    /// SheetDistanceFalloff (1/m): exponential fade with distance from the camera.
    #[export(range = (0.0, 1.0, 0.001))]
    ray_sheet_distance_falloff: f32,
    /// SheetSurfaceFade (m): sheets fade in over this distance before the surface.
    #[export(range = (0.0, 1000.0, 0.1, or_greater))]
    ray_sheet_surface_fade: f32,
    /// SheetCloudSoftness: edge softness of the cloud gaps seen by the sheets.
    #[export(range = (0.0, 0.5, 0.0005))]
    ray_sheet_cloud_softness: f32,
    /// GodRayGapErosion: shrinks the lit gaps (0 = whole gaps).
    #[export_subgroup(name = "Ray", prefix = "ray_")]
    #[export(range = (0.0, 0.99, 0.01))]
    ray_gap_erosion: f32,
    /// CloudShadowStrength: how much a cloud blocks a sheet.
    #[export_subgroup(name = "Cloud Shadow", prefix = "ray_cloud_shadow_")]
    #[export(range = (0.0, 1.0, 0.01))]
    ray_cloud_shadow_strength: f32,
    /// GodRayCameraNearFadeStart (m): no sheets closer to the camera than this.
    #[export_subgroup(name = "Near Fade", prefix = "ray_near_")]
    #[export(range = (0.0, 100.0, 0.1, or_greater))]
    ray_near_fade_start: f32,
    /// GodRayCameraNearFade (m): fade-in range after the start.
    #[export(range = (0.0, 100.0, 0.1, or_greater))]
    ray_near_fade: f32,
    /// GodRayCameraNearFadePower: fade-in curve.
    #[export(range = (0.05, 8.0, 0.01))]
    ray_near_fade_power: f32,
    /// GodRayDepthCutoff (m): no sheets beyond this distance (0 = off).
    #[export_subgroup(name = "Ray", prefix = "ray_")]
    #[export(range = (0.0, 1000.0, 0.1, or_greater))]
    ray_depth_cutoff: f32,
    /// CloudSunFollow: 1 = the beams follow the sun direction, 0 = vertical.
    #[export(range = (0.0, 1.0, 0.01))]
    ray_sun_follow: f32,
    /// Drive the sheets' cloud threshold over the day like Stable3D's demo day sequence
    /// (`DS_GodRays`: overcast at night, clearing towards noon) from the sibling
    /// PixelDayNight's `time_of_day`. Off = 1 - PixelSky coverage (gaps match the ground
    /// cloud shadows).
    #[export]
    ray_day_cycle: bool,
    /// Dawn / dusk shafts with `ray_day_cycle`: Stable3D's day curve is overcast around sunrise
    /// and sunset (no shafts then) and only partly open in the morning / afternoon. This keeps
    /// the sheets' cloud gaps at least this far towards the curve's mid-morning opening
    /// (0 = the curve as is, 1 = open like mid-morning all day), so a low sun still pours long
    /// shafts. Night is unaffected (rays fade with `pt_night`).
    #[export(range = (0.0, 1.0, 0.01))]
    ray_dawn_dusk: f32,
    /// LDR eye adaptation of the sheet stack (0 = off, raw Stable3D stack). A view ray that
    /// crosses the beams side-on passes many sheets, so the stack sits at the cap over the
    /// whole frame: in Stable3D the HDR add is then pulled back by UE's auto exposure, here it
    /// would be a flat, milky veil. The stack is scaled by min(1, adaptation / potential),
    /// where potential is the stack the ray would gather if every sheet it crosses were lit,
    /// so views along / against the sun (few sheets, distinct shafts) are unchanged and
    /// side-on views keep the shafts at a comparable strength instead of washing out.
    #[export(range = (0.0, 10.0, 0.01, or_greater))]
    ray_adaptation: f32,
    #[export_group(name = "Debug")]
    #[export(enum = (Final, Normals, Depth, PixelSize, OutlineIds, RawColor, OutlineCause, GodRays))]
    debug_view: i32,

    pipeline: Option<Pipeline>,
    outline_tex: Option<Gd<ImageTexture>>,
    outline_key: (u64, i64, Vec<[u32; 4]>),
    bound_camera: Option<Gd<Camera3D>>,
    saved_cull_mask: u32,
    low_res: Vector2i,
    texel_world: f32,
    /// Snap space: camera basis the lattice was last anchored for, and the anchor (world).
    snap_basis: Basis,
    snap_anchor: Vector3,
    /// Subtree currently highlighted by auto focus.
    focused: Option<Gd<Node>>,
    focus_frames: u32,
    base: Base<Node>,
}

/// Stable3D's RayStrength (0.01) x Global Brightness (2) of `MI_StablePixelArtGodRays_Default`.
const STABLE3D_RAY_STRENGTH: f32 = 0.02;

/// LDR calibration of Stable3D's ray strength. Stable3D adds its rays to UE's HDR scene color
/// before auto exposure and the filmic tonemapper, from an orthographic camera standing far
/// back (MaxRayLength / SheetSurfaceFade ~300 m), so its light sheets stack to the cap in the
/// cloud gaps. Here the rays go into the finished LDR color (as a linear-light screen, see
/// `pixel_resolve.gdshader`) from cameras a few tens of metres away, where the surface fade
/// leaves the stack at roughly a quarter to a half of the cap. x4 brings the shafts back to
/// Stable3D's subtle default look (judged on the hex island overview / player view at day,
/// afternoon and noon), so `PixelRenderer.ray_strength = 1` means "Stable3D default".
const LDR_RAY_CALIBRATION: f32 = 4.0;

/// Default `ray_adaptation`: the potential sheet stack (in SheetQuantum units) up to which the
/// god rays are added unscaled. Tuned on the hex island player view (yaw sweep 0..300 at
/// time 0.4) so the side-on views keep about the frame contrast of the along-the-sun ones.
const RAY_ADAPTATION: f32 = 1.6;

/// Stable3D demo day sequence (`DS_GodRays`): the shared "Cloud Threshold" over one day
/// (0 = midnight, 0.5 = noon), cubic keys with flat tangents. Higher = fewer clouds.
const STABLE3D_THRESHOLD_KEYS: [(f32, f32); 7] =
    [(0.0, 0.1), (0.229_167, 0.1), (0.375, 0.47), (0.5, 0.67), (0.625, 0.47), (0.770_833, 0.1), (1.0, 0.1)];

/// `ray_dawn_dusk` = 1 holds the sheets' cloud threshold at the day curve's mid-morning key.
const DAWN_DUSK_THRESHOLD: f32 = 0.47;

/// Cloud threshold of the Stable3D day curve at `time_of_day` (wraps).
pub fn stable3d_cloud_threshold(time_of_day: f32) -> f32 {
    let t = time_of_day.rem_euclid(1.0);
    for w in STABLE3D_THRESHOLD_KEYS.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t <= t1 {
            let x = ((t - t0) / (t1 - t0).max(1e-6)).clamp(0.0, 1.0);
            return v0 + (v1 - v0) * x * x * (3.0 - 2.0 * x);
        }
    }
    STABLE3D_THRESHOLD_KEYS[6].1
}

#[cfg(test)]
mod tests {
    use super::stable3d_cloud_threshold as f;

    #[test]
    fn stable3d_day_curve() {
        assert!((f(0.5) - 0.67).abs() < 1e-5);
        assert!((f(0.1) - 0.1).abs() < 1e-5);
        assert!((f(0.375) - 0.47).abs() < 1e-5);
        assert!((f(1.5) - 0.67).abs() < 1e-5);
        let d = f(0.7);
        assert!(d > 0.1 && d < 0.47);
    }
}

#[godot_api]
impl INode for PixelRenderer {
    fn on_validate_property(&self, property: &mut godot::register::info::PropertyInfo) {
        crate::util::space_enum_hint(property);
    }

    fn init(base: Base<Node>) -> Self {
        let mut outline_colors = PackedColorArray::new();
        for _ in 0..8 {
            outline_colors.push(Color::from_rgba(0.0, 0.0, 0.0, 0.0));
        }
        PixelRenderer {
            enabled: true,
            mode: 0,
            editor_preview: false,
            pixel_size: 4,
            fixed_height: 0,
            camera: None,
            snap_camera: true,
            smooth_scroll: true,
            stabilize_rotation: true,
            sharp_upscale: true,
            meta_layer: globals::DEFAULT_META_LAYER as i32,
            depth_range: 200.0,
            outlines: true,
            outline_background: true,
            depth_threshold: 0.35,
            normal_threshold: 0.3,
            outline_darken: 0.55,
            highlight: 0.22,
            outline_colors,
            outline_palette: None,
            id_edges: true,
            fold_limit: 4.0,
            ledge_walls: true,
            extended_ids: true,
            object_pixel_size: 1,
            auto_id_first: 1,
            auto_focus: false,
            focus_target: None,
            focused: None,
            focus_frames: 0,
            object_outline: true,
            palette: None,
            palette_enabled: false,
            palette_mix: 1.0,
            dither_enabled: true,
            dither_spread: 0.03,
            dither_size: 4,
            saturation: 1.05,
            contrast: 1.0,
            brightness: 0.0,
            god_rays: false,
            ray_color: Color::from_rgb(1.0, 0.984, 0.266),
            ray_use_sun_color: false,
            ray_strength: 1.0,
            ray_samples: 24,
            ray_max_distance: 300.0,
            ray_bands: 1,
            ray_sheet_spacing: 2.23,
            ray_sheet_quantum: 2.660_933,
            ray_sheet_max_stack: 3.0,
            ray_sheet_distance_falloff: 0.0,
            ray_sheet_surface_fade: 327.88,
            ray_sheet_cloud_softness: 0.001,
            ray_gap_erosion: 0.4,
            ray_cloud_shadow_strength: 0.9,
            ray_near_fade_start: 5.9,
            ray_near_fade: 8.59,
            ray_near_fade_power: 1.35,
            ray_depth_cutoff: 283.39,
            ray_sun_follow: 1.0,
            ray_day_cycle: true,
            ray_dawn_dusk: 0.0,
            ray_adaptation: RAY_ADAPTATION,
            debug_view: 0,
            pipeline: None,
            outline_tex: None,
            outline_key: (0, 0, Vec::new()),
            bound_camera: None,
            saved_cull_mask: u32::MAX,
            low_res: Vector2i::ZERO,
            texel_world: 0.0,
            snap_basis: Basis::IDENTITY,
            snap_anchor: Vector3::ZERO,
            base,
        }
    }

    fn ready(&mut self) {
        globals::ensure();
        globals::set_meta_layer(self.meta_layer as u32);
        self.push_object_defaults();
        self.base_mut().set_process_priority(10_000);
        self.base_mut().set_process(true);
        if !is_editor() && self.enabled {
            self.build();
        }
    }

    fn process(&mut self, _delta: f64) {
        if is_editor() {
            // Built lazily (also after the scene tab is switched back): never from a setter.
            let host_lost = self.pipeline.as_ref().is_some_and(|p| p.host.as_ref().is_some_and(|h| !h.is_valid()));
            if host_lost {
                self.teardown();
            }
            if self.editor_preview && self.pipeline.is_none() {
                self.build();
            }
        }
        if self.pipeline.is_some() {
            self.sync();
        }
        if !is_editor() {
            self.update_focus();
        }
    }

    fn exit_tree(&mut self) {
        globals::set("pt_rays_a", &Vector4::ZERO.to_variant());
        self.teardown();
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        if self.base().get_parent().is_some_and(|p| p.find_children_ex("*").type_("PixelRenderer").owned(false).done().len() > 1) {
            w.push("Only one PixelRenderer per scene is supported.");
        }
        w
    }
}

impl PixelRenderer {
    fn build(&mut self) {
        if self.pipeline.is_some() {
            return;
        }
        let host = if is_editor() {
            match EditorHost::find() {
                Some(h) => Some(h),
                None => return,
            }
        } else {
            None
        };
        let mut root = Node::new_alloc();
        root.set_name("PixelPipeline");

        let mut resolve_vp = SubViewport::new_alloc();
        resolve_vp.set_name("ResolveViewport");
        resolve_vp.set_disable_3d(true);
        resolve_vp.set_update_mode(UpdateMode::ALWAYS);
        resolve_vp.set_transparent_background(false);
        resolve_vp.set_default_canvas_item_texture_filter(godot::classes::viewport::DefaultCanvasItemTextureFilter::NEAREST);

        let mk3d = |name: &str, transparent: bool| {
            let mut vp = SubViewport::new_alloc();
            vp.set_name(name);
            vp.set_update_mode(UpdateMode::ALWAYS);
            vp.set_transparent_background(transparent);
            vp.set_msaa_3d(Msaa::DISABLED);
            vp.set_screen_space_aa(ScreenSpaceAa::DISABLED);
            vp.set_use_debanding(false);
            vp.set_use_taa(false);
            vp
        };
        let mut color_vp = mk3d("ColorViewport", false);
        let mut meta_vp = mk3d("MetaViewport", true);
        let mut id_vp = mk3d("IdViewport", true);
        if let Some(h) = &host {
            // Render the edited scene's world (the editor viewport's), wherever this node lives.
            if let Some(world) = h.viewport.find_world_3d() {
                color_vp.set_world_3d(&world);
                meta_vp.set_world_3d(&world);
                id_vp.set_world_3d(&world);
            }
        }
        meta_vp.set_positional_shadow_atlas_size(0);
        id_vp.set_positional_shadow_atlas_size(0);

        let mut color_cam = Camera3D::new_alloc();
        color_cam.set_name("ColorCamera");
        let mut meta_cam = Camera3D::new_alloc();
        meta_cam.set_name("MetaCamera");
        let mut env = Environment::new_gd();
        env.set_background(BgMode::CLEAR_COLOR);
        env.set_ambient_source(AmbientSource::DISABLED);
        env.set_reflection_source(ReflectionSource::DISABLED);
        env.set_tonemapper(ToneMapper::LINEAR);
        env.set_tonemap_exposure(1.0);
        meta_cam.set_environment(&env);
        let mut id_cam = Camera3D::new_alloc();
        id_cam.set_name("IdCamera");
        id_cam.set_environment(&env);
        color_vp.add_child(&color_cam);
        meta_vp.add_child(&meta_cam);
        id_vp.add_child(&id_cam);

        let mut resolve_rect = ColorRect::new_alloc();
        resolve_rect.set_name("Resolve");
        resolve_rect.set_anchors_preset(LayoutPreset::FULL_RECT);
        let mut resolve_mat = shader_material("pixel_resolve.gdshader");
        resolve_mat.set_shader_parameter("color_tex", &color_vp.get_texture().to_variant());
        resolve_mat.set_shader_parameter("meta_tex", &meta_vp.get_texture().to_variant());
        resolve_mat.set_shader_parameter("id_tex", &id_vp.get_texture().to_variant());
        resolve_mat.set_shader_parameter("margin", &Vector2i::new(MARGIN, MARGIN).to_variant());
        resolve_rect.set_material(&resolve_mat);

        // Child viewports render before their parent, so color/meta are always this frame's.
        resolve_vp.add_child(&color_vp);
        resolve_vp.add_child(&meta_vp);
        resolve_vp.add_child(&id_vp);
        resolve_vp.add_child(&resolve_rect);

        let mut display_rect = ColorRect::new_alloc();
        display_rect.set_name("Upscale");
        display_rect.set_anchors_preset(LayoutPreset::FULL_RECT);
        display_rect.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);
        let mut upscale_mat = shader_material("pixel_upscale.gdshader");
        upscale_mat.set_shader_parameter("source", &resolve_vp.get_texture().to_variant());
        display_rect.set_material(&upscale_mat);

        root.add_child(&resolve_vp);
        let display = match &host {
            Some(h) => {
                // Above the editor's 3D image, below its overlay controls (gizmo axes, info);
                // mouse-transparent so editing works as usual.
                display_rect.set_name("PixelPreviewOverlay");
                let mut c = h.container.clone();
                c.add_child(&display_rect);
                None
            }
            None => {
                let mut display = CanvasLayer::new_alloc();
                display.set_name("PixelDisplay");
                display.set_layer(-100);
                display.add_child(&display_rect);
                root.add_child(&display);
                Some(display)
            }
        };
        let mut this = self.to_gd();
        add_internal(&mut this, &root);

        self.pipeline = Some(Pipeline {
            root,
            resolve_vp,
            color_vp,
            meta_vp,
            id_vp,
            color_cam,
            meta_cam,
            id_cam,
            resolve_mat,
            upscale_mat,
            display,
            display_rect,
            host,
            rendering: true,
        });
        self.low_res = Vector2i::ZERO;
        if is_editor() {
            self.notify_editor_preview(true);
        }
    }

    /// Publishes the inherit-chain fallbacks and refreshes PixelObjects when they change.
    fn push_object_defaults(&mut self) {
        object::set_auto_first(self.auto_id_first as u32);
        if object::set_defaults(self.object_pixel_size, self.object_outline) && self.base().is_inside_tree() {
            let mut tree = self.base().get_tree();
            tree.call_group_flags(GroupCallFlags::DEFERRED, globals::REFRESH_GROUP, "refresh_pixel_settings", &[]);
        }
    }

    /// The (cached) 256 x 2 outline palette texture: palette resource, legacy colors, overrides.
    fn outline_texture(&mut self) -> Option<Gd<ImageTexture>> {
        let legacy: Vec<[u32; 4]> = self.outline_colors.as_slice().iter().map(|c| [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]).collect();
        let pal_id = self.outline_palette.as_ref().map(|p| p.instance_id().to_i64()).unwrap_or(0);
        let key = (outline_palette::version(), pal_id, legacy);
        if self.outline_tex.is_some() && self.outline_key == key {
            return self.outline_tex.clone();
        }
        let (mut ext, mut int) = match &self.outline_palette {
            Some(p) => p.bind().rows(),
            None => outline_palette::default_rows_vec(),
        };
        for (i, c) in self.outline_colors.as_slice().iter().enumerate().take(8) {
            if c.a > 0.0 && ext[i].a == 0.0 {
                ext[i] = *c;
            }
            if c.a > 0.0 && int[i].a == 0.0 {
                int[i] = *c;
            }
        }
        let img = outline_palette::bake(&ext, &int, &outline_palette::overrides())?;
        match &mut self.outline_tex {
            Some(t) => t.update(&img),
            None => self.outline_tex = ImageTexture::create_from_image(&img),
        }
        self.outline_key = key;
        self.outline_tex.clone()
    }

    fn release_camera(&mut self) {
        if let Some(mut cam) = self.bound_camera.take()
            && cam.is_instance_valid()
        {
            cam.set_cull_mask(self.saved_cull_mask);
        }
    }

    fn teardown(&mut self) {
        self.release_camera();
        if let Some(p) = self.pipeline.take() {
            let editor = p.host.is_some();
            if editor {
                // The overlay lives in the editor's tree, not under our pipeline root.
                free_node(Some(p.display_rect));
                globals::set("pt_texel_world", &0.0f32.to_variant());
            }
            free_node(Some(p.root));
            if editor {
                self.notify_editor_preview(false);
            }
        }
    }

    /// Keeps the auto-focus target highlighted (re-applied every 30 frames so runtime-spawned
    /// children are covered); switching targets restores the previous one.
    fn update_focus(&mut self) {
        let want: Option<Gd<Node>> =
            if self.auto_focus { self.focus_target.clone().filter(|t| t.is_instance_valid()).map(|t| t.upcast()) } else { None };
        if want != self.focused {
            if let Some(prev) = self.focused.take().filter(|p| p.is_instance_valid()) {
                object::highlight(&prev, 0);
            }
            if let Some(t) = &want {
                object::highlight(t, 2);
            }
            self.focused = want;
            self.focus_frames = 0;
            return;
        }
        if let Some(t) = &self.focused {
            self.focus_frames += 1;
            if self.focus_frames >= 30 {
                self.focus_frames = 0;
                object::highlight(t, 2);
            }
        }
    }

    /// Editor preview on/off: PixelObjects apply in the editor only while it is shown.
    fn notify_editor_preview(&mut self, on: bool) {
        if object::set_editor_preview(on) && self.base().is_inside_tree() {
            let mut tree = self.base().get_tree();
            tree.call_group_flags(GroupCallFlags::DEFERRED, globals::REFRESH_GROUP, "refresh_pixel_settings", &[]);
        }
    }

    fn active_camera(&self) -> Option<Gd<Camera3D>> {
        if let Some(c) = &self.camera
            && c.is_instance_valid()
        {
            return Some(c.clone());
        }
        self.base().get_viewport().and_then(|vp| vp.get_camera_3d())
    }

    /// Pauses/resumes the low-res viewports (editor preview while the 3D screen is hidden).
    fn set_rendering(&mut self, on: bool) {
        let Some(p) = &mut self.pipeline else { return };
        if p.rendering == on {
            return;
        }
        p.rendering = on;
        let mode = if on { UpdateMode::ALWAYS } else { UpdateMode::DISABLED };
        p.resolve_vp.set_update_mode(mode);
        p.color_vp.set_update_mode(mode);
        let anchor = self.mode == 0;
        p.meta_vp.set_update_mode(if on && anchor { UpdateMode::ALWAYS } else { UpdateMode::DISABLED });
        p.id_vp.set_update_mode(if on && anchor && self.extended_ids { UpdateMode::ALWAYS } else { UpdateMode::DISABLED });
        p.display_rect.set_visible(on);
    }

    /// Camera + on-screen size to mirror: the editor camera in preview mode, else the game's.
    fn view_source(&mut self) -> Option<(Gd<Camera3D>, Vector2)> {
        let host = self.pipeline.as_ref().and_then(|p| p.host.as_ref());
        if let Some(h) = host {
            if !h.is_shown() {
                return None;
            }
            let (cam, size) = (h.camera()?, h.screen_size());
            // The editor re-applies its own cull mask (e.g. "View Gizmos"); re-capture it and keep
            // the editor camera blank: the overlay covers it and it would only cost a full render.
            if self.bound_camera.as_ref() != Some(&cam) {
                self.release_camera();
                self.bound_camera = Some(cam.clone());
            }
            let mask = cam.get_cull_mask();
            if mask != 0 {
                self.saved_cull_mask = mask;
                let mut c = cam.clone();
                c.set_cull_mask(0);
            }
            return Some((cam, size));
        }
        let cam = self.active_camera()?;
        // (Re)bind the game camera: hide the 3D world from it (we draw it ourselves).
        if self.bound_camera.as_ref() != Some(&cam) {
            self.release_camera();
            self.saved_cull_mask = cam.get_cull_mask();
            let mut c = cam.clone();
            c.set_cull_mask(0);
            self.bound_camera = Some(cam.clone());
        }
        let vp = self.base().get_viewport()?;
        Some((cam, vp.get_visible_rect().size))
    }

    /// God-ray tint (RayColor, or the sun's hue with `ray_use_sun_color`) and the sheets'
    /// cloud threshold (Stable3D day curve, or 1 - PixelSky coverage), from this node's siblings.
    fn ray_inputs(&self) -> (Color, f32) {
        let Some(parent) = self.base().get_parent() else { return (self.ray_color, 0.5) };
        let mut tint = self.ray_color;
        let mut threshold = 0.5;
        let mut have_day = false;
        for n in parent.get_children().iter_shared() {
            if self.ray_use_sun_color {
                if let Ok(l) = n.clone().try_cast::<godot::classes::DirectionalLight3D>() {
                    // Stable3D: sun illuminance divided by its largest channel.
                    let c = l.get_color();
                    let peak = c.r.max(c.g).max(c.b).max(1e-4);
                    tint = Color::from_rgb(c.r / peak, c.g / peak, c.b / peak);
                }
            }
            if self.ray_day_cycle && !have_day && n.is_class("PixelDayNight") {
                if let Ok(t) = n.get("time_of_day").try_to::<f32>() {
                    let curve = stable3d_cloud_threshold(t);
                    threshold = curve + (curve.max(DAWN_DUSK_THRESHOLD) - curve) * self.ray_dawn_dusk.clamp(0.0, 1.0);
                    have_day = true;
                }
            }
            if !have_day && n.is_class("PixelSky") {
                if let Ok(c) = n.get("coverage").try_to::<f32>() {
                    threshold = 1.0 - c;
                }
            }
        }
        (tint, threshold)
    }

    fn sync(&mut self) {
        let Some((cam, screen)) = self.view_source() else {
            self.set_rendering(false);
            return;
        };
        if screen.x < 2.0 || screen.y < 2.0 {
            return;
        }
        self.set_rendering(true);
        let ps = self.pixel_size.clamp(1, 16) as f32;
        let mut h = if self.fixed_height > 0 { self.fixed_height } else { (screen.y / ps).ceil() as i32 };
        h = (h + (h & 1)).max(16);
        let view_texels = Vector2::new(screen.x * h as f32 / screen.y, h as f32);
        let mut w = view_texels.x.ceil() as i32;
        w = (w + (w & 1)).max(16);
        let res = Vector2i::new(w + 2, h + 2);
        let src = Vector2i::new(w + 2 + 2 * MARGIN, h + 2 + 2 * MARGIN);

        let ortho = cam.get_projection() == ProjectionType::ORTHOGONAL;
        let keep_width = cam.get_keep_aspect_mode() == KeepAspect::WIDTH;
        // Perspective cameras can opt into the same snapping with a `pixel_snap_distance` meta
        // (metres to their focus, e.g. a third-person camera's arm length): the lattice is the
        // texel size on that plane, so whatever lies near it stays locked while the camera moves.
        let snap_distance = if ortho || !cam.has_meta("pixel_snap_distance") {
            0.0
        } else {
            cam.get_meta("pixel_snap_distance").try_to::<f32>().unwrap_or(0.0)
        };
        let texel = if ortho {
            let size = cam.get_size();
            if keep_width { size / view_texels.x } else { size / view_texels.y }
        } else if snap_distance > 0.0 {
            let span = 2.0 * snap_distance * (cam.get_fov().to_radians() * 0.5).tan();
            if keep_width { span / view_texels.x } else { span / view_texels.y }
        } else {
            0.0
        };
        let snapping = ortho || snap_distance > 0.0;
        self.texel_world = texel;

        let t = cam.get_global_transform();
        let (right, up, back) = (t.basis.col_a().normalized(), t.basis.col_b().normalized(), t.basis.col_c().normalized());
        let mut origin = t.origin;
        let (mut sx, mut sy, mut fx, mut fy) = (0i64, 0i64, 0.0f32, 0.0f32);
        if snapping && self.snap_camera && texel > 0.0 {
            // Snap space (port of `Camera3DTexelSnapped3._process`): rotation changes the snap
            // space, so the lattice is re-anchored on the camera itself; positions are snapped
            // relative to that anchor until the camera rotates again.
            let basis = Basis::from_cols(right, up, back);
            if self.stabilize_rotation {
                let moved = |a: Vector3, b: Vector3| (a - b).length_squared() > 1e-12;
                let sb = self.snap_basis;
                if moved(sb.col_a(), right) || moved(sb.col_b(), up) || moved(sb.col_c(), back) {
                    self.snap_basis = basis;
                    self.snap_anchor = origin;
                }
            } else {
                self.snap_basis = basis;
                self.snap_anchor = Vector3::ZERO;
            }
            let rel = origin - self.snap_anchor;
            let x = rel.dot(right) / texel;
            let y = rel.dot(up) / texel;
            sx = x.round() as i64;
            sy = y.round() as i64;
            fx = x - sx as f32;
            fy = y - sy as f32;
            origin += right * ((sx as f32 - x) * texel) + up * ((sy as f32 - y) * texel);
        }
        let snapped = Transform3D::new(Basis::from_cols(right, up, back), origin);

        let meta_mask = globals::meta_mask();
        let id_mask = globals::id_mask();
        // Editor preview: the editor camera's mask also has the editor's own layers (bits 24+:
        // misc tools, grid / axis lines, gizmos). Leave them out, otherwise the grid and the axis
        // lines get pixelated into the preview image on top of the scene.
        let editor_layers = if self.pipeline.as_ref().is_some_and(|p| p.host.is_some()) { EDITOR_ONLY_LAYERS } else { 0 };
        let cull = self.saved_cull_mask & !meta_mask & !editor_layers;
        let anchor = self.mode == 0;
        let extended = self.extended_ids && anchor;
        let multiply = self.outline_palette.as_ref().is_some_and(|p| p.bind().is_multiply());
        self.push_object_defaults();
        let outline_tex = self.outline_texture();
        let env = cam.get_environment();
        let attrs = cam.get_attributes();
        let (near, far) = (cam.get_near(), cam.get_far());
        let fov = cam.get_fov();
        let size_src = texel * src.y as f32;
        let fov_src = if ortho {
            fov
        } else {
            let half = (fov.to_radians() * 0.5).tan() * src.y as f32 / view_texels.y;
            (half.atan() * 2.0).to_degrees().min(179.0)
        };

        let (ray_tint, ray_threshold) = if self.god_rays { self.ray_inputs() } else { (self.ray_color, 0.5) };
        // The sheets' cloud mask for other effects (PixelParticleField dust in the shafts).
        let rays_on = if self.god_rays { 1.0 } else { 0.0 };
        globals::set("pt_rays_a", &Vector4::new(rays_on, ray_threshold, self.ray_sheet_cloud_softness, self.ray_gap_erosion).to_variant());
        globals::set("pt_rays_b", &Vector4::new(self.ray_cloud_shadow_strength, self.ray_sun_follow, self.ray_strength, 0.0).to_variant());

        let Some(p) = &mut self.pipeline else { return };
        if self.low_res != res {
            self.low_res = res;
            p.resolve_vp.set_size(res);
            p.color_vp.set_size(src);
            p.meta_vp.set_size(src);
            p.id_vp.set_size(src);
            p.resolve_mat.set_shader_parameter("source_size", &src.to_variant());
            p.resolve_mat.set_shader_parameter("out_size", &res.to_variant());
            p.upscale_mat.set_shader_parameter("source_size", &Vector2::new(res.x as f32, res.y as f32).to_variant());
        }
        if p.rendering {
            let mode = if extended { UpdateMode::ALWAYS } else { UpdateMode::DISABLED };
            if p.id_vp.get_update_mode() != mode {
                p.id_vp.set_update_mode(mode);
            }
            let mode = if anchor { UpdateMode::ALWAYS } else { UpdateMode::DISABLED };
            if p.meta_vp.get_update_mode() != mode {
                p.meta_vp.set_update_mode(mode);
            }
        }
        for (cam3, kind) in [(&mut p.color_cam, 0), (&mut p.meta_cam, 1), (&mut p.id_cam, 2)] {
            cam3.set_global_transform(snapped);
            cam3.set_keep_aspect_mode(KeepAspect::HEIGHT);
            cam3.set_near(near);
            cam3.set_far(far);
            if ortho {
                cam3.set_projection(ProjectionType::ORTHOGONAL);
                cam3.set_size(size_src.max(0.001));
            } else {
                cam3.set_projection(ProjectionType::PERSPECTIVE);
                cam3.set_fov(fov_src);
            }
            if kind == 1 {
                cam3.set_cull_mask(meta_mask);
            } else if kind == 2 {
                cam3.set_cull_mask(id_mask);
            } else {
                cam3.set_cull_mask(cull);
                match &env {
                    Some(e) => cam3.set_environment(e),
                    None => cam3.set_environment(Gd::<Environment>::null_arg()),
                }
                match &attrs {
                    Some(a) => cam3.set_attributes(a),
                    None => cam3.set_attributes(Gd::<godot::classes::CameraAttributes>::null_arg()),
                }
            }
            cam3.set_current(true);
        }

        // World lattice of resolve texel (0, 0) (see pixel_resolve.gdshader), mod 24 =
        // lcm of the block sizes (1-4) and the Bayer sizes (4, 8).
        let lattice = if snapping && self.snap_camera {
            let lx = sx + MARGIN as i64 - (src.x / 2) as i64;
            let ly = sy + (src.y / 2) as i64 - MARGIN as i64 - 1;
            Vector2i::new(lx.rem_euclid(24) as i32, ly.rem_euclid(24) as i32)
        } else {
            Vector2i::ZERO
        };
        let shift = if self.smooth_scroll { Vector2::new(fx, -fy) } else { Vector2::ZERO };

        let m = &mut p.resolve_mat;
        m.set_shader_parameter("lattice", &lattice.to_variant());
        m.set_shader_parameter("max_pixel_size", &4.to_variant());
        m.set_shader_parameter("outlines", &self.outlines.to_variant());
        m.set_shader_parameter("outline_background", &self.outline_background.to_variant());
        m.set_shader_parameter("depth_threshold", &self.depth_threshold.to_variant());
        m.set_shader_parameter("normal_threshold", &self.normal_threshold.to_variant());
        m.set_shader_parameter("outline_darken", &self.outline_darken.to_variant());
        m.set_shader_parameter("highlight", &self.highlight.to_variant());
        m.set_shader_parameter("use_id_tex", &extended.to_variant());
        m.set_shader_parameter("low_res_mode", &(!anchor).to_variant());
        m.set_shader_parameter("outline_multiply", &multiply.to_variant());
        m.set_shader_parameter("id_edges", &self.id_edges.to_variant());
        m.set_shader_parameter("fold_limit", &self.fold_limit.to_variant());
        m.set_shader_parameter("ledge_wall", &self.ledge_walls.to_variant());
        if let Some(t) = &outline_tex {
            m.set_shader_parameter("outline_palette", &t.to_variant());
        }
        let lut = if self.palette_enabled { self.palette.as_mut().and_then(|pal| pal.bind_mut().lut_texture()) } else { None };
        m.set_shader_parameter("palette_enabled", &lut.is_some().to_variant());
        if let Some(l) = &lut {
            m.set_shader_parameter("palette_lut", &l.to_variant());
        }
        m.set_shader_parameter("palette_mix", &self.palette_mix.to_variant());
        m.set_shader_parameter("dither_enabled", &self.dither_enabled.to_variant());
        m.set_shader_parameter("dither_spread", &self.dither_spread.to_variant());
        m.set_shader_parameter("dither_size", &self.dither_size.to_variant());
        m.set_shader_parameter("saturation", &self.saturation.to_variant());
        m.set_shader_parameter("contrast", &self.contrast.to_variant());
        m.set_shader_parameter("brightness", &self.brightness.to_variant());
        m.set_shader_parameter("debug_view", &self.debug_view.to_variant());
        m.set_shader_parameter("rays_enabled", &self.god_rays.to_variant());
        if self.god_rays {
            m.set_shader_parameter("ray_color", &ray_tint.to_variant());
            m.set_shader_parameter("ray_strength", &(STABLE3D_RAY_STRENGTH * LDR_RAY_CALIBRATION * self.ray_strength).to_variant());
            m.set_shader_parameter("ray_samples", &self.ray_samples.to_variant());
            m.set_shader_parameter("ray_max_distance", &self.ray_max_distance.to_variant());
            m.set_shader_parameter("ray_bands", &self.ray_bands.to_variant());
            m.set_shader_parameter("ray_sheet_spacing", &self.ray_sheet_spacing.to_variant());
            m.set_shader_parameter("ray_sheet_quantum", &self.ray_sheet_quantum.to_variant());
            m.set_shader_parameter("ray_sheet_max_stack", &self.ray_sheet_max_stack.to_variant());
            m.set_shader_parameter("ray_sheet_distance_falloff", &self.ray_sheet_distance_falloff.to_variant());
            m.set_shader_parameter("ray_sheet_surface_fade", &self.ray_sheet_surface_fade.to_variant());
            m.set_shader_parameter("ray_sheet_cloud_softness", &self.ray_sheet_cloud_softness.to_variant());
            m.set_shader_parameter("ray_gap_erosion", &self.ray_gap_erosion.to_variant());
            m.set_shader_parameter("ray_cloud_shadow_strength", &self.ray_cloud_shadow_strength.to_variant());
            m.set_shader_parameter("ray_near_fade_start", &self.ray_near_fade_start.to_variant());
            m.set_shader_parameter("ray_near_fade", &self.ray_near_fade.to_variant());
            m.set_shader_parameter("ray_near_fade_power", &self.ray_near_fade_power.to_variant());
            m.set_shader_parameter("ray_depth_cutoff", &self.ray_depth_cutoff.to_variant());
            m.set_shader_parameter("ray_sun_follow", &self.ray_sun_follow.to_variant());
            m.set_shader_parameter("ray_adaptation", &self.ray_adaptation.to_variant());
            m.set_shader_parameter("ray_cloud_threshold", &ray_threshold.to_variant());
        }
        // The camera basis is also used by the outline pass (wall folds), so always set it.
        {
            let aspect = src.x as f32 / src.y as f32;
            let (r, u) = if ortho {
                let hh = size_src * 0.5;
                (right * (hh * aspect), up * hh)
            } else {
                let tv = (fov_src.to_radians() * 0.5).tan();
                (right * (tv * aspect), up * tv)
            };
            m.set_shader_parameter("cam_origin", &origin.to_variant());
            m.set_shader_parameter("cam_right", &r.to_variant());
            m.set_shader_parameter("cam_up", &u.to_variant());
            m.set_shader_parameter("cam_forward", &(-back).to_variant());
            m.set_shader_parameter("cam_ortho", &ortho.to_variant());
        }
        let u = &mut p.upscale_mat;
        u.set_shader_parameter("view_texels", &view_texels.to_variant());
        u.set_shader_parameter("center", &Vector2::new(res.x as f32 * 0.5, res.y as f32 * 0.5).to_variant());
        u.set_shader_parameter("shift", &shift.to_variant());
        u.set_shader_parameter("sharp", &self.sharp_upscale.to_variant());

        globals::set("pt_meta_depth_range", &self.depth_range.max(1.0).to_variant());
        globals::set("pt_texel_world", &texel.to_variant());
    }
}

#[godot_api]
impl PixelRenderer {
    #[func]
    fn set_enabled(&mut self, v: bool) {
        self.enabled = v;
        if is_editor() || !self.base().is_inside_tree() {
            return;
        }
        if v {
            self.build();
        } else {
            self.teardown();
        }
    }

    pub fn is_editor_preview(&self) -> bool {
        self.editor_preview
    }

    #[func]
    pub fn set_editor_preview(&mut self, v: bool) {
        self.editor_preview = v;
        // Turning on is picked up by `process` (building adds children; never do that from a
        // setter that the inspector/plugin may call mid-notification).
        if is_editor() && !v {
            self.teardown();
        }
    }

    #[func]
    fn set_meta_layer(&mut self, v: i32) {
        self.meta_layer = v.clamp(1, 20);
        if self.base().is_inside_tree() {
            globals::set_meta_layer(self.meta_layer as u32);
            let mut tree = self.base().get_tree();
            tree.call_group(globals::REFRESH_GROUP, "refresh_pixel_settings", &[]);
        }
    }

    /// Registers a runtime outline color for `id` (exterior and interior rows).
    #[func]
    fn set_outline_color(&mut self, id: i32, color: Color) {
        outline_palette::set_override(id.clamp(0, 255) as u32, color, color);
    }

    /// Removes a runtime outline color override.
    #[func]
    fn clear_outline_color(&mut self, id: i32) {
        outline_palette::clear_override(id.clamp(0, 255) as u32);
    }

    /// Highlights every pixelized mesh in `node`'s subtree: 0 none, 1 hover (id 254),
    /// 2 selection (id 255). Works on any node (PixelObject or plain geometry).
    #[func]
    fn highlight_node(&mut self, node: Gd<Node>, kind: i32) {
        object::highlight(&node, kind);
    }

    /// Current low-res resolution (including the 1-texel scroll margin).
    #[func]
    pub fn get_low_res_size(&self) -> Vector2i {
        self.low_res
    }

    /// World point the camera snap lattice is anchored at (moves only when the camera rotates,
    /// with `stabilize_rotation`; the world origin otherwise).
    #[func]
    pub fn get_snap_anchor(&self) -> Vector3 {
        self.snap_anchor
    }

    /// World size of one low-res texel (0 for perspective cameras).
    #[func]
    pub fn get_texel_world_size(&self) -> f32 {
        self.texel_world
    }

    /// The low-res color viewport (e.g. for screenshots/debugging).
    #[func]
    fn get_color_viewport(&self) -> Option<Gd<SubViewport>> {
        self.pipeline.as_ref().map(|p| p.color_vp.clone())
    }

    /// The resolved low-res image before upscaling.
    #[func]
    fn get_resolve_viewport(&self) -> Option<Gd<SubViewport>> {
        self.pipeline.as_ref().map(|p| p.resolve_vp.clone())
    }

    /// One-line status for overlays.
    #[func]
    pub fn get_status_line(&self) -> GString {
        let cam = self.bound_camera.as_ref().map(|c| if c.get_projection() == ProjectionType::ORTHOGONAL { "ortho" } else { "persp" }).unwrap_or("none");
        GString::from(
            format!(
                "pixel {}px | low-res {}x{} | texel {:.3} m | cam {} | outlines {} | ids {} | palette {} | rays {}",
                self.pixel_size,
                self.low_res.x,
                self.low_res.y,
                self.texel_world,
                cam,
                if self.outlines { "on" } else { "off" },
                if self.extended_ids { "0-255" } else { "0-7" },
                if self.palette_enabled { "on" } else { "off" },
                if self.god_rays { "on" } else { "off" },
            )
            .as_str(),
        )
    }
}
