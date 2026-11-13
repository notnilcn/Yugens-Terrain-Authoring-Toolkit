//! `PixelParticleField` — "particle effects without particles": a prerendered flipbook of
//! 1-texel particles (`pt_core::flipbook`, tiles in space and loops in time) drawn on a few
//! stacked, world-anchored sheets (`particle_field.gdshader`) instead of simulating particles.
//!
//! The reference technique scrolls one screen-space texture with the camera, which is only
//! right for an orthographic camera. Here every sheet is a real plane in the world (horizontal
//! falling sheets, or vertical sheets facing the camera's yaw), so a perspective camera — even
//! a near-orthographic low-FOV one — sees each layer with its own parallax and texel scale, and
//! translation / orbit / zoom keep the particles where they are. [`ParticleLayers`] is the
//! drawing half, shared with `PixelWeather`'s rain.

use godot::classes::camera_3d::ProjectionType;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::image::Format;
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::{ArrayMesh, Camera3D, INode3D, Image, ImageTexture, MeshInstance3D, Node, Node3D, ShaderMaterial};
use godot::prelude::*;
use pt_core::flipbook::{self, Preset, Shape};

use super::{active_camera, camera_focus, find_terrain, ground_height, weather};
use crate::globals;
use crate::util::*;

/// Sheet orientation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Projection {
    /// Horizontal sheets that fall (or rise) through a stack; fully world-fixed.
    Ground = 0,
    /// Vertical sheets facing the camera's yaw; world-vertical streaks (rain).
    Upright = 1,
}

/// Everything the layer shader needs besides the sheet.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub projection: Projection,
    pub layers: i32,
    pub spacing: f32,
    pub texel: f32,
    /// Half-size of each sheet; 0 = from the camera's view.
    pub extent: f32,
    pub base_y: f32,
    pub height: f32,
    pub fall_speed: f32,
    pub wind: Vector2,
    pub slant: f32,
    /// Ground: push by the travelling gusts of `pt_wind` (m).
    pub gust: f32,
    pub band: f32,
    pub fps: f32,
    pub color: Color,
    pub color_b: Color,
    pub brightness: f32,
    pub amount: f32,
    pub lit: f32,
    pub beam: f32,
    pub shade: f32,
    pub night: f32,
    /// Particles only within this horizontal distance (m) of `fade_center` (0 = everywhere);
    /// the sheets then shrink to that disc.
    pub focus_radius: f32,
    /// Centre of the `focus_radius` disc (where the camera's view meets the ground).
    pub fade_center: Vector3,
}

/// The drawing half: one internal MeshInstance3D with `layers` quads and the baked sheet.
#[derive(Default)]
pub struct ParticleLayers {
    mi: Option<Gd<MeshInstance3D>>,
    mat: Option<Gd<ShaderMaterial>>,
    tex: Option<Gd<ImageTexture>>,
    baked: Option<flipbook::Params>,
    mesh_layers: i32,
}

/// Quads of every layer: (x, y) in [-1, 1], z = layer index.
fn layer_mesh(n: i32) -> Gd<ArrayMesh> {
    let mut pos = PackedVector3Array::new();
    let mut idx = PackedInt32Array::new();
    for i in 0..n.max(1) {
        let z = i as f32;
        let b = (i * 4) as i32;
        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            pos.push(Vector3::new(x, y, z));
        }
        for k in [0, 1, 2, 0, 2, 3] {
            idx.push(b + k);
        }
    }
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
    arrays.set(ArrayType::INDEX.ord() as usize, &idx.to_variant());
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
    mesh
}

/// Uploads a baked sheet as a texture.
pub fn sheet_texture(b: &flipbook::Flipbook) -> Option<Gd<ImageTexture>> {
    let img = Image::create_from_data(b.width() as i32, b.height() as i32, false, Format::RGBA8, &PackedByteArray::from(b.rgba.as_slice()))?;
    ImageTexture::create_from_image(&img)
}

