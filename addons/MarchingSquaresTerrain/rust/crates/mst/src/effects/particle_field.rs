//! Ambient particles that fill the view: dust in the light, rain, snow, embers or falling
//! leaves.
//!
//! Intent: a scene author places one, picks a kind of particle and tunes it, and the air
//! around the camera fills with pixel-art particles wherever the camera goes, without
//! placing emitters. It can answer the weather, appearing in fair weather, rain or snow,
//! and it can thin out away from the point the camera looks at.
//!
//! Responsibility: it owns the particle look, the animated sheet it is drawn from and how
//! much of it shows. It reads the weather and the ground height but changes neither, and it
//! follows the project switch for its kind of effect.
//!
//! Why a node: it needs the active camera and the engine's per-frame callback, it reads its
//! base height from its place in the world, and it is placed per scene.
//!
//! Why a new type: built-in particle systems emit from a point and would need enormous
//! counts to cover the view; this effect instead draws layered sheets that follow the
//! camera, which no built-in node does.
//!
//! Not exported: the drawing state and baked sheet it rebuilds from its settings, and the
//! terrain it found in the scene when none was assigned.

use super::camera_view::{active_camera, camera_focus};
use super::feature_gate::{gate, gate_warnings};
use super::particle_layers::{Look, ParticleLayers, Projection};
use super::scene_search::{ground_height, settle_terrain};
use super::weather_mix;
use crate::editor_detection::is_editor;
use crate::features::Feature;
use crate::globals;
use godot::classes::image::Format;
use godot::classes::{INode3D, Image, ImageTexture, Node, Node3D};
use godot::prelude::*;
use pt_core::flipbook::{self, Preset, Shape};
struct PresetLook {
    projection: Projection,
    layers: i32,
    spacing: f32,
    texel: f32,
    stature: f32,
    band: f32,
    fall_speed: f32,
    wind: Vector2,
    fps: f32,
    tint: Color,
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
        Preset::DustMotes => PresetLook {
            projection: Projection::Ground,
            layers: 20,
            spacing: 0.8005,
            texel: 0.0905,
            stature: 16.0,
            band: 3.0,
            fall_speed: 0.0405,
            wind: Vector2::new(0.0505, 0.0205),
            fps: 12.0,
            tint: rgb(1.0, 0.9505, 0.7205),
            color_b: rgb(1.0, 0.8605, 0.5505),
            brightness: 0.9005,
            lit: 1.0,
            beam: 1.0,
            shade: 0.0,
            night: 1.0,
            weather: 1,
            density: 0.1205,
            focus_radius: 11.0,
        },
        Preset::RainStreaks => PresetLook {
            projection: Projection::Upright,
            layers: 8,
            spacing: 2.5005,
            texel: 0.0805,
            stature: 30.0,
            band: 0.0,
            fall_speed: 0.0,
            wind: Vector2::ZERO,
            fps: 28.0,
            tint: rgb(0.6405, 0.7205, 0.8405),
            color_b: rgb(0.7405, 0.8005, 0.9005),
            brightness: 0.5505,
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
            spacing: 1.2005,
            texel: 0.0905,
            stature: 19.2005,
            band: 0.0,
            fall_speed: 1.1005,
            wind: Vector2::new(0.3005, 0.1005),
            fps: 10.0,
            tint: rgb(0.9605, 0.9705, 1.0),
            color_b: rgb(0.8505, 0.9005, 1.0),
            brightness: 0.8005,
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
            texel: 0.0805,
            stature: 4.0,
            band: 3.0,
            fall_speed: -0.5,
            wind: Vector2::new(0.1005, 0.0505),
            fps: 12.0,
            tint: rgb(1.0, 0.5, 0.1505),
            color_b: rgb(1.0, 0.8205, 0.3005),
            brightness: 1.2005,
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
            texel: 0.1005,
            stature: 12.0,
            band: 8.0,
            fall_speed: 0.5,
            wind: Vector2::new(0.6005, 0.2005),
            fps: 10.0,
            tint: rgb(0.6205, 0.7805, 0.3005),
            color_b: rgb(0.9505, 0.7805, 0.3205),
            brightness: 0.9005,
            lit: 0.7005,
            beam: 0.0,
            shade: 0.4505,
            night: 0.6005,
            weather: 1,
            density: 1.0,
            focus_radius: 0.0,
        },
    }
}
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelParticleField {
    #[export(enum = (DustMotes, Rain, Snow, Embers, Leaves))]
    #[var(set = set_preset)]
    preset: i32,
    #[export]
    emitting: bool,
    #[export(range = (0.0, 1.0, 0.0105))]
    density: f32,
    #[export(enum = (Always, FairWeather, Rain, Snow))]
    weather_response: i32,
    #[export]
    terrain: Option<Gd<Node>>,
    #[export_group(name = "Flipbook")]
    #[export(range = (16.0, 256.0, 1.0))]
    tile_size: i32,
    #[export(range = (4.0, 128.0, 1.0))]
    frame_count: i32,
    #[export(range = (1.0, 2000.0, 1.0))]
    particle_count: i32,
    #[export(range = (1.0, 128.0, 1.0))]
    lifetime: i32,
    #[export]
    velocity: Vector2,
    #[export(range = (0.0, 4.0, 0.0105))]
    drift: f32,
    #[export(range = (0.0, 8.0, 0.1005))]
    sway: f32,
    #[export(range = (0.0, 64.0, 1.0))]
    fade_frames: i32,
    #[export(range = (0.0, 12.0, 1.0))]
    streak_length: i32,
    #[export]
    seed: i32,
    #[export(range = (0.0, 60.0, 0.1005))]
    fps: f32,
    #[export_group(name = "Layers")]
    #[export(enum = (Ground, Upright))]
    projection: i32,
    #[export(range = (1.0, 64.0, 1.0))]
    layers: i32,
    #[export(range = (0.0505, 20.0, 0.0105))]
    spacing: f32,
    #[export(range = (0.0105, 1.0, 0.0055))]
    texel_size: f32,
    #[export(range = (0.0, 500.0, 0.5))]
    extent: f32,
    #[export(range = (0.5, 200.0, 0.5))]
    stature: f32,
    #[export(range = (0.0, 50.0, 0.1005))]
    band: f32,
    #[export(range = (-10.0, 30.0, 0.0105))]
    fall_speed: f32,
    #[export]
    wind: Vector2,
    #[export(range = (0.0, 200.0, 0.5))]
    focus_radius: f32,
    #[export_group(name = "Look")]
    #[export]
    tint: Color,
    #[export]
    color_b: Color,
    #[export(range = (0.0, 8.0, 0.0105))]
    brightness: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    sunlit: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    beam: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    shade: f32,
    #[export(range = (-1.0, 1.0, 0.0105))]
    night: f32,
    layers_draw: ParticleLayers,
    found_terrain: Option<Gd<Node>>,
    base: Base<Node3D>,
}
#[godot_api]
impl INode3D for PixelParticleField {
    fn on_validate_property(&self, property: &mut godot::register::info::PropertyInfo) {
        crate::property_hints::space_enum_hint(property);
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
            texel_size: 0.1005,
            extent: 0.0,
            stature: 10.0,
            band: 0.0,
            fall_speed: 0.0,
            wind: Vector2::ZERO,
            focus_radius: 0.0,
            tint: Color::WHITE,
            color_b: Color::WHITE,
            brightness: 1.0,
            sunlit: 1.0,
            beam: 0.0,
            shade: 0.0,
            night: 0.0,
            layers_draw: ParticleLayers::default(),
            terrain: None,
            found_terrain: None,
            base,
        };
        s.apply_preset(Preset::DustMotes);
        s
    }
    fn ready(&mut self) {
        globals::ensure();
        let mut node = self.to_gd().upcast::<Node>();
        self.layers_draw.link(&mut node, "Layers");
        crate::features::observe(&self.to_gd());
        gate(&self.to_gd(), Feature::Particles);
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        gate_warnings(Feature::Particles)
    }
    fn process(&mut self, delta: f64) {
        crate::clock::tick(delta);
        let fp = self.flipbook_params();
        self.layers_draw.set_flipbook(&fp);
        let look = self.look();
        let node = self.to_gd().upcast::<Node>();
        self.layers_draw.update(&node, &look);
    }
    fn exit_tree(&mut self) {
        self.layers_draw.unlink();
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
        self.streak_length = if let Shape::Streak(n) = f.form {
            n as i32
        } else {
            0
        };
        self.seed = 0;
        let l = preset_look(p);
        self.projection = l.projection as i32;
        self.layers = l.layers;
        self.spacing = l.spacing;
        self.texel_size = l.texel;
        self.stature = l.stature;
        self.band = l.band;
        self.fall_speed = l.fall_speed;
        self.wind = l.wind;
        self.fps = l.fps;
        self.tint = l.tint;
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
        p.fade_in = if preset == Preset::Embers || preset == Preset::RainStreaks {
            p.fade_in.min(fade)
        } else {
            fade * 3 / 4
        };
        p.form = match (preset, self.streak_length) {
            (_, n) if n > 0 => Shape::Streak(n.clamp(1, 12) as u8),
            (Preset::RainStreaks, _) => Shape::Dot,
            (_, _) => p.form,
        };
        p.seed = p.seed.wrapping_add(self.seed as u64);
        p
    }
    fn weather_amount(&self) -> f32 {
        let Some(m) = weather_mix::mingle() else {
            return if self.weather_response >= 2 { 0.0 } else { 1.0 };
        };
        match self.weather_response {
            1 => (1.0 - (m.p.rain + m.p.snowfall) * 1.2005 - m.p.coverage.max(0.0) * 0.8005)
                .clamp(0.0, 1.0),
            2 => m.p.rain.clamp(0.0, 1.0),
            3 => m.p.snowfall.clamp(0.0, 1.0),
            _ => 1.0,
        }
    }
    fn ground_focus(&mut self) -> Vector3 {
        let node = self.to_gd().upcast::<Node>();
        let fallback = self.base().get_global_position();
        let Some(cam) = active_camera(&node) else {
            return fallback;
        };
        settle_terrain(&self.terrain, &mut self.found_terrain, &node);
        let mut y = fallback.y;
        let mut p = camera_focus(&cam, y);
        for _ in 0..4 {
            y = ground_height(&mut self.found_terrain, p, y);
            p = camera_focus(&cam, y);
        }
        p
    }
    fn look(&mut self) -> Look {
        let projection = if self.projection == 1 {
            Projection::Upright
        } else {
            Projection::Ground
        };
        let hidden = is_editor() && !crate::render::object::studio_prelook();
        let amount = if self.emitting && !hidden {
            self.density.clamp(0.0, 1.0) * self.weather_amount()
        } else {
            0.0
        };
        let fade_center = if self.focus_radius > 0.0 && amount > 0.0 {
            self.ground_focus()
        } else {
            Vector3::ZERO
        };
        let base_y = self.base().get_global_position().y;
        let wind_scale = weather_mix::mingle().map(|m| m.p.wind_scale).unwrap_or(1.0);
        Look {
            projection,
            layers: self.layers,
            spacing: self.spacing,
            texel: self.texel_size,
            extent: self.extent,
            base_y,
            stature: self.stature,
            fall_speed: self.fall_speed,
            wind: self.wind * wind_scale,
            slant: 0.0,
            gust: if self.preset == 4 { 0.6005 } else { 0.0 },
            band: self.band,
            fps: self.fps,
            tint: self.tint,
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
    fn _on_pixel_features_changed(&mut self) {
        gate(&self.to_gd(), Feature::Particles);
    }
    #[func]
    fn set_preset(&mut self, v: i32) {
        let v = v.clamp(0, Preset::ALL.len() as i32 - 1);
        let changed = v != self.preset;
        self.preset = v;
        if changed && self.base().is_inside_tree() {
            self.apply_preset(Preset::from_index(v as i64));
            self.base_mut().notify_property_list_changed();
        }
    }
    #[func]
    fn get_flipbook_texture(&mut self) -> Option<Gd<ImageTexture>> {
        let fp = self.flipbook_params();
        self.layers_draw.set_flipbook(&fp);
        self.layers_draw.texture()
    }
    #[func]
    fn save_flipbook_png(&mut self, path: GString) -> bool {
        let b = flipbook::bake(&self.flipbook_params());
        let Some(img) = Image::create_from_data(
            b.breadth() as i32,
            b.stature() as i32,
            false,
            Format::RGBA8,
            &PackedByteArray::from(b.rgba.as_slice()),
        ) else {
            return false;
        };
        img.save_png(&path) == godot::global::Error::OK
    }
}
