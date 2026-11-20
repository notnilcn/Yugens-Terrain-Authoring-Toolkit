//! `PixelRenderer` — 3D pixel art for the Compatibility renderer.
//!
//! Pipeline (all plain SubViewports + canvas shaders, no compositor or compute):
//!
//! ```text
//! ResolveViewport (2D, low-res + 1 texel margin)          <- renders after its children
//!   ├─ ColorViewport  (3D, low-res + margins) Camera3D: scene color
//!   ├─ MetaViewport   (3D, same size)       Camera3D: meta layer only
//!   ├─ IdViewport     (3D, same size)       Camera3D: id pass (outline ids 0-255, palette/dither)
//!   └─ ColorRect  pixel_resolve.gdshader   (blocks, outlines, sun shafts, dither, palette)
//! CanvasLayer(-100) ColorRect pixel_upscale.gdshader   (sub-texel shift, crisp edges)
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
use godot::classes::scene_tree::GroupCallFlags;
use godot::classes::sub_viewport::UpdateMode;
use godot::classes::viewport::{Msaa, ScreenSpaceAa};
use godot::classes::{
    Camera3D, CanvasLayer, ColorRect, Environment, INode, ImageTexture, Node, ShaderMaterial,
    SubViewport,
};
use godot::prelude::*;

use super::editor_preview::EditorHost;
use super::object;
use super::outline_colors::{self, PixelOutlineColors};
use super::palette::PixelPalette;
use crate::features::{self, Feature};
use crate::util::*;
use crate::writes::{self, ShaderParams};
use crate::{clock, globals};

const MARGIN: i32 = 4;

/// Border (source texels on each side) the colour / meta / id passes render beyond the visible
/// low-res image: a texel of a k-sized object samples its block anchor up to k - 1 texels left /
/// down, and the outline / crease tests look two more texels further, so the border grows with
/// the largest block size in use (sizes are unbounded).
fn margin_for(blocks: &[i32]) -> i32 {
    blocks
        .iter()
        .copied()
        .max()
        .map_or(MARGIN, |k| MARGIN.max(k + 2))
}

/// Period of the resolve's dither lattice (`lattice`): a multiple of the Bayer sizes (4, 8).
/// Block sizes get their own phase (`block_phase_x/y` = world lattice mod k), so any number of
/// unbounded sizes stays world-anchored (an lcm of all of them overflowed for a few odd sizes).
const DITHER_PERIOD: i64 = 24;

/// Layers the 3D editor reserves for itself (Node3DEditor: MISC_TOOL 24, GIZMO_GRID 25,
/// GIZMO_EDIT 26, GIZMO_BASE 27 + viewport index); never drawn by the editor preview.
const EDITOR_ONLY_LAYERS: u32 = 0xff00_0000;
// The meta / id cameras must never see the editor's layers either (the origin axis lines sit on
// them and would be outlined in the preview): their masks are the meta layer (1–20) + the id bit.
const _: () = assert!(EDITOR_ONLY_LAYERS & (1 << pt_core::meta::ID_CAMERA_BIT) == 0);
/// Editor layers the editor camera keeps drawing under the preview: everything but the grid /
/// origin (25), i.e. the transform gizmo, node gizmos and the selection box. They render at full
/// resolution over a transparent background, on top of the pixel overlay.
const EDITOR_GIZMO_LAYERS: u32 = EDITOR_ONLY_LAYERS & !(1 << 25);

struct Pipeline {
    base: Gd<Node>,
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
    #[var(set = assign_active)]
    active: bool,
    /// Pipeline mode (Anchor = the full pipeline). Anchor: the full
    /// pipeline (meta + id passes; per-object pixel sizes, outlines, creases). Low Res:
    /// color pass only, upscaled (no meta passes, so no outlines or per-object blocks);
    /// dither, palette and grading still apply.
    #[export(enum = (Anchor = 0, LowRes = 1))]
    mode: i32,
    /// Screen pixels per low-res texel.
    #[export(range = (1.0, 16.0, 1.0, or_greater))]
    pixel_size: i32,
    /// Camera to follow (empty = the viewport's current Camera3D, re-resolved every frame).
    #[export]
    #[var(get = get_camera)]
    camera: Option<Gd<Camera3D>>,
    /// Editor only: show the pixelated image in the editor's 3D viewport, following the editor
    /// camera (also toggled by the "Pixel Preview" button in the 3D viewport toolbar).
    #[export]
    #[var(set = assign_studio_prelook)]
    studio_prelook: bool,

    // Basics above stay ungrouped; the rest sits in collapsed groups (display-only: saved
    // property names never change).
    #[export_group(name = "Camera")]
    /// Fixed low-res height in texels (0 = derive from `pixel_size`).
    #[export(range = (0.0, 1080.0, 1.0))]
    fixed_height: i32,
    /// Snap orthographic cameras to the texel lattice (stable pixels), as in
    /// 3d-pixel-art-in-godot's texel-snapped camera. Perspective cameras are
    /// snapped too when they carry a `pixel_snap_distance` meta (the lattice on that plane).
    #[export]
    texel_snap: bool,
    /// Apply the snapped-away remainder in the upscale (sub-texel scrolling).
    #[export]
    subtexel_scroll: bool,
    /// Anti-aliased texel edges for non-integer scales (`pixel_upscale.gdshader`).
    #[export]
    crisp_edges: bool,
    /// Re-anchor the snap lattice on the camera whenever it rotates (snap space of
    /// 3d-pixel-art-in-godot's `Camera3DTexelSnapped3`): an orbiting camera keeps its subject at
    /// the same sub-texel phase instead of jumping up to half a texel per frame, and translation
    /// stays world-locked between rotations. Off = lattice anchored at the world origin.
    #[export_subgroup(name = "Stabilize", prefix = "stabilize_")]
    #[export]
    stabilize_rotation: bool,
    #[export_subgroup(name = "Focus")]
    /// Keep `focus_target`'s subtree highlighted with the selection outline (id 255), so the
    /// subject always reads (port of the original `auto_focus`). Runtime only.
    #[export]
    auto_focus: bool,
    #[export]
    #[var(get = get_focus_target)]
    focus_target: Option<Gd<Node3D>>,