impl ParticleLayers {
    /// Creates the internal mesh instance under `parent` (idempotent).
    pub fn attach(&mut self, parent: &mut Gd<Node>, name: &str) {
        if self.mi.as_ref().is_some_and(|m| m.is_instance_valid()) {
            return;
        }
        let mut mi = MeshInstance3D::new_alloc();
        mi.set_name(name);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        // Color pass only: no outlines, never hides terrain edges.
        let layers = mi.get_layer_mask() & !globals::meta_mask();
        mi.set_layer_mask(layers);
        mi.set_as_top_level(true);
        mi.set_global_transform(Transform3D::IDENTITY);
        mi.set_ignore_occlusion_culling(true);
        // The vertex shader places the sheets: one huge box keeps them from being culled.
        mi.set_custom_aabb(Aabb::new(Vector3::splat(-1.0e5), Vector3::splat(2.0e5)));
        let mat = self.mat.get_or_insert_with(|| shader_material("particle_field.gdshader")).clone();
        mi.set_material_override(&mat);
        mi.set_visible(false);
        add_internal(parent, &mi);
        self.mi = Some(mi);
        self.mesh_layers = 0;
    }

    pub fn detach(&mut self) {
        free_node(self.mi.take());
        self.mesh_layers = 0;
    }

    /// Bakes (when the parameters changed) and binds the sheet.
    pub fn set_flipbook(&mut self, p: &flipbook::Params) {
        if self.baked.as_ref() == Some(p) && self.tex.is_some() {
            return;
        }
        let b = flipbook::bake(p);
        self.tex = sheet_texture(&b);
        self.baked = Some(*p);
        let mat = self.mat.get_or_insert_with(|| shader_material("particle_field.gdshader"));
        if let Some(t) = &self.tex {
            mat.set_shader_parameter("sheet", &t.to_variant());
        }
        mat.set_shader_parameter("sheet_grid", &Vector2i::new(b.cols as i32, b.rows as i32).to_variant());
        mat.set_shader_parameter("tile", &(b.size as i32).to_variant());
        mat.set_shader_parameter("frames", &(b.frames as i32).to_variant());
    }

    pub fn texture(&self) -> Option<Gd<ImageTexture>> {
        self.tex.clone()
    }

    pub fn set_visible(&mut self, on: bool) {
        if let Some(mi) = &mut self.mi
            && mi.is_instance_valid()
            && mi.is_visible() != on
        {
            mi.set_visible(on);
        }
    }

