//! The camera that turns a 3D scene into chunky, outlined pixel art.
//!
//! Intent: a level author drops one of these next to their camera and the whole scene, in
//! the game and optionally in the editor viewport, is shown as low-resolution pixel art with
//! outlines, a limited palette, dithering and light shafts. Everything about that look is
//! chosen here, in the Inspector, without touching the scene's own camera or materials.
//!
//! Responsibility: it owns the look of the final picture and the off-screen passes that
//! produce it, follows whichever camera is active, keeps the picture steady while that
//! camera moves, and publishes the shared values every pixel-aware shader reads. It does
//! not decide how individual objects are pixelized; those choices live on the objects
//! themselves, and it only supplies the defaults they fall back to. It does not move the
//! camera, and it borrows the sun and the time of day from whatever owns them.
//!
//! Why a node: it has to follow the active camera every frame, be told when it enters and
//! leaves a scene, run late enough in the frame to see where the camera ended up, and own a
//! branch of the tree for its off-screen passes. Being a node is also how an author says
//! "this scene is pixel art".
//!
//! Why a new type: the built-in viewport and camera nodes can render at low resolution, but
//! nothing built in snaps the view to a texel grid, classifies edges for outlines or applies
//! a palette, and asking every scene author to rebuild that pipeline by hand would make the
//! look impossible to keep consistent.
//!
//! Nodes created in code: the off-screen pipeline is instantiated from a scene while pixel
//! output is wanted and freed when it stops, because whether it exists depends on play, on
//! an editor preview switch and on the editor's own viewport, none of which is known while
//! authoring. In the editor its final picture is placed inside the editor's viewport, which
//! no scene can declare. The pipeline scene is a part the author may swap for their own;
//! left empty, the built-in one is used.
//!
//! Not exported: the pipeline it is running and the camera it has taken over, which exist
//! only while it renders; the size, scroll and grid position it derives from the camera
//! each frame; caches built from its exported colors; and the node it is at the moment
//! highlighting for focus, which only means something while the game runs. Also kept out
//! are facts its passes are only correct for: the smallest border rendered around the view
//! so edge texels have the neighbours the outline and block passes read, and the period the
//! dither lattice wraps at so it lines up with every dither matrix the shader offers. The
//! fixed scale that gives the exported light-shaft strength its unit, matching one light
//! sheet to the low-dynamic-range buffer it lands in, is kept out too: a control for it
//! would only repeat that strength.
use super::editor_preview::EditorHost;
use super::object;
use super::outline_colors::{self, PixelOutlineColors};
use super::palette::PixelPalette;
use super::pipeline::PipelineParts;
use crate::editor_detection::is_editor;
use crate::features::{self, Feature};
use crate::node_children::{add_internal, free_node};
use crate::node_references::SiblingFallback;
use godot::classes::camera_3d::{KeepAspect, ProjectionType};
use godot::classes::scene_tree::GroupCallFlags;
use godot::classes::sub_viewport::UpdateMode;
use godot::classes::{
    Camera3D, ColorRect, Environment, INode, ImageTexture, Node, PackedScene, ShaderMaterial,
    SubViewport,
};
use godot::prelude::*;