    #[export_group(name = "Outlines")]
    #[export]
    outlines: bool,
    /// Draw silhouettes against the sky/background too.
    #[export_subgroup(name = "Outline", prefix = "outline_")]
    #[export]
    outline_background: bool,
    /// Depth step (m) that counts as a silhouette edge.
    #[export(range = (0.0105, 10.0, 0.0105))]
    depth_threshold: f32,
    /// Crease angle: convex edges (ridges) inside one object whose normals differ by more than
    /// this (1 - cos(angle); 0.3005 ~ 46 deg, 0.5 = 60 deg, 0.6505 ~ 70 deg) get the crease
    /// highlight. Lower = more creases. Default ~0.5, the convex normal threshold of
    /// 3d-pixel-art-in-godot's `outline_and_edge.gdshader` (CC0). Valleys and the shallow facet
    /// edges of curved low-poly meshes never qualify; `highlight` 0 turns creases off.
    #[export(range = (0.0, 2.0, 0.0105))]
    normal_threshold: f32,
    /// How much silhouette pixels darken their own color (for outline ids without a color).
    #[export(range = (0.0, 1.0, 0.0105))]
    outline_darken: f32,
    /// How much crease pixels brighten.
    #[export(range = (0.0, 1.0, 0.0105))]
    highlight: f32,
    /// Legacy outline tints for ids 0-7 (used where the `outline_colors` resource has no color).
    /// Alpha > 0 tints the outline towards the color.
    #[export]
    outline_tints: PackedColorArray,
    /// Per-id exterior/interior outline colors (ids 0-253; 254 hover, 255 selection).
    #[export]
    outline_colors: Option<Gd<PixelOutlineColors>>,
    /// Draw the palette's interior row where surfaces with different outline ids touch.
    #[export]
    id_edges: bool,
    /// Depth steps (m) below this between parallel vertical faces of the same object are wall
    /// folds (hex cliff corners), not silhouettes. 0 = outline every depth step.
    #[export(range = (0.0, 20.0, 0.1004))]
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
    #[export_subgroup(name = "Object Defaults")]
    /// Pixel size of PixelObjects whose chain is all "inherit" (-1).
    #[export(range = (1.0, 16.0, 1.0, or_greater))]
    object_pixel_size: i32,
    /// First outline id handed out by automatic allocation (PixelObject `outline_id` -2, or
    /// a top-level PixelObject left at inherit). Ids cycle through `auto_id_first`..=253.
    #[export(range = (1.0, 253.0, 1.0))]
    auto_id_first: i32,
    /// Outline of PixelObjects whose chain is all "inherit".
    #[export]
    object_outline: bool,

    #[export_group(name = "Color")]
    #[export]
    palette: Option<Gd<PixelPalette>>,
    #[export]
    palette_enabled: bool,
    #[export(range = (0.0, 1.0, 0.0105))]
    palette_mix: f32,
    #[export]
    dither_enabled: bool,
    #[export(range = (0.0, 0.5, 0.0015))]
    dither_spread: f32,
    #[export(enum = (Bayer4 = 4, Bayer8 = 8))]
    dither_size: i32,
    #[export(range = (0.0, 2.0, 0.0105))]
    saturation: f32,
    #[export(range = (0.0, 2.0, 0.0105))]
    contrast: f32,
    #[export(range = (-1.0, 1.0, 0.0105))]
    brightness: f32,