    /// Centres the sheets on the camera focus and pushes the look; hides them at amount 0.
    pub fn update(&mut self, node: &Gd<Node>, look: &Look) {
        let Some(mut mi) = self.mi.clone().filter(|m| m.is_instance_valid()) else { return };
        let on = look.amount > 0.001 && look.layers > 0;
        self.set_visible(on);
        if !on {
            return;
        }
        let n = look.layers.clamp(1, 64);
        if self.mesh_layers != n {
            mi.set_mesh(&layer_mesh(n));
            self.mesh_layers = n;
        }
        let cam = active_camera(node);
        let plane_y = match look.projection {
            Projection::Ground => look.base_y,
            Projection::Upright => look.base_y + 2.0,
        };
        let focus = cam.as_ref().map(|c| camera_focus(c, plane_y)).unwrap_or(Vector3::new(0.0, plane_y, 0.0));
        let mut extent = if look.extent > 0.0 { look.extent } else { cam.as_ref().map(auto_extent).unwrap_or(40.0) };
        let mut focus = focus;
        if look.focus_radius > 0.0 {
            // Only the disc around the focus can hold particles: no fill outside it.
            extent = extent.min(look.focus_radius * 1.1 + 1.0);
            focus = Vector3::new(look.fade_center.x, focus.y, look.fade_center.z);
        }
        let Some(m) = &mut self.mat else { return };
        let set = |m: &mut Gd<ShaderMaterial>, k: &str, v: Variant| m.set_shader_parameter(k, &v);
        set(m, "mode", (look.projection as i32).to_variant());
        set(m, "layers", n.to_variant());
        set(m, "spacing", look.spacing.max(0.01).to_variant());
        set(m, "texel_size", look.texel.max(0.001).to_variant());
        set(m, "extent", extent.to_variant());
        set(m, "base_y", look.base_y.to_variant());
        set(m, "height", look.height.max(0.1).to_variant());
        set(m, "focus", focus.to_variant());
        set(m, "fall_speed", look.fall_speed.to_variant());
        set(m, "wind", look.wind.to_variant());
        set(m, "slant", look.slant.to_variant());
        set(m, "gust", look.gust.to_variant());
        set(m, "band", look.band.max(0.0).to_variant());
        set(m, "fps", look.fps.max(0.0).to_variant());
        set(m, "color", look.color.to_variant());
        set(m, "color_b", look.color_b.to_variant());
        set(m, "brightness", look.brightness.to_variant());
        set(m, "amount", look.amount.clamp(0.0, 1.0).to_variant());
        set(m, "lit", look.lit.clamp(0.0, 1.0).to_variant());
        set(m, "beam", look.beam.clamp(0.0, 1.0).to_variant());
        set(m, "shade", look.shade.max(0.0).to_variant());
        set(m, "night", look.night.clamp(-1.0, 1.0).to_variant());
        set(m, "focus_radius", look.focus_radius.max(0.0).to_variant());
        set(m, "fade_center", look.fade_center.to_variant());
    }
}

/// Half-size that covers the camera's view around its focus (with margin for the pitch).
fn auto_extent(cam: &Gd<Camera3D>) -> f32 {
    let aspect = cam.get_viewport().map(|v| v.get_visible_rect().size).map(|s| s.x / s.y.max(1.0)).unwrap_or(16.0 / 9.0).max(1.0);
    let e = if cam.get_projection() == ProjectionType::ORTHOGONAL {
        cam.get_size() * 0.5 * aspect * 1.6
    } else {
        let t = cam.get_global_transform();
        let fwd = -t.basis.col_c();
        // Distance to the focus at the camera's own height minus 20 m, roughly the view depth.
        let d = if fwd.y < -0.05 { (20.0 / -fwd.y).max(10.0) + t.origin.y.abs() / -fwd.y } else { 60.0 };
        d * (cam.get_fov().to_radians() * 0.5).tan() * aspect * 1.8 + 4.0
    };
    e.clamp(4.0, 250.0)
}

// ---------------------------------------------------------------------------------------------

/// Preset look (sheet orientation and masks) to go with each flipbook preset.
struct PresetLook {
    projection: Projection,
    layers: i32,
    spacing: f32,
    texel: f32,
    height: f32,
    band: f32,
    fall_speed: f32,
    wind: Vector2,
    fps: f32,
    color: Color,
    color_b: Color,
    brightness: f32,
    lit: f32,
    beam: f32,
    shade: f32,
    night: f32,
    weather: i32,
    density: f32,
    focus_radius: f32,
}

