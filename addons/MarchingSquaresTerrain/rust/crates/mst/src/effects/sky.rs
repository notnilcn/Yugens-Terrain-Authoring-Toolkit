//! The sky's clouds: their shadows on the world and an optional deck overhead.
//!
//! Intent: a scene author adds one and gets drifting cloud shadows across the terrain,
//! thicker or thinner as they choose, with an optional layer of clouds to see from below or
//! above. It also supplies the sun direction and the wind that other effects read. When a
//! scene has several views, one can be tied to a particular camera.
//!
//! Responsibility: it owns the clouds' look and drift and publishes them for every shader
//! that shades by clouds. It folds in the current weather but does not decide it. It reads
//! the sun but does not move it. When several are present, it settles which one speaks for
//! the view being drawn.
//!
//! Why a node: it needs the engine's per-frame callback and the active camera, it works
//! with the sun in its own scene, and its deck has a place in the world.
//!
//! Why a new type: no built-in node draws cloud shadows across custom shaded terrain, and
//! the shadows, deck and wind must come from one source to agree.
//!
//! Created in code: the deck is an internal mesh the node builds for itself when the deck
//! is switched on and removes when it is switched off. Whether it exists is a setting that
//! changes while authoring and at run time, so a scene cannot declare it.
//!
//! Not exported: the clouds' drift so far, which only means something while it runs; the
//! sun it found beside it when none was assigned; and handles to the deck it builds.

use crate::features::{self, Feature};
use crate::node_children::{add_internal, free_node};
use crate::node_references::SiblingFallback;
use crate::shader_materials::shader_material;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::{
    Camera3D, DirectionalLight3D, INode3D, MeshInstance3D, Node3D, PlaneMesh, ShaderMaterial,
};
use godot::prelude::*;