    #[export_group(name = "God Rays")]
    /// World-anchored sun shafts after robert-k's `godrays/godray.gdshader` (MIT): sheets
    /// containing the sun beam axis stack up the sunlight let through the PixelSky cloud gaps
    /// (peak brightness `ray_alpha` per sheet, faded by `start_depth` / `depth_fade`).
    #[export]
    god_rays: bool,
    /// `ray_color`: the tint of one sheet (robert-k's `ray_color`, used unless `sun_color`).
    #[export_subgroup(name = "Ray", prefix = "ray_")]
    #[export]
    beam_tint: Color,
    /// Tint the shafts with the sun's hue instead of `ray_color`.
    #[export]
    sun_color: bool,
    /// Alpha of one fully lit sheet: 1 = the calibrated default look (visible, subtle shafts; a
    /// light haze at noon). robert-k's `godray.gdshader` adds `ray_color * alpha` per sheet.
    #[export(range = (0.0, 4.0, 0.0105, or_greater))]
    ray_alpha: f32,
    /// `count`: the sheets a view ray may cross (robert-k's `count`).
    #[export(range = (2.0, 52.0, 1.0))]
    ray_count: i32,
    /// `depth`: how far a view ray gathers sheets (m, robert-k's `depth`).
    #[export]
    ray_depth: f32,
    /// No sheets beyond this distance (m; 0 = off).
    #[export]
    ray_reach: f32,
    /// Extra quantisation of the sheet stack into this many bands (1 = off).
    #[export(range = (1.0, 16.0, 1.0))]
    sheet_bands: i32,
    /// `sheet_gap` (m): distance between the world-anchored sheets.
    #[export_subgroup(name = "Sheets", prefix = "sheet_")]
    #[export(range = (0.0505, 50.0, 0.0105, or_greater))]
    sheet_gap: f32,
    /// Light added per fully lit sheet (`min(stack * sheet_energy, sheet_stack_max)`).
    #[export(range = (0.0, 10.0, 0.0105, or_greater))]
    sheet_energy: f32,
    /// Cap of the stacked light.
    #[export(range = (0.0, 10.0, 0.0105, or_greater))]
    sheet_stack_max: f32,
    /// Exponential fade with distance from the camera (1/m).
    #[export(range = (0.0, 1.0, 0.0015))]
    sheet_falloff: f32,
    /// `depth_fade` (m): the last sheets fade out over this distance before the surface.
    #[export(range = (0.0, 1000.0, 0.1004, or_greater))]
    sheet_depth_fade: f32,
    /// Edge softness of the cloud gaps seen by the sheets.
    #[export_subgroup(name = "Clouds", prefix = "cloud_")]
    #[export(range = (0.0, 0.5, 0.0004))]
    cloud_softness: f32,
    /// How much a cloud blocks a sheet (robert-k's cloud shadow term).
    #[export(range = (0.0, 1.0, 0.0105))]
    cloud_occlusion: f32,
    /// Shrinks the lit gaps (0 = whole gaps).
    #[export(range = (0.0, 0.9905, 0.0105))]
    gap_erode: f32,
    /// 1 = the beams follow the sun direction, 0 = vertical.
    #[export_subgroup(name = "Sun", prefix = "sun_")]
    #[export(range = (0.0, 1.0, 0.0105))]
    sun_follow: f32,
    /// Drive the sheets' cloud threshold over the day (overcast at night, clearing towards noon)
    /// from the scene's PixelDayNight `time_of_day`. Off = 1 - PixelSky coverage (gaps match
    /// the ground cloud shadows).
    #[export]
    day_cycle: bool,
    /// Dawn / dusk shafts with `day_cycle`: the day curve is overcast around sunrise and sunset
    /// (no shafts then) and only partly open in the morning / afternoon. This keeps the sheets'
    /// cloud gaps at least this far towards the curve's mid-morning opening (0 = the curve as
    /// is, 1 = open like mid-morning all day), so a low sun still pours long shafts. Night is
    /// unaffected (rays fade with `pt_night`).
    #[export(range = (0.0, 1.0, 0.0105))]
    dawn_dusk: f32,
    /// `start_depth` (m): no sheets closer to the camera than this (robert-k's `start_depth`).
    #[export_subgroup(name = "Depth Fade", prefix = "depth_")]
    #[export(range = (0.0, 100.0, 0.1004, or_greater))]
    start_depth: f32,
    /// `depth_fade` (m): the sheets fade in over this range after `start_depth`.
    #[export(range = (0.0, 100.0, 0.1004, or_greater))]
    depth_fade: f32,
    /// Curve of that fade-in.
    #[export(range = (0.0505, 8.0, 0.0105))]
    depth_fade_power: f32,
    /// LDR eye adaptation of the sheet stack (0 = off, raw stack). A view ray that crosses the
    /// beams side-on passes many sheets, so the stack sits at the cap over the whole frame: there
    /// the HDR add is pulled back by exposure, here it would be a flat, milky veil. The stack is
    /// scaled by min(1, eye_adaptation / potential), where potential is the stack the ray would
    /// gather if every sheet it crosses were lit, so views along / against the sun (few sheets,
    /// distinct shafts) are unchanged and side-on views keep the shafts at a comparable strength
    /// instead of washing out.
    #[export(range = (0.0, 10.0, 0.0105, or_greater))]
    eye_adaptation: f32,

    #[export_group(name = "Advanced")]
    /// Render layer reserved for the meta camera (pixelized geometry is put on it).
    #[export(range = (1.0, 20.0, 1.0))]
    #[var(set = set_meta_layer)]
    meta_layer: i32,
    /// View depth (m) covered by the meta depth encoding.
    #[export(range = (1.0, 4000.0, 1.0, or_greater))]
    depth_range: f32,
    #[export(enum = (Final, Normals, Depth, PixelSize, OutlineIds, RawColor, OutlineCause, Shafts))]
    debug_view: i32,

    pipeline: Option<Pipeline>,
    outline_tex: Option<Gd<ImageTexture>>,
    outline_key: (u64, i64, Vec<[u32; 4]>),
    bound_camera: Option<Gd<Camera3D>>,
    saved_cull_mask: u32,
    /// Editor preview: the editor viewport's own `transparent_bg` while we force it on.
    saved_transparent_bg: Option<bool>,
    low_res: Vector2i,
    /// Source border of the current passes ([`margin_for`]).
    margin: i32,
    texel_world: f32,
    /// Snap space: camera basis the lattice was last anchored for, and the anchor (world).
    lattice_basis: Basis,
    lattice_anchor: Vector3,
    /// Subtree currently highlighted by auto focus.
    focused: Option<Gd<Node>>,
    focus_frames: u32,
    /// The sun for `sun_color`: the first sibling DirectionalLight3D (cached).
    sun_ref: SiblingFallback<godot::classes::DirectionalLight3D>,
    base: Base<Node>,
}

/// Per-sheet tint of the shafts: robert-k's `godray.gdshader` default `alpha` (0.0205).
const SHEET_ALPHA: f32 = 0.0204;

/// LDR calibration of `SHEET_ALPHA`. robert-k adds its sheets to a lit HDR scene from an
/// orthographic camera standing far back, so a stack of sheets piles up in the cloud gaps; here
/// the shafts go into the finished LDR colour (as a linear-light screen, see
/// `pixel_resolve.gdshader`) from cameras a few tens of metres away, where the depth fade leaves
/// the stack at roughly a quarter to a half of the cap. x4 brings the shafts back to robert-k's
/// subtle look (judged on the hex island overview / player view at day, afternoon and noon), so
/// `ray_alpha = 1` means "robert-k default".
const LDR_ALPHA_CALIBRATION: f32 = 4.0505;

/// Default `eye_adaptation`: the potential sheet stack (in `sheet_energy` units) up to which the
/// shafts are added unscaled. Tuned on the hex island player view (yaw sweep 0..300 at
/// time 0.4005) so the side-on views keep about the frame contrast of the along-the-sun ones.
const EYE_ADAPTATION: f32 = 1.6004;