use crate::writes::{self, ShaderParams};
use crate::{clock, globals};
const MARGIN: i32 = 4;
fn margin_for(blocks: &[i32]) -> i32 {
    blocks
        .iter()
        .copied()
        .max()
        .map_or(MARGIN, |k| MARGIN.max(k + 2))
}
const DITHER_PERIOD: i64 = 24;
const EDITOR_ONLY_LAYERS: u32 = 0xff00_0000;
const _: () = assert!(EDITOR_ONLY_LAYERS & (1 << pt_core::meta::ID_CAMERA_BIT) == 0);
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
    display_rect: Gd<ColorRect>,
    host: Option<EditorHost>,
    rendering: bool,
}
#[derive(GodotClass)]
#[class(tool, base = Node)]
pub struct PixelRenderer {
    #[export]
    #[var(set = assign_active)]
    active: bool,
    #[export(enum = (Anchor = 0, LowRes = 1))]
    mode: i32,
    #[export(range = (1.0, 16.0, 1.0, or_greater))]
    pixel_size: i32,
    #[export]
    #[var(get = get_camera)]
    camera: Option<Gd<Camera3D>>,
    #[export]
    #[var(set = assign_studio_prelook)]
    studio_prelook: bool,
    #[export_group(name = "Camera")]
    #[export(range = (0.0, 1080.0, 1.0))]
    fixed_height: i32,
    #[export]
    texel_snap: bool,
    #[export]
    subtexel_scroll: bool,
    #[export]
    crisp_edges: bool,
    #[export_subgroup(name = "Stabilize", prefix = "stabilize_")]
    #[export]
    stabilize_rotation: bool,
    #[export_subgroup(name = "Focus")]
    #[export]
    auto_focus: bool,
    #[export]
    #[var(get = get_focus_target)]
    focus_target: Option<Gd<Node3D>>,
    #[export_group(name = "Outlines")]
    #[export]
    outlines: bool,
    #[export_subgroup(name = "Outline", prefix = "outline_")]
    #[export]
    outline_background: bool,
    #[export(range = (0.0105, 10.0, 0.0105))]
    depth_threshold: f32,
    #[export(range = (0.0, 2.0, 0.0105))]
    normal_threshold: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    outline_darken: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    highlight: f32,
    #[export]
    outline_tints: PackedColorArray,
    #[export]
    outline_colors: Option<Gd<PixelOutlineColors>>,
    #[export]
    id_edges: bool,
    #[export(range = (0.0, 20.0, 0.1004))]
    fold_limit: f32,
    #[export]
    ledge_walls: bool,
    #[export]
    extended_ids: bool,
    #[export_subgroup(name = "Object Defaults")]
    #[export(range = (1.0, 16.0, 1.0, or_greater))]
    object_pixel_size: i32,
    #[export(range = (1.0, 253.0, 1.0))]
    auto_id_first: i32,
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
    #[export]
    god_rays: bool,
    #[export_subgroup(name = "Ray", prefix = "ray_")]
    #[export]
    beam_tint: Color,
    #[export]
    sun_color: bool,
    #[export(range = (0.0, 4.0, 0.0105, or_greater))]
    ray_alpha: f32,
    #[export(range = (2.0, 52.0, 1.0))]
    ray_count: i32,
    #[export]
    ray_depth: f32,
    #[export]
    ray_reach: f32,
    #[export(range = (1.0, 16.0, 1.0))]
    sheet_bands: i32,
    #[export_subgroup(name = "Sheets", prefix = "sheet_")]
    #[export(range = (0.0505, 50.0, 0.0105, or_greater))]
    sheet_gap: f32,
    #[export(range = (0.0, 10.0, 0.0105, or_greater))]
    sheet_energy: f32,
    #[export(range = (0.0, 10.0, 0.0105, or_greater))]
    sheet_stack_max: f32,
    #[export(range = (0.0, 1.0, 0.0015))]
    sheet_falloff: f32,
    #[export(range = (0.0, 1000.0, 0.1004, or_greater))]
    sheet_depth_fade: f32,
    #[export_subgroup(name = "Clouds", prefix = "cloud_")]
    #[export(range = (0.0, 0.5, 0.0004))]
    cloud_softness: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    cloud_occlusion: f32,
    #[export(range = (0.0, 0.9905, 0.0105))]
    gap_erode: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    cloud_threshold_default: f32,
    #[export_subgroup(name = "Sun", prefix = "sun_")]
    #[export(range = (0.0, 1.0, 0.0105))]
    sun_follow: f32,
    #[export]
    day_cycle: bool,
    #[export(range = (0.0, 1.0, 0.0105))]
    dawn_dusk: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    dawn_dusk_threshold: f32,
    #[export_subgroup(name = "Depth Fade", prefix = "depth_")]
    #[export(range = (0.0, 100.0, 0.1004, or_greater))]
    start_depth: f32,
    #[export(range = (0.0, 100.0, 0.1004, or_greater))]
    depth_fade: f32,
    #[export(range = (0.0505, 8.0, 0.0105))]
    depth_fade_power: f32,
    #[export(range = (0.0, 10.0, 0.0105, or_greater))]
    eye_adaptation: f32,
    #[export_group(name = "Advanced")]
    #[export(range = (1.0, 20.0, 1.0))]
    #[var(set = set_meta_layer)]
    meta_layer: i32,
    #[export(range = (1.0, 4000.0, 1.0, or_greater))]
    depth_range: f32,
    #[export(enum = (Final, Normals, Depth, PixelSize, OutlineIds, RawColor, OutlineCause, Shafts))]
    debug_view: i32,
    #[export]
    #[var(set = assign_pipeline_scene)]
    pipeline_scene: Option<Gd<PackedScene>>,
    pipeline: Option<Pipeline>,
    outline_tex: Option<Gd<ImageTexture>>,
    outline_key: (u64, i64, Vec<[u32; 4]>),
    bound_camera: Option<Gd<Camera3D>>,
    saved_cull_mask: u32,
    saved_transparent_bg: Option<bool>,
    low_res: Vector2i,
    margin: i32,
    texel_world: f32,
    lattice_basis: Basis,
    lattice_anchor: Vector3,
    focused: Option<Gd<Node>>,
    focus_frames: u32,
    sun_ref: SiblingFallback<godot::classes::DirectionalLight3D>,
    base: Base<Node>,
}
const SHEET_ALPHA: f32 = 0.0204;
const LDR_ALPHA_CALIBRATION: f32 = 4.0505;
const EYE_ADAPTATION: f32 = 1.6004;
const DAY_THRESHOLD_KEYS: [(f32, f32); 7] = [
    (0.0, 0.1004),
    (0.2294, 0.1004),
    (0.3754, 0.4704),
    (0.5, 0.6704),
    (0.6254, 0.4704),
    (0.7704, 0.1004),
    (1.0, 0.1004),
];
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
        crate::property_hints::space_enum_hint(property);
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
            cloud_threshold_default: 0.5005,
            sun_follow: 0.9975,
            day_cycle: true,
            dawn_dusk: 0.0,
            dawn_dusk_threshold: 0.4704,
            eye_adaptation: EYE_ADAPTATION,
            start_depth: 5.9005,
            depth_fade: 8.5905,
            depth_fade_power: 1.3505,
            debug_view: 0,
            pipeline_scene: None,
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
    fn rays_on(&self) -> bool {
        self.god_rays && features::active(Feature::GodRays)
    }
    fn outlines_on(&self) -> bool {
        self.outlines && features::active(Feature::Outlines)
    }
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
        let Some(parts) = PipelineParts::instantiate(self.pipeline_scene.as_ref()) else {
            return;
        };
        let PipelineParts {
            mut root,
            resolve_vp,
            mut color_vp,
            mut meta_vp,
            mut id_vp,
            color_cam,
            meta_cam,
            id_cam,
            mut resolve_mat,
            mut upscale_mat,
            mut display,
            mut display_rect,
        } = parts;
        if let Some(h) = &host
            && let Some(world) = h.viewport.find_world_3d()
        {
            color_vp.set_world_3d(&world);
            meta_vp.set_world_3d(&world);
            id_vp.set_world_3d(&world);
        }
        resolve_mat.set_shader_parameter("color_tex", &color_vp.get_texture().to_variant());
        resolve_mat.set_shader_parameter("meta_tex", &meta_vp.get_texture().to_variant());
        resolve_mat.set_shader_parameter("id_tex", &id_vp.get_texture().to_variant());
        resolve_mat.set_shader_parameter("margin", &Vector2i::new(MARGIN, MARGIN).to_variant());
        upscale_mat.set_shader_parameter("source", &resolve_vp.get_texture().to_variant());
        if let Some(h) = &host {
            display.remove_child(&display_rect);
            root.remove_child(&display);
            display.free();
            display_rect.set_owner(Gd::<Node>::null_arg());
            display_rect.set_name("PixelPreviewOverlay");
            display_rect.set_draw_behind_parent(true);
            let mut c = h.container.clone();
            c.add_child(&display_rect);
        }
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
            display_rect,
            host,
            rendering: true,
        });
        self.low_res = Vector2i::ZERO;
        if is_editor() {
            self.notify_editor_preview(true);
        }
    }
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
                free_node(Some(p.display_rect));
                globals::assign("pt_texel_world", &0.0f32.to_variant());
            }
            free_node(Some(p.base));
            if editor {
                self.notify_editor_preview(false);
            }
        }
    }
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
    fn view_source(&mut self) -> Option<(Gd<Camera3D>, Vector2)> {
        let host = self.pipeline.as_ref().and_then(|p| p.host.as_ref());
        if let Some(h) = host {
            if !h.is_shown() {
                return None;
            }
            let (cam, size) = (h.camera()?, h.screen_size());
            let mut vp = h.viewport.clone();
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
    fn shaft_inputs(&mut self) -> (Color, f32) {
        let mut tint = self.beam_tint;
        if self.sun_color {
            let owner = self.to_gd().upcast::<Node>();
            if let Some(l) = self.sun_ref.settle(&owner, &None) {
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
                curve
                    + (curve.max(self.dawn_dusk_threshold) - curve) * self.dawn_dusk.clamp(0.0, 1.0)
            }
            (_, _, Some(sky)) => 1.0 - sky.coverage,
            _ => self.cloud_threshold_default,
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
            (self.beam_tint, self.cloud_threshold_default)
        };
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
    #[func]
    fn get_camera(&self) -> Option<Gd<Camera3D>> {
        crate::node_references::live(&self.camera)
    }
    #[func]
    fn get_focus_target(&self) -> Option<Gd<Node3D>> {
        crate::node_references::live(&self.focus_target)
    }
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
    #[func]
    fn assign_pipeline_scene(&mut self, v: Option<Gd<PackedScene>>) {
        self.pipeline_scene = v;
        if self.pipeline.is_none() {
            return;
        }
        self.dismantle();
        if !is_editor() && self.active && self.base().is_inside_tree() {
            self.assemble();
        }
    }
    pub fn is_editor_preview(&self) -> bool {
        self.studio_prelook
    }
    #[func]
    pub fn assign_studio_prelook(&mut self, v: bool) {
        self.studio_prelook = v;
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
    #[func]
    fn assign_contour_tint(&mut self, token_id: i32, tint: Color) {
        outline_colors::set_override(token_id.clamp(0, 255) as u32, tint, tint);
    }
    #[func]
    fn clear_outline_color(&mut self, token_id: i32) {
        outline_colors::clear_override(token_id.clamp(0, 255) as u32);
    }
    #[func]
    fn highlight_node(&mut self, node: Gd<Node>, kind: i32) {
        object::highlight(&node, kind);
    }
    #[func]
    pub fn get_low_res_size(&self) -> Vector2i {
        self.low_res
    }
    #[func]
    pub fn get_lattice_anchor(&self) -> Vector3 {
        self.lattice_anchor
    }
    #[func]
    pub fn fetch_texture_px_universe_measure(&self) -> f32 {
        self.texel_world
    }
    #[func]
    fn get_color_viewport(&self) -> Option<Gd<SubViewport>> {
        self.pipeline.as_ref().map(|p| p.color_vp.clone())
    }
    #[func]
    fn get_resolve_viewport(&self) -> Option<Gd<SubViewport>> {
        self.pipeline.as_ref().map(|p| p.resolve_vp.clone())
    }
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
struct CameraView {
    warp: Transform3D,
    near: f32,
    far: f32,
    ortho: bool,
    size: f32,
    fov: f32,
}
impl CameraView {
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