fn preset_look(p: Preset) -> PresetLook {
    let rgb = Color::from_rgb;
    match p {
        // Sparse motes drifting in the sunbeams near where the camera looks: a few sheets in a
        // low band above the ground (never over water), only inside the god-ray shafts.
        Preset::DustMotes => PresetLook {
            projection: Projection::Ground,
            layers: 20,
            spacing: 0.8,
            texel: 0.09,
            height: 16.0,
            band: 3.0,
            fall_speed: 0.04,
            wind: Vector2::new(0.05, 0.02),
            fps: 12.0,
            color: rgb(1.0, 0.95, 0.72),
            color_b: rgb(1.0, 0.86, 0.55),
            brightness: 0.9,
            lit: 1.0,
            beam: 1.0,
            shade: 0.0,
            night: 1.0,
            weather: 1,
            density: 0.12,
            focus_radius: 11.0,
        },
        Preset::RainStreaks => PresetLook {
            projection: Projection::Upright,
            layers: 8,
            spacing: 2.5,
            texel: 0.08,
            height: 30.0,
            band: 0.0,
            fall_speed: 0.0,
            wind: Vector2::ZERO,
            fps: 28.0,
            color: rgb(0.64, 0.72, 0.84),
            color_b: rgb(0.74, 0.8, 0.9),
            brightness: 0.55,
            lit: 0.0,
            beam: 0.0,
            shade: 0.0,
            night: 0.0,
            weather: 2,
            density: 1.0,
            focus_radius: 0.0,
        },
        Preset::Snow => PresetLook {
            projection: Projection::Ground,
            layers: 16,
            spacing: 1.2,
            texel: 0.09,
            height: 19.2,
            band: 0.0,
            fall_speed: 1.1,
            wind: Vector2::new(0.3, 0.1),
            fps: 10.0,
            color: rgb(0.96, 0.97, 1.0),
            color_b: rgb(0.85, 0.9, 1.0),
            brightness: 0.8,
            lit: 0.0,
            beam: 0.0,
            shade: 0.0,
            night: 0.0,
            weather: 3,
            density: 1.0,
            focus_radius: 0.0,
        },
        Preset::Embers => PresetLook {
            projection: Projection::Ground,
            layers: 8,
            spacing: 0.5,
            texel: 0.08,
            height: 4.0,
            band: 3.0,
            fall_speed: -0.5,
            wind: Vector2::new(0.1, 0.05),
            fps: 12.0,
            color: rgb(1.0, 0.5, 0.15),
            color_b: rgb(1.0, 0.82, 0.3),
            brightness: 1.2,
            lit: 0.0,
            beam: 0.0,
            shade: 0.0,
            night: -0.5,
            weather: 0,
            density: 1.0,
            focus_radius: 0.0,
        },
        Preset::Leaves => PresetLook {
            projection: Projection::Ground,
            layers: 12,
            spacing: 1.0,
            texel: 0.1,
            height: 12.0,
            band: 8.0,
            fall_speed: 0.5,
            wind: Vector2::new(0.6, 0.2),
            fps: 10.0,
            color: rgb(0.62, 0.78, 0.3),
            color_b: rgb(0.95, 0.78, 0.32),
            brightness: 0.9,
            lit: 0.7,
            beam: 0.0,
            shade: 0.45,
            night: 0.6,
            weather: 1,
            density: 1.0,
            focus_radius: 0.0,
        },
    }
}