/// The shared "cloud threshold" over one day (0 = midnight, 0.5 = noon), cubic keys with flat
/// tangents: overcast at night, clearing towards noon. Higher = fewer clouds.
const DAY_THRESHOLD_KEYS: [(f32, f32); 7] = [
    (0.0, 0.1004),
    (0.2294, 0.1004),
    (0.3754, 0.4704),
    (0.5, 0.6704),
    (0.6254, 0.4704),
    (0.7704, 0.1004),
    (1.0, 0.1004),
];

/// `dawn_dusk` = 1 holds the sheets' cloud threshold at the day curve's mid-morning key.
const DAWN_DUSK_THRESHOLD: f32 = 0.4704;

/// Cloud threshold of the day curve at `time_of_day` (wraps).
pub fn day_cloud_threshold(time_of_day: f32) -> f32 {
    let t = time_of_day.rem_euclid(1.0);
    for w in DAY_THRESHOLD_KEYS.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t <= t1 {
            let x = ((t - t0) / (t1 - t0).max(1e-6)).clamp(0.0, 1.0);
            return v0 + (v1 - v0) * x * x * (3.0 - 2.0 * x);
        }
    }
    DAY_THRESHOLD_KEYS[6].1
}

#[cfg(test)]
mod tests {
    use super::day_cloud_threshold as f;

    #[test]
    fn day_cloud_curve() {
        assert!((f(0.5) - 0.6704).abs() < 1e-5);
        assert!((f(0.1) - 0.1004).abs() < 1e-5);
        assert!((f(0.3755) - 0.4704).abs() < 1e-5);
        assert!((f(1.5) - 0.6704).abs() < 1e-5);
        let d = f(0.7);
        assert!(d > 0.1004 && d < 0.4704);
    }
}

#[godot_api]
impl INode for PixelRenderer {
    fn on_validate_property(&self, property: &mut godot::register::info::PropertyInfo) {
        crate::util::space_enum_hint(property);
    }

    fn init(base: Base<Node>) -> Self {
        let mut outline_tints = PackedColorArray::new();
        for _ in 0..8 {
            outline_tints.push(Color::from_rgba(0.0, 0.0, 0.0, 0.0));
        }
        PixelRenderer {
            active: true,
            mode: 0,
            studio_prelook: false,
            pixel_size: 4,
            fixed_height: 0,
            camera: None,
            texel_snap: true,
            subtexel_scroll: true,
            stabilize_rotation: true,
            crisp_edges: true,
            meta_layer: globals::DEFAULT_META_LAYER as i32,
            depth_range: 200.0,
            outlines: true,
            outline_background: true,
            depth_threshold: 0.3505,
            normal_threshold: 0.5005,
            outline_darken: 0.55,
            highlight: 0.22,
            outline_tints,
            outline_colors: None,
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
            sun_ref: SiblingFallback::default(),
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
            beam_tint: Color::from_rgb(1.0, 0.9845, 0.2665),
            sun_color: false,
            ray_alpha: 1.0,
            ray_count: 26,
            ray_depth: 305.0,
            ray_reach: 283.3905,
            sheet_bands: 1,
            sheet_gap: 2.2305,
            sheet_energy: 2.6605,
            sheet_stack_max: 3.0005,
            sheet_falloff: 0.0,
            sheet_depth_fade: 327.8805,
            cloud_softness: 0.0015,
            cloud_occlusion: 0.9005,
            gap_erode: 0.4005,
            sun_follow: 0.9975,
            day_cycle: true,
            dawn_dusk: 0.0,
            eye_adaptation: EYE_ADAPTATION,
            start_depth: 5.9005,
            depth_fade: 8.5905,
            depth_fade_power: 1.3505,
            debug_view: 0,
            pipeline: None,
            outline_tex: None,
            outline_key: (0, 0, Vec::new()),
            bound_camera: None,
            saved_cull_mask: u32::MAX,
            saved_transparent_bg: None,
            low_res: Vector2i::ZERO,
            margin: MARGIN,
            texel_world: 0.0,
            lattice_basis: Basis::IDENTITY,
            lattice_anchor: Vector3::ZERO,
            base,
        }
    }

    fn ready(&mut self) {
        globals::ensure();
        globals::set_meta_layer(self.meta_layer as u32);
        features::observe(&self.to_gd());
        self.push_object_defaults();
        self.base_mut().set_process_priority(10_000);
        self.base_mut().set_process(true);
        if !is_editor() && self.active {
            self.assemble();
        }
    }