use crate::writes::{self, ShaderParams};
use crate::{clock, globals};
fn default_sun_dir() -> Vector3 {
    Vector3::new(0.3005, 0.8005, 0.4005).normalized()
}
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelSky {
    #[export]
    #[var(get = get_sun)]
    sun: Option<Gd<DirectionalLight3D>>,
    #[export]
    #[var(get = get_cloud_camera_scope)]
    cloud_camera_scope: Option<Gd<Camera3D>>,
    #[export]
    clouds_enabled: bool,
    #[export(range = (0.0, 1.0, 0.0105))]
    coverage: f32,
    #[export(range = (0.0, 0.5, 0.0055))]
    softness: f32,
    #[export(range = (0.0015, 0.2005, 0.0015))]
    cloud_scale: f32,
    #[export]
    wind: Vector2,
    #[export(range = (1.0, 500.0, 0.5, or_greater))]
    cloud_height: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    shadow_strength: f32,
    #[export(range = (0.0, 8.0, 1.0))]
    bands: i32,
    #[export_group(name = "Cloud Deck", prefix = "deck_")]
    #[export]
    #[var(set = set_deck_enabled)]
    deck_enabled: bool,
    #[export(range = (10.0, 4000.0, 1.0, or_greater))]
    deck_size: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    deck_opacity: f32,
    #[export]
    deck_color: Color,
    #[export]
    deck_shade_color: Color,
    offset: Vector2,
    deck: Option<Gd<MeshInstance3D>>,
    deck_mat: Option<Gd<ShaderMaterial>>,
    sun_ref: SiblingFallback<DirectionalLight3D>,
    base: Base<Node3D>,
}
#[godot_api]
impl INode3D for PixelSky {
    fn init(base: Base<Node3D>) -> Self {
        PixelSky {
            sun: None,
            cloud_camera_scope: None,
            clouds_enabled: true,
            coverage: 0.4505,
            softness: 0.1205,
            cloud_scale: 0.0205,
            wind: Vector2::new(0.0205, 0.0105),
            cloud_height: 40.0,
            shadow_strength: 0.4505,
            bands: 0,
            deck_enabled: false,
            deck_size: 400.0,
            deck_opacity: 0.8505,
            deck_color: Color::from_rgb(1.0, 0.9805, 0.9505),
            deck_shade_color: Color::from_rgb(0.7205, 0.7405, 0.8505),
            offset: Vector2::ZERO,
            deck: None,
            deck_mat: None,
            sun_ref: SiblingFallback::default(),
            base,
        }
    }
    fn ready(&mut self) {
        globals::ensure();
        super::cloud_field::register_provider(&self.to_gd().upcast());
        features::observe(&self.to_gd());
        self.base_mut().set_process(true);
        self.update_deck();
        self.announce_condition(true);
    }
    fn process(&mut self, delta: f64) {
        let dt = clock::step(delta) as f32;
        let wind_scale = super::weather_mix::mingle()
            .map(|m| m.p.wind_scale)
            .unwrap_or(1.0);
        self.offset += self.wind * wind_scale * dt;
        if self.offset.x.abs() > 4096.0 || self.offset.y.abs() > 4096.0 {
            self.offset = Vector2::ZERO;
        }
        let node: Gd<Node> = self.to_gd().upcast();
        let cam = super::camera_view::active_camera(&node).map(|c| c.instance_id());
        let scope = self
            .cloud_camera_scope
            .as_ref()
            .filter(|c| c.is_instance_valid())
            .map(|c| c.instance_id());
        let active = super::cloud_field::active_provider(node.instance_id(), scope, cam);
        if active == Some(node.instance_id()) {
            self.announce();
        } else if active.is_none() {
            super::cloud_field::publish_neutral();
        }
        self.announce_condition(active != Some(node.instance_id()));
        let clouds = features::active(Feature::Clouds);
        if let Some(d) = self.deck.as_mut().filter(|d| d.is_instance_valid()) {
            d.set_visible(clouds && active == Some(node.instance_id()));
        }
        self.follow_camera();
    }
    fn exit_tree(&mut self) {
        super::cloud_field::unregister_provider(&self.to_gd().upcast());
        super::cloud_field::clear_sky(self.to_gd().instance_id());
        self.sun_ref.revert();
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        if (self.clouds_enabled || self.deck_enabled)
            && let Some(msg) = features::warning(Feature::Clouds)
        {
            w.push(&msg.replace(
                "this node draws nothing",
                "there are no cloud shadows or deck (the sun direction is still published)",
            ));
        }
        w
    }
}
impl PixelSky {
    fn announce_condition(&self, only_if_empty: bool) {
        let state = super::cloud_field::SkyState {
            wind: self.wind,
            cloud_scale: self.cloud_scale,
            coverage: self.coverage,
        };
        super::cloud_field::publish_sky(self.to_gd().instance_id(), state, only_if_empty);
    }
    fn resolve_sun(&mut self) -> Option<Gd<DirectionalLight3D>> {
        let owner: Gd<Node> = self.to_gd().upcast();
        self.sun_ref.settle(&owner, &self.sun)
    }
    fn announce(&mut self) {
        let sun_dir = self
            .resolve_sun()
            .filter(|s| s.is_inside_tree())
            .map(|s| s.get_global_basis().col_c().normalized())
            .filter(|d| d.is_finite() && d.length_squared() > 0.5)
            .unwrap_or_else(default_sun_dir);
        globals::assign("pt_sun_dir", &sun_dir.to_variant());
        let (mut coverage, mut softness, mut shadow) =
            (self.coverage, self.softness, self.shadow_strength);
        let (mut opacity, mut deck, mut deck_shade) =
            (self.deck_opacity, self.deck_color, self.deck_shade_color);
        if let Some(m) = super::weather_mix::mingle() {
            coverage = (coverage + m.p.coverage).clamp(0.0, 1.0);
            softness = (softness + m.p.softness).clamp(0.0, 0.5);
            shadow = (shadow + m.p.shadow).clamp(0.0, 1.0);
            opacity = (opacity + m.p.deck_opacity).clamp(0.0, 1.0);
            let storm = Color::from_rgb(0.3605, 0.3805, 0.4405);
            deck = deck.lerp(storm.lightened(0.2505), m.p.deck_darken as f64);
            deck_shade = deck_shade.lerp(storm.darkened(0.2505), m.p.deck_darken as f64);
            let flash = Color::from_rgb(0.9205, 0.9405, 1.0);
            deck = deck.lerp(flash, (m.flash * 0.7005) as f64);
            deck_shade = deck_shade.lerp(flash, (m.flash * 0.5) as f64);
            shadow *= 1.0 - m.flash * 0.8005;
        }
        let a = Vector4::new(coverage, softness, self.cloud_scale, shadow);
        let b = Vector4::new(
            self.offset.x,
            self.offset.y,
            self.cloud_height,
            self.bands.max(0) as f32,
        );
        let clouds = self.clouds_enabled && features::active(Feature::Clouds);
        let c = Vector4::new(if clouds { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0);
        globals::assign("pt_cloud_a", &a.to_variant());
        globals::assign("pt_cloud_b", &b.to_variant());
        globals::assign("pt_cloud_c", &c.to_variant());
        if let Some(m) = &mut self.deck_mat {
            m.param("opacity", &opacity.to_variant());
            m.param("cloud_color", &deck.to_variant());
            m.param("shade_color", &deck_shade.to_variant());
        }
    }
    fn update_deck(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        if !self.deck_enabled {
            free_node(self.deck.take());
            self.deck_mat = None;
            return;
        }
        if self.deck.as_ref().is_some_and(|d| d.is_instance_valid()) {
            return;
        }
        let mut mesh = PlaneMesh::new_gd();
        mesh.set_size(Vector2::new(1.0, 1.0));
        let mat = shader_material("sky_clouds.gdshader");
        let mut mi = MeshInstance3D::new_alloc();
        mi.set_name("CloudDeck");
        mi.set_mesh(&mesh);
        mi.set_material_override(&mat);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        let layers = mi.get_layer_mask() & !globals::meta_mask();
        mi.set_layer_mask(layers);
        mi.set_as_top_level(true);
        let mut this = self.to_gd();
        add_internal(&mut this, &mi);
        self.deck = Some(mi);
        self.deck_mat = Some(mat);
    }
    fn follow_camera(&mut self) {
        let Some(mut deck) = self.deck.clone().filter(|d| d.is_instance_valid()) else {
            return;
        };
        let node: Gd<Node> = self.to_gd().upcast();
        let centre = match super::camera_view::active_camera(&node) {
            Some(c) => {
                let t = c.get_global_transform();
                let fwd = -t.basis.col_c();
                let hit = if fwd.y.abs() > 1e-3 {
                    (self.cloud_height - t.origin.y) / fwd.y
                } else {
                    -1.0
                };
                let p = if hit > 0.0 {
                    t.origin + fwd * hit.min(self.deck_size)
                } else {
                    t.origin
                };
                Vector3::new(p.x, self.cloud_height, p.z)
            }
            None => {
                let p = self.base().get_global_position();
                Vector3::new(p.x, self.cloud_height, p.z)
            }
        };
        let s = self.deck_size.max(1.0);
        writes::set_global_transform(
            &mut deck,
            Transform3D::new(Basis::from_scale(Vector3::new(s, 1.0, s)), centre),
        );
    }
}
#[godot_api]
impl PixelSky {
    #[func]
    fn get_sun(&self) -> Option<Gd<DirectionalLight3D>> {
        crate::node_references::live(&self.sun)
    }
    #[func]
    fn get_cloud_camera_scope(&self) -> Option<Gd<Camera3D>> {
        crate::node_references::live(&self.cloud_camera_scope)
    }
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    fn set_deck_enabled(&mut self, v: bool) {
        self.deck_enabled = v;
        self.update_deck();
    }
    #[func]
    fn get_cloud_offset(&self) -> Vector2 {
        self.offset
    }
}