/// Flipbook particles in world-anchored layers: dust motes in the sunbeams, rain streaks,
/// snow, embers, falling leaves / pollen. A small 1-texel particle animation that tiles and
/// loops (baked from the Flipbook parameters) is drawn on stacked world sheets, so the effect
/// costs one texture fetch per texel instead of a particle simulation, stays crisp in the pixel
/// pipeline and keeps still under camera moves with ortho and perspective cameras.
///
/// Place the node at the bottom of the volume: Ground sheets fall through `layers x spacing`
/// metres above it (particles live up to `band` above the terrain), Upright sheets stand
/// `height` metres tall from it. Brightness follows the sun (shadow map and the god-ray cloud
/// gaps, so dust shows in the light shafts), the time of day and optionally the weather.
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelParticleField {
    /// Look preset: sets the flipbook, the layers and the look below.
    #[export(enum = (DustMotes, Rain, Snow, Embers, Leaves))]
    #[var(set = set_preset)]
    preset: i32,
    #[export]
    emitting: bool,
    /// Fraction of particles shown (whole particles drop out).
    #[export(range = (0.0, 1.0, 0.01))]
    density: f32,
    /// Follow PixelWeather: Always, Fair Weather (fades with rain / snow / cloud cover),
    /// Rain (rain intensity), Snow (snowfall).
    #[export(enum = (Always, FairWeather, Rain, Snow))]
    weather_response: i32,

    #[export_group(name = "Flipbook")]
    /// Tile size in texels (the sheet holds `frame_count` tiles).
    #[export(range = (16.0, 256.0, 1.0))]
    tile_size: i32,
    #[export(range = (4.0, 128.0, 1.0))]
    frame_count: i32,
    /// Particles emitted per loop.
    #[export(range = (1.0, 2000.0, 1.0))]
    particle_count: i32,
    /// Particle lifetime (frames).
    #[export(range = (1.0, 128.0, 1.0))]
    lifetime: i32,
    /// Particle velocity (texels per frame; y = down the sheet).
    #[export]
    velocity: Vector2,
    /// Random wandering speed (texels per frame).
    #[export(range = (0.0, 4.0, 0.01))]
    drift: f32,
    /// Sideways sway (texels).
    #[export(range = (0.0, 8.0, 0.1))]
    sway: f32,
    /// Fade in / out (frames).
    #[export(range = (0.0, 64.0, 1.0))]
    fade_frames: i32,
    /// Rain streak length (texels; 0 = dots).
    #[export(range = (0.0, 12.0, 1.0))]
    streak_length: i32,
    #[export]
    seed: i32,
    /// Flipbook frames per second.
    #[export(range = (0.0, 60.0, 0.1))]
    fps: f32,

    #[export_group(name = "Layers")]
    #[export(enum = (Ground, Upright))]
    projection: i32,
    #[export(range = (1.0, 64.0, 1.0))]
    layers: i32,
    /// Distance between sheets (m).
    #[export(range = (0.05, 20.0, 0.01))]
    spacing: f32,
    /// World size of one particle texel (m).
    #[export(range = (0.01, 1.0, 0.005))]
    texel_size: f32,
    /// Half-size of each sheet (m); 0 = cover the camera's view.
    #[export(range = (0.0, 500.0, 0.5))]
    extent: f32,
    /// Upright sheet height (m).
    #[export(range = (0.5, 200.0, 0.5))]
    height: f32,
    /// Ground: particles only up to this far above the terrain (0 = everywhere).
    #[export(range = (0.0, 50.0, 0.1))]
    band: f32,
    /// Ground: sheet fall speed (m/s, negative rises). Upright: extra fall scroll.
    #[export(range = (-10.0, 30.0, 0.01))]
    fall_speed: f32,
    /// Horizontal drift (m/s, X / Z).
    #[export]
    wind: Vector2,
    /// Particles only within this distance (m) of where the camera looks at the ground, fading
    /// out towards it (0 = the whole view).
    #[export(range = (0.0, 200.0, 0.5))]
    focus_radius: f32,

    #[export_group(name = "Look")]
    #[export]
    color: Color,
    /// Second color (half of the particles).
    #[export]
    color_b: Color,
    #[export(range = (0.0, 8.0, 0.01))]
    brightness: f32,
    /// 0 = self-lit, 1 = lit by the sun only (shadows hide it).
    #[export(range = (0.0, 1.0, 0.01))]
    sunlit: f32,
    /// 0..1: only inside the god-ray light shafts (cloud gaps).
    #[export(range = (0.0, 1.0, 0.01))]
    beam: f32,
    /// Brightness left in shade (sunlit particles).
    #[export(range = (0.0, 1.0, 0.01))]
    shade: f32,
    /// 1 = fades out at night, -1 = shows only at night.
    #[export(range = (-1.0, 1.0, 0.01))]
    night: f32,

    layers_draw: ParticleLayers,
    terrain: Option<Gd<Node>>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for PixelParticleField {
    fn on_validate_property(&self, property: &mut godot::register::info::PropertyInfo) {
        crate::util::space_enum_hint(property);
    }

    fn init(base: Base<Node3D>) -> Self {
        let mut s = PixelParticleField {
            preset: 0,
            emitting: true,
            density: 1.0,
            weather_response: 1,
            tile_size: 128,
            frame_count: 64,
            particle_count: 150,
            lifetime: 48,
            velocity: Vector2::ZERO,
            drift: 0.0,
            sway: 0.0,
            fade_frames: 12,
            streak_length: 0,
            seed: 0,
            fps: 12.0,
            projection: 0,
            layers: 1,
            spacing: 1.0,
            texel_size: 0.1,
            extent: 0.0,
            height: 10.0,
            band: 0.0,
            fall_speed: 0.0,
            wind: Vector2::ZERO,
            focus_radius: 0.0,
            color: Color::WHITE,
            color_b: Color::WHITE,
            brightness: 1.0,
            sunlit: 1.0,
            beam: 0.0,
            shade: 0.0,
            night: 0.0,
            layers_draw: ParticleLayers::default(),
            terrain: None,
            base,
        };
        s.apply_preset(Preset::DustMotes);
        s
    }

    fn ready(&mut self) {
        globals::ensure();
        let mut node = self.to_gd().upcast::<Node>();
        self.layers_draw.attach(&mut node, "Layers");
        self.base_mut().set_process(true);
    }

    fn process(&mut self, _delta: f64) {
        let fp = self.flipbook_params();
        self.layers_draw.set_flipbook(&fp);
        let look = self.look();
        let node = self.to_gd().upcast::<Node>();
        self.layers_draw.update(&node, &look);
    }

    fn exit_tree(&mut self) {
        self.layers_draw.detach();
    }
}

impl PixelParticleField {
    fn apply_preset(&mut self, p: Preset) {
        let f = p.params();
        self.tile_size = f.size as i32;
        self.frame_count = f.frames as i32;
        self.particle_count = f.count as i32;
        self.lifetime = f.lifetime as i32;
        self.velocity = Vector2::new(f.velocity[0], f.velocity[1]);
        self.drift = f.drift;
        self.sway = f.sway;
        self.fade_frames = f.fade_out as i32;
        self.streak_length = if let Shape::Streak(n) = f.shape { n as i32 } else { 0 };
        self.seed = 0;
        let l = preset_look(p);
        self.projection = l.projection as i32;
        self.layers = l.layers;
        self.spacing = l.spacing;
        self.texel_size = l.texel;
        self.height = l.height;
        self.band = l.band;
        self.fall_speed = l.fall_speed;
        self.wind = l.wind;
        self.fps = l.fps;
        self.color = l.color;
        self.color_b = l.color_b;
        self.brightness = l.brightness;
        self.sunlit = l.lit;
        self.beam = l.beam;
        self.shade = l.shade;
        self.night = l.night;
        self.weather_response = l.weather;
        self.density = l.density;
        self.focus_radius = l.focus_radius;
    }

    fn flipbook_params(&self) -> flipbook::Params {
        let preset = Preset::from_index(self.preset as i64);
        let mut p = preset.params();
        p.size = self.tile_size.clamp(16, 256) as u32;
        p.frames = self.frame_count.clamp(4, 128) as u32;
        p.count = self.particle_count.clamp(1, 4000) as u32;
        p.lifetime = self.lifetime.clamp(1, 128) as u32;
        p.velocity = [self.velocity.x, self.velocity.y];
        p.drift = self.drift.max(0.0);
        p.sway = self.sway.max(0.0);
        let fade = self.fade_frames.max(0) as u32;
        p.fade_out = fade;
        p.fade_in = if preset == Preset::Embers || preset == Preset::RainStreaks { p.fade_in.min(fade) } else { fade * 3 / 4 };
        p.shape = match (preset, self.streak_length) {
            (_, n) if n > 0 => Shape::Streak(n.clamp(1, 12) as u8),
            (Preset::RainStreaks, _) => Shape::Dot,
            (_, _) => p.shape,
        };
        p.seed = p.seed.wrapping_add(self.seed as u64);
        p
    }

    fn weather_amount(&self) -> f32 {
        let Some(m) = weather::mix() else { return if self.weather_response >= 2 { 0.0 } else { 1.0 } };
        match self.weather_response {
            1 => (1.0 - (m.p.rain + m.p.snowfall) * 1.2 - m.p.coverage.max(0.0) * 0.8).clamp(0.0, 1.0),
            2 => m.p.rain.clamp(0.0, 1.0),
            3 => m.p.snowfall.clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    /// Where the camera's view meets the terrain (a few plane refinements against the ground).
    fn ground_focus(&mut self) -> Vector3 {
        let node = self.to_gd().upcast::<Node>();
        let fallback = self.base().get_global_position();
        let Some(cam) = active_camera(&node) else { return fallback };
        if !self.terrain.as_ref().is_some_and(|t| t.is_instance_valid()) {
            self.terrain = find_terrain(&node);
        }
        let mut y = fallback.y;
        let mut p = camera_focus(&cam, y);
        for _ in 0..4 {
            y = ground_height(&mut self.terrain, p, y);
            p = camera_focus(&cam, y);
        }
        p
    }

    fn look(&mut self) -> Look {
        let projection = if self.projection == 1 { Projection::Upright } else { Projection::Ground };
        // A pixel-pipeline effect: the plain editor viewport (no Pixel Preview) shows none.
        let hidden = is_editor() && !crate::render::object::editor_preview();
        let amount = if self.emitting && !hidden { self.density.clamp(0.0, 1.0) * self.weather_amount() } else { 0.0 };
        let fade_center = if self.focus_radius > 0.0 && amount > 0.0 { self.ground_focus() } else { Vector3::ZERO };
        let base_y = self.base().get_global_position().y;
        let wind_scale = weather::mix().map(|m| m.p.wind_scale).unwrap_or(1.0);
        Look {
            projection,
            layers: self.layers,
            spacing: self.spacing,
            texel: self.texel_size,
            extent: self.extent,
            base_y,
            height: self.height,
            fall_speed: self.fall_speed,
            wind: self.wind * wind_scale,
            slant: 0.0,
            // Leaves / pollen ride the global wind gusts (pixel_wind.gdshaderinc).
            gust: if self.preset == 4 { 0.6 } else { 0.0 },
            band: self.band,
            fps: self.fps,
            color: self.color,
            color_b: self.color_b,
            brightness: self.brightness,
            amount,
            lit: self.sunlit,
            beam: self.beam,
            shade: self.shade,
            night: self.night,
            focus_radius: self.focus_radius,
            fade_center,
        }
    }
}

#[godot_api]
impl PixelParticleField {
    #[func]
    fn set_preset(&mut self, v: i32) {
        let v = v.clamp(0, Preset::ALL.len() as i32 - 1);
        let changed = v != self.preset;
        self.preset = v;
        // While a scene loads (outside the tree) only the index is stored: the saved values of
        // the other properties follow and must not be overwritten.
        if changed && self.base().is_inside_tree() {
            self.apply_preset(Preset::from_index(v as i64));
            self.base_mut().notify_property_list_changed();
        }
    }

    /// The baked flipbook sheet (frames packed row-major).
    #[func]
    fn get_flipbook_texture(&mut self) -> Option<Gd<ImageTexture>> {
        let fp = self.flipbook_params();
        self.layers_draw.set_flipbook(&fp);
        self.layers_draw.texture()
    }

    /// Saves the flipbook sheet as a PNG (e.g. "res://dust.png"). Returns true on success.
    #[func]
    fn save_flipbook_png(&mut self, path: GString) -> bool {
        let b = flipbook::bake(&self.flipbook_params());
        let Some(img) = Image::create_from_data(b.width() as i32, b.height() as i32, false, Format::RGBA8, &PackedByteArray::from(b.rgba.as_slice())) else {
            return false;
        };
        img.save_png(&path) == godot::global::Error::OK
    }
}