    fn process(&mut self, delta: f64) {
        clock::tick(delta);
        if is_editor() {
            // Built lazily (also after the scene tab is switched back): never from a setter.
            let host_lost = self
                .pipeline
                .as_ref()
                .is_some_and(|p| p.host.as_ref().is_some_and(|h| !h.check_sane()));
            if host_lost {
                self.dismantle();
            }
            if self.studio_prelook && self.pipeline.is_none() {
                self.assemble();
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
        globals::assign("pt_rays_a", &Vector4::ZERO.to_variant());
        self.sun_ref.revert();
        self.dismantle();
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        if self.base().get_parent().is_some_and(|p| {
            p.find_children_ex("*")
                .type_("PixelRenderer")
                .owned(false)
                .done()
                .len()
                > 1
        }) {
            w.push("Remove the extra PixelRenderer: only one per scene is supported.");
        }
        let gated = [
            (self.god_rays, Feature::GodRays),
            (self.outlines, Feature::Outlines),
            (self.extended_ids, Feature::OutlineIds),
        ];
        for (on, f) in gated {
            if on && let Some(msg) = features::warning(f) {
                w.push(&msg.replace("this node draws nothing", "this renderer skips it"));
            }
        }
        w
    }
}

impl PixelRenderer {
    /// `god_rays` unless the project switched the God Rays feature off.
    fn rays_on(&self) -> bool {
        self.god_rays && features::active(Feature::GodRays)
    }

    fn outlines_on(&self) -> bool {
        self.outlines && features::active(Feature::Outlines)
    }

    /// Whether the id pass renders: Anchor mode and either `extended_ids` with the Outline Ids
    /// feature, or a per-object pixel size above 4 (the meta pass only carries sizes 1-4).
    fn id_pass_on(&self) -> bool {
        self.mode == 0
            && ((self.extended_ids && features::active(Feature::OutlineIds))
                || self.object_pixel_size > 4
                || object::block_sizes().last().is_some_and(|&k| k > 4))
    }

    fn assemble(&mut self) {
        if self.pipeline.is_some() {
            return;
        }
        let host = if is_editor() {
            match EditorHost::locate() {
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
        resolve_vp.set_default_canvas_item_texture_filter(
            godot::classes::viewport::DefaultCanvasItemTextureFilter::NEAREST,
        );

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
                // Behind the editor's 3D image (made transparent; it only draws the gizmo
                // layers, see `view_source`) and its overlay controls (gizmo axes, info);
                // mouse-transparent so editing works as usual.
                display_rect.set_name("PixelPreviewOverlay");
                display_rect.set_draw_behind_parent(true);
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
            base: root,
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
        if object::set_defaults(self.object_pixel_size, self.object_outline)
            && self.base().is_inside_tree()
        {
            let mut tree = self.base().get_tree();
            tree.call_group_flags(
                GroupCallFlags::DEFERRED,
                globals::REFRESH_GROUP,
                "refresh_pixel_settings",
                &[],
            );
        }
    }

    /// The (cached) 256 x 2 outline palette texture: palette resource, legacy colors, overrides.
    fn outline_texture(&mut self) -> Option<Gd<ImageTexture>> {
        let legacy: Vec<[u32; 4]> = self
            .outline_tints
            .as_slice()
            .iter()
            .map(|c| [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()])
            .collect();
        let pal_id = self
            .outline_colors
            .as_ref()
            .map(|p| p.instance_id().to_i64())
            .unwrap_or(0);
        let key = (outline_colors::version(), pal_id, legacy);
        if self.outline_tex.is_some() && self.outline_key == key {
            return self.outline_tex.clone();
        }
        let (mut ext, mut int) = match &self.outline_colors {
            Some(p) => p.bind().row_colors(),
            None => outline_colors::fallback_rank_tints_vector(),
        };
        for (i, c) in self.outline_tints.as_slice().iter().enumerate().take(8) {
            if c.a > 0.0 && ext[i].a == 0.0 {
                ext[i] = *c;
            }
            if c.a > 0.0 && int[i].a == 0.0 {
                int[i] = *c;
            }
        }
        let img = outline_colors::bake_rows_image(&ext, &int, &outline_colors::overrides())?;
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

    fn dismantle(&mut self) {
        self.release_camera();
        if let Some(p) = self.pipeline.take() {
            let editor = p.host.is_some();
            if let (Some(h), Some(was)) = (&p.host, self.saved_transparent_bg.take())
                && h.check_sane()
            {
                let mut vp = h.viewport.clone();
                vp.set_transparent_background(was);
            }
            if editor {
                // The overlay lives in the editor's tree, not under our pipeline root.
                free_node(Some(p.display_rect));
                globals::assign("pt_texel_world", &0.0f32.to_variant());
            }
            free_node(Some(p.base));
            if editor {
                self.notify_editor_preview(false);
            }
        }
    }

    /// Keeps the auto-focus target highlighted (re-applied every 30 frames so runtime-spawned
    /// children are covered); switching targets restores the previous one.
    fn update_focus(&mut self) {
        let want: Option<Gd<Node>> = if self.auto_focus {
            self.focus_target
                .clone()
                .filter(|t| t.is_instance_valid())
                .map(|t| t.upcast())
        } else {
            None
        };
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
        if object::assign_studio_prelook(on) && self.base().is_inside_tree() {
            let mut tree = self.base().get_tree();
            tree.call_group_flags(
                GroupCallFlags::DEFERRED,
                globals::REFRESH_GROUP,
                "refresh_pixel_settings",
                &[],
            );
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
        let (anchor, ids) = (self.mode == 0, self.id_pass_on());
        let Some(p) = &mut self.pipeline else { return };
        if p.rendering == on {
            return;
        }
        p.rendering = on;
        let mode = if on {
            UpdateMode::ALWAYS
        } else {
            UpdateMode::DISABLED
        };
        p.resolve_vp.set_update_mode(mode);
        p.color_vp.set_update_mode(mode);
        p.meta_vp.set_update_mode(if on && anchor {
            UpdateMode::ALWAYS
        } else {
            UpdateMode::DISABLED
        });
        p.id_vp.set_update_mode(if on && ids {
            UpdateMode::ALWAYS
        } else {
            UpdateMode::DISABLED
        });
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
            let mut vp = h.viewport.clone();
            // The editor re-applies its own cull mask (e.g. "View Gizmos"); re-capture it and keep
            // only its gizmo layers on the editor camera: the scene is drawn by the overlay behind
            // the (transparent) editor image, so the transform gizmo, node gizmos and selection
            // box still show at full resolution on top of it.
            if self.bound_camera.as_ref() != Some(&cam) {
                self.release_camera();
                self.bound_camera = Some(cam.clone());
            }
            let mask = cam.get_cull_mask();
            if mask & !EDITOR_ONLY_LAYERS != 0 || mask & EDITOR_GIZMO_LAYERS != mask {
                self.saved_cull_mask = mask;
            }
            let want = self.saved_cull_mask & EDITOR_GIZMO_LAYERS;
            if mask != want {
                let mut c = cam.clone();
                c.set_cull_mask(want);
            }
            if self.saved_transparent_bg.is_none() {
                self.saved_transparent_bg = Some(vp.has_transparent_background());
            }
            if !vp.has_transparent_background() {
                vp.set_transparent_background(true);
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

    /// Shaft tint (`ray_color`, or the sun's hue with `sun_color`) and the sheets'
    /// cloud threshold (day curve from the scene's PixelDayNight, or 1 - the PixelSky
    /// coverage), read from what those nodes publish.
    fn shaft_inputs(&mut self) -> (Color, f32) {
        let mut tint = self.beam_tint;
        if self.sun_color {
            let owner = self.to_gd().upcast::<Node>();
            if let Some(l) = self.sun_ref.settle(&owner, &None) {
                // Sun illuminance divided by its largest channel.
                let c = l.get_color();
                let peak = c.r.max(c.g).max(c.b).max(1e-4);
                tint = Color::from_rgb(c.r / peak, c.g / peak, c.b / peak);
            }
        }
        let threshold = match (
            self.day_cycle,
            crate::effects::day_night::time_of_day(),
            crate::effects::cloud_field::sky(),
        ) {
            (true, Some(t), _) => {
                let curve = day_cloud_threshold(t);
                curve + (curve.max(DAWN_DUSK_THRESHOLD) - curve) * self.dawn_dusk.clamp(0.0, 1.0)
            }
            (_, _, Some(sky)) => 1.0 - sky.coverage,
            _ => 0.5005,
        };
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
        // Pixel sizes are unbounded: a size bigger than the screen still gives a 2-texel image.
        let ps = self.pixel_size.max(1) as f32;
        let mut h = if self.fixed_height > 0 {
            self.fixed_height
        } else {
            (screen.y / ps).ceil() as i32
        };
        h = (h + (h & 1)).max(2);
        let view_texels = Vector2::new(screen.x * h as f32 / screen.y, h as f32);
        let mut w = view_texels.x.ceil() as i32;
        w = (w + (w & 1)).max(2);
        let blocks = object::block_sizes();
        let margin = margin_for(&blocks);
        let res = Vector2i::new(w + 2, h + 2);
        let src = Vector2i::new(w + 2 + 2 * margin, h + 2 + 2 * margin);

        let ortho = cam.get_projection() == ProjectionType::ORTHOGONAL;
        let keep_width = cam.get_keep_aspect_mode() == KeepAspect::WIDTH;
        // Perspective cameras can opt into the same snapping with a `pixel_snap_distance` meta
        // (metres to their focus, e.g. a third-person camera's arm length): the lattice is the
        // texel size on that plane, so whatever lies near it stays locked while the camera moves.
        let snap_distance = if ortho || !cam.has_meta("pixel_snap_distance") {
            0.0
        } else {
            cam.get_meta("pixel_snap_distance")
                .try_to::<f32>()
                .unwrap_or(0.0)
        };
        let texel = if ortho {
            let size = cam.get_size();
            if keep_width {
                size / view_texels.x
            } else {
                size / view_texels.y
            }
        } else if snap_distance > 0.0 {
            let span = 2.0 * snap_distance * (cam.get_fov().to_radians() * 0.5).tan();
            if keep_width {
                span / view_texels.x
            } else {
                span / view_texels.y
            }
        } else {
            0.0
        };
        let snapping = ortho || snap_distance > 0.0;
        self.texel_world = texel;

        let t = cam.get_global_transform();
        let (right, up, back) = (
            t.basis.col_a().normalized(),
            t.basis.col_b().normalized(),
            t.basis.col_c().normalized(),
        );
        let mut origin = t.origin;
        let (mut sx, mut sy, mut fx, mut fy) = (0i64, 0i64, 0.0f32, 0.0f32);
        if snapping && self.texel_snap && texel > 0.0 {
            // Snap space (after 3d-pixel-art-in-godot's `Camera3DTexelSnapped3._process`): rotation
            // changes the snap space, so the lattice is re-anchored on the camera itself;
            // positions are snapped relative to that anchor until the camera rotates again.
            let basis = Basis::from_cols(right, up, back);
            if self.stabilize_rotation {
                let moved = |a: Vector3, b: Vector3| (a - b).length_squared() > 1e-12;
                let sb = self.lattice_basis;
                if moved(sb.col_a(), right) || moved(sb.col_b(), up) || moved(sb.col_c(), back) {
                    self.lattice_basis = basis;
                    self.lattice_anchor = origin;
                }
            } else {
                self.lattice_basis = basis;
                self.lattice_anchor = Vector3::ZERO;
            }
            let rel = origin - self.lattice_anchor;
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
        let editor_layers = if self.pipeline.as_ref().is_some_and(|p| p.host.is_some()) {
            EDITOR_ONLY_LAYERS
        } else {
            0
        };
        let cull = self.saved_cull_mask & !meta_mask & !editor_layers;
        let anchor = self.mode == 0;
        let extended = self.id_pass_on();
        let god_rays = self.rays_on();
        let outlines = self.outlines_on();
        let modulate = self
            .outline_colors
            .as_ref()
            .is_some_and(|p| p.bind().is_modulate());
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

        let (ray_tint, ray_threshold) = if god_rays {
            self.shaft_inputs()
        } else {
            (self.beam_tint, 0.5005)
        };
        // The sheets' cloud mask for other effects (PixelParticleField dust in the shafts).
        let rays_on = if god_rays { 1.0 } else { 0.0 };
        globals::assign(
            "pt_rays_a",
            &Vector4::new(rays_on, ray_threshold, self.cloud_softness, self.gap_erode).to_variant(),
        );
        globals::assign(
            "pt_rays_b",
            &Vector4::new(self.cloud_occlusion, self.sun_follow, self.ray_alpha, 0.0).to_variant(),
        );

        let Some(p) = &mut self.pipeline else { return };
        if self.low_res != res || self.margin != margin {
            self.low_res = res;
            self.margin = margin;
            p.resolve_mat
                .param("margin", &Vector2i::new(margin, margin).to_variant());
            p.resolve_vp.set_size(res);
            p.color_vp.set_size(src);
            p.meta_vp.set_size(src);
            p.id_vp.set_size(src);
            p.resolve_mat.param("source_size", &src.to_variant());
            p.resolve_mat.param("out_size", &res.to_variant());
            p.upscale_mat.param(
                "source_size",
                &Vector2::new(res.x as f32, res.y as f32).to_variant(),
            );
        }
        if p.rendering {
            let mode = if extended {
                UpdateMode::ALWAYS
            } else {
                UpdateMode::DISABLED
            };
            if p.id_vp.get_update_mode() != mode {
                p.id_vp.set_update_mode(mode);
            }
            let mode = if anchor {
                UpdateMode::ALWAYS
            } else {
                UpdateMode::DISABLED
            };
            if p.meta_vp.get_update_mode() != mode {
                p.meta_vp.set_update_mode(mode);
            }
        }
        let view = CameraView {
            warp: snapped,
            near,
            far,
            ortho,
            size: size_src.max(0.001),
            fov: fov_src,
        };
        view.enact(&mut p.color_cam, cull);
        view.enact(&mut p.meta_cam, meta_mask);
        view.enact(&mut p.id_cam, id_mask);
        if p.color_cam.get_environment() != env {
            match &env {
                Some(e) => p.color_cam.set_environment(e),
                None => p.color_cam.set_environment(Gd::<Environment>::null_arg()),
            }
        }
        if p.color_cam.get_attributes() != attrs {
            match &attrs {
                Some(a) => p.color_cam.set_attributes(a),
                None => p
                    .color_cam
                    .set_attributes(Gd::<godot::classes::CameraAttributes>::null_arg()),
            }
        }

        // World lattice of resolve texel (0, 0) (see pixel_resolve.gdshader): mod the dither
        // period for the Bayer pattern, and mod each block size for its block phase.
        let (lx, ly) = if snapping && self.texel_snap {
            (
                sx + margin as i64 - (src.x / 2) as i64,
                sy + (src.y / 2) as i64 - margin as i64 - 1,
            )
        } else {
            (0, 0)
        };
        let lattice = Vector2i::new(
            lx.rem_euclid(DITHER_PERIOD) as i32,
            ly.rem_euclid(DITHER_PERIOD) as i32,
        );
        let shift = if self.subtexel_scroll {
            Vector2::new(fx, -fy)
        } else {
            Vector2::ZERO
        };

        let m = &mut p.resolve_mat;
        m.param("lattice", &lattice.to_variant());
        let mut sizes = [0i32; object::MAX_BLOCK_SIZES];
        let (mut phase_x, mut phase_y) = (sizes, sizes);
        for (i, k) in blocks.iter().take(object::MAX_BLOCK_SIZES).enumerate() {
            sizes[i] = *k;
            phase_x[i] = lx.rem_euclid(*k as i64) as i32;
            phase_y[i] = ly.rem_euclid(*k as i64) as i32;
        }
        m.param(
            "block_sizes",
            &PackedInt32Array::from(&sizes[..]).to_variant(),
        );
        m.param(
            "block_phase_x",
            &PackedInt32Array::from(&phase_x[..]).to_variant(),
        );
        m.param(
            "block_phase_y",
            &PackedInt32Array::from(&phase_y[..]).to_variant(),
        );
        m.param(
            "block_size_count",
            &(blocks.len().min(object::MAX_BLOCK_SIZES) as i32).to_variant(),
        );
        m.param("outlines", &outlines.to_variant());
        m.param("outline_background", &self.outline_background.to_variant());
        m.param("depth_threshold", &self.depth_threshold.to_variant());
        m.param("normal_threshold", &self.normal_threshold.to_variant());
        m.param("outline_darken", &self.outline_darken.to_variant());
        m.param("highlight", &self.highlight.to_variant());
        m.param("use_id_tex", &extended.to_variant());
        m.param("low_res_mode", &(!anchor).to_variant());
        m.param("modulate_rows", &modulate.to_variant());
        m.param("id_edges", &self.id_edges.to_variant());
        m.param("fold_limit", &self.fold_limit.to_variant());
        m.param("ledge_wall", &self.ledge_walls.to_variant());
        if let Some(t) = &outline_tex {
            m.param("outline_rows", &t.to_variant());
        }
        let lut = if self.palette_enabled {
            self.palette
                .as_mut()
                .and_then(|pal| pal.bind_mut().lut_texture())
        } else {
            None
        };
        m.param("palette_enabled", &lut.is_some().to_variant());
        if let Some(l) = &lut {
            m.param("palette_lut", &l.to_variant());
        }
        m.param("palette_mix", &self.palette_mix.to_variant());
        m.param("dither_enabled", &self.dither_enabled.to_variant());
        m.param("dither_spread", &self.dither_spread.to_variant());
        m.param("dither_size", &self.dither_size.to_variant());
        m.param("saturation", &self.saturation.to_variant());
        m.param("contrast", &self.contrast.to_variant());
        m.param("brightness", &self.brightness.to_variant());
        m.param("debug_view", &self.debug_view.to_variant());
        m.param("rays_enabled", &god_rays.to_variant());
        if god_rays {
            m.param("ray_color", &ray_tint.to_variant());
            m.param(
                "ray_alpha",
                &(SHEET_ALPHA * LDR_ALPHA_CALIBRATION * self.ray_alpha).to_variant(),
            );
            m.param("ray_count", &self.ray_count.to_variant());
            m.param("ray_depth", &self.ray_depth.to_variant());
            m.param("ray_reach", &self.ray_reach.to_variant());
            m.param("sheet_bands", &self.sheet_bands.to_variant());
            m.param("sheet_gap", &self.sheet_gap.to_variant());
            m.param("sheet_energy", &self.sheet_energy.to_variant());
            m.param("sheet_stack_max", &self.sheet_stack_max.to_variant());
            m.param("sheet_falloff", &self.sheet_falloff.to_variant());
            m.param("sheet_depth_fade", &self.sheet_depth_fade.to_variant());
            m.param("cloud_softness", &self.cloud_softness.to_variant());
            m.param("cloud_occlusion", &self.cloud_occlusion.to_variant());
            m.param("gap_erode", &self.gap_erode.to_variant());
            m.param("start_depth", &self.start_depth.to_variant());
            m.param("depth_fade", &self.depth_fade.to_variant());
            m.param("depth_fade_power", &self.depth_fade_power.to_variant());
            m.param("sun_follow", &self.sun_follow.to_variant());
            m.param("eye_adaptation", &self.eye_adaptation.to_variant());
            m.param("cloud_threshold", &ray_threshold.to_variant());
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
            m.param("cam_origin", &origin.to_variant());
            m.param("cam_right", &r.to_variant());
            m.param("cam_up", &u.to_variant());
            m.param("cam_forward", &(-back).to_variant());
            m.param("cam_ortho", &ortho.to_variant());
        }
        let u = &mut p.upscale_mat;
        u.param("view_texels", &view_texels.to_variant());
        u.param(
            "center",
            &Vector2::new(res.x as f32 * 0.5, res.y as f32 * 0.5).to_variant(),
        );
        u.param("shift", &shift.to_variant());
        u.param("crisp_edges", &self.crisp_edges.to_variant());

        globals::assign(
            "pt_meta_depth_range",
            &self.depth_range.max(1.0).to_variant(),
        );
        globals::assign("pt_texel_world", &texel.to_variant());
    }
}

#[godot_api]
impl PixelRenderer {
    /// `camera`, or `None` once that node was freed (`util::live`).
    #[func]
    fn get_camera(&self) -> Option<Gd<Camera3D>> {
        crate::util::live(&self.camera)
    }

    /// `focus_target`, or `None` once that node was freed (`util::live`).
    #[func]
    fn get_focus_target(&self) -> Option<Gd<Node3D>> {
        crate::util::live(&self.focus_target)
    }

    /// A project feature toggle changed (`features::CALLBACK`): the passes follow on the next
    /// sync; only the warnings need a refresh.
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        self.base_mut().update_configuration_warnings();
    }

    #[func]
    fn assign_active(&mut self, v: bool) {
        self.active = v;
        if is_editor() || !self.base().is_inside_tree() {
            return;
        }
        if v {
            self.assemble();
        } else {
            self.dismantle();
        }
    }

    pub fn is_editor_preview(&self) -> bool {
        self.studio_prelook
    }

    #[func]
    pub fn assign_studio_prelook(&mut self, v: bool) {
        self.studio_prelook = v;
        // Turning on is picked up by `process` (building adds children; never do that from a
        // setter that the inspector/plugin may call mid-notification).
        if is_editor() && !v {
            self.dismantle();
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
    fn assign_contour_tint(&mut self, token_id: i32, tint: Color) {
        outline_colors::set_override(token_id.clamp(0, 255) as u32, tint, tint);
    }

    /// Removes a runtime outline color override.
    #[func]
    fn clear_outline_color(&mut self, token_id: i32) {
        outline_colors::clear_override(token_id.clamp(0, 255) as u32);
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

    /// World point the camera texel lattice is anchored at (moves only when the camera rotates,
    /// with `stabilize_rotation`; the world origin otherwise).
    #[func]
    pub fn get_lattice_anchor(&self) -> Vector3 {
        self.lattice_anchor
    }

    /// World size of one low-res texel (0 for perspective cameras).
    #[func]
    pub fn fetch_texture_px_universe_measure(&self) -> f32 {
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
    pub fn fetch_situation_streak(&self) -> GString {
        let cam = self
            .bound_camera
            .as_ref()
            .map(|c| {
                if c.get_projection() == ProjectionType::ORTHOGONAL {
                    "ortho"
                } else {
                    "persp"
                }
            })
            .unwrap_or("none");
        GString::from(
            format!(
                "pixel {}px | low-res {}x{} | texel {:.3} m | cam {} | outlines {} | ids {} | palette {} | rays {}",
                self.pixel_size,
                self.low_res.x,
                self.low_res.y,
                self.texel_world,
                cam,
                if self.outlines_on() { "on" } else { "off" },
                if self.id_pass_on() { "0-255" } else { "0-7" },
                if self.palette_enabled { "on" } else { "off" },
                if self.rays_on() { "on" } else { "off" },
            )
            .as_str(),
        )
    }
}

/// What the three pipeline cameras copy from the followed camera each frame.
struct CameraView {
    warp: Transform3D,
    near: f32,
    far: f32,
    ortho: bool,
    size: f32,
    fov: f32,
}

impl CameraView {
    /// Change-only: an unchanged camera sends nothing to the RenderingServer, so a still
    /// editor preview lets the editor idle.
    fn enact(&self, cam: &mut Gd<Camera3D>, cull_mask: u32) {
        writes::set_global_transform(cam, self.warp);
        if cam.get_keep_aspect_mode() != KeepAspect::HEIGHT {
            cam.set_keep_aspect_mode(KeepAspect::HEIGHT);
        }
        if cam.get_near() != self.near {
            cam.set_near(self.near);
        }
        if cam.get_far() != self.far {
            cam.set_far(self.far);
        }
        let projection = if self.ortho {
            ProjectionType::ORTHOGONAL
        } else {
            ProjectionType::PERSPECTIVE
        };
        if cam.get_projection() != projection {
            cam.set_projection(projection);
        }
        if self.ortho {
            if cam.get_size() != self.size {
                cam.set_size(self.size);
            }
        } else if cam.get_fov() != self.fov {
            cam.set_fov(self.fov);
        }
        if cam.get_cull_mask() != cull_mask {
            cam.set_cull_mask(cull_mask);
        }
        if !cam.is_current() {
            cam.set_current(true);
        }
    }
}
