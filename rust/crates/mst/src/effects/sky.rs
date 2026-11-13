//! `PixelSky` — drives the shared cloud field (shadows, god rays, optional visible deck)
//! and publishes the sun direction to every Marching Squares Terrain shader.

use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::{Camera3D, DirectionalLight3D, INode3D, MeshInstance3D, Node3D, PlaneMesh, ShaderMaterial};
use godot::prelude::*;

use crate::globals;
use crate::util::*;

/// Direction to the sun when no light is assigned or found.
fn default_sun_dir() -> Vector3 {
    Vector3::new(0.3, 0.8, 0.4).normalized()
}

/// Animated cloud field shared by all pixel shaders. Cloud shadows are projected along the
/// sun onto terrain, objects, grass and water; the `PixelRenderer` god rays shine through
/// the gaps; and an optional flat cloud deck shows the clouds themselves.
///
/// Updates every frame, in the editor too, so shadows can be previewed while authoring.
/// Several PixelSky nodes may coexist: one drives the shared field per frame (see
/// `cloud_camera_scope` and `effects::cloud_field`).
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelSky {
    /// Light the clouds are projected from. Empty = first DirectionalLight3D among this
    /// node's siblings.
    #[export]
    sun: Option<Gd<DirectionalLight3D>>,
    /// When set, this sky drives the cloud field only while this camera is the active one
    /// (two-camera scenes switch cloud fields with the camera). Empty = fallback provider.
    #[export]
    cloud_camera_scope: Option<Gd<Camera3D>>,
    /// Enables cloud shadows (and god rays, which need the cloud field).
    #[export]
    clouds_enabled: bool,
    /// Fraction of the sky covered by clouds.
    #[export(range = (0.0, 1.0, 0.01))]
    coverage: f32,
    /// Width of the cloud edge transition.
    #[export(range = (0.0, 0.5, 0.005))]
    softness: f32,
    /// Noise frequency per world unit (smaller = bigger clouds).
    #[export(range = (0.001, 0.2, 0.001))]
    cloud_scale: f32,
    /// Cloud drift in noise units per second.
    #[export]
    wind: Vector2,
    /// World Y of the cloud plane.
    #[export(range = (1.0, 500.0, 0.5, or_greater))]
    cloud_height: f32,
    /// How dark cloud shadows are (0 = no shadow, 1 = black).
    #[export(range = (0.0, 1.0, 0.01))]
    shadow_strength: f32,
    /// Quantize cloud density into this many steps (0 = smooth).
    #[export(range = (0.0, 8.0, 1.0))]
    bands: i32,

    #[export_group(name = "Cloud Deck", prefix = "deck_")]
    /// Draw the clouds as a flat plane at `cloud_height` that follows the camera.
    #[export]
    #[var(set = set_deck_enabled)]
    deck_enabled: bool,
    /// Side length of the deck plane.
    #[export(range = (10.0, 4000.0, 1.0, or_greater))]
    deck_size: f32,
    #[export(range = (0.0, 1.0, 0.01))]
    deck_opacity: f32,
    #[export]
    deck_color: Color,
    /// Color of the sun-facing-away side of each cloud.
    #[export]
    deck_shade_color: Color,

    offset: Vector2,
    deck: Option<Gd<MeshInstance3D>>,
    deck_mat: Option<Gd<ShaderMaterial>>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for PixelSky {
    fn init(base: Base<Node3D>) -> Self {
        PixelSky {
            sun: None,
            cloud_camera_scope: None,
            clouds_enabled: true,
            coverage: 0.45,
            softness: 0.12,
            cloud_scale: 0.02,
            wind: Vector2::new(0.02, 0.01),
            cloud_height: 40.0,
            shadow_strength: 0.45,
            bands: 0,
            deck_enabled: false,
            deck_size: 400.0,
            deck_opacity: 0.85,
            deck_color: Color::from_rgb(1.0, 0.98, 0.95),
            deck_shade_color: Color::from_rgb(0.72, 0.74, 0.85),
            offset: Vector2::ZERO,
            deck: None,
            deck_mat: None,
            base,
        }
    }

    fn ready(&mut self) {
        globals::ensure();
        self.to_gd().add_to_group("__pixel_sky");
        super::cloud_field::register_provider(&self.to_gd().upcast());
        self.base_mut().set_process(true);
        self.update_deck();
    }

    fn process(&mut self, delta: f64) {
        // Wrap the offset so it never loses float precision in long sessions.
        let wind_scale = super::weather::mix().map(|m| m.p.wind_scale).unwrap_or(1.0);
        self.offset += self.wind * wind_scale * delta as f32;
        if self.offset.x.abs() > 4096.0 || self.offset.y.abs() > 4096.0 {
            self.offset = Vector2::ZERO;
        }
        let node: Gd<Node> = self.to_gd().upcast();
        let cam = super::active_camera(&node).map(|c| c.instance_id());
        let scope = self.cloud_camera_scope.as_ref().filter(|c| c.is_instance_valid()).map(|c| c.instance_id());
        let active = super::cloud_field::active_provider(node.instance_id(), scope, cam);
        if active == Some(node.instance_id()) {
            self.publish();
        } else if active.is_none() {
            super::cloud_field::publish_neutral();
        }
        if let Some(d) = self.deck.as_mut().filter(|d| d.is_instance_valid()) {
            d.set_visible(active == Some(node.instance_id()));
        }
        self.follow_camera();
    }

    fn exit_tree(&mut self) {
        super::cloud_field::unregister_provider(&self.to_gd().upcast());
    }
}

impl PixelSky {
    fn resolve_sun(&self) -> Option<Gd<DirectionalLight3D>> {
        if let Some(s) = &self.sun
            && s.is_instance_valid()
        {
            return Some(s.clone());
        }
        let parent = self.base().get_parent()?;
        parent.get_children().iter_shared().find_map(|n| n.try_cast::<DirectionalLight3D>().ok())
    }

    /// Writes the sun direction and cloud parameters to the global shader uniforms.
    fn publish(&mut self) {
        let sun_dir = self
            .resolve_sun()
            .filter(|s| s.is_inside_tree())
            // A DirectionalLight3D shines along its -Z, so the direction to the sun is +Z.
            .map(|s| s.get_global_basis().col_c().normalized())
            .filter(|d| d.is_finite() && d.length_squared() > 0.5)
            .unwrap_or_else(default_sun_dir);
        globals::set("pt_sun_dir", &sun_dir.to_variant());
        // PixelWeather offsets (Clear = none) and the lightning flash.
        let (mut coverage, mut softness, mut shadow) = (self.coverage, self.softness, self.shadow_strength);
        let (mut opacity, mut deck, mut deck_shade) = (self.deck_opacity, self.deck_color, self.deck_shade_color);
        if let Some(m) = super::weather::mix() {
            coverage = (coverage + m.p.coverage).clamp(0.0, 1.0);
            softness = (softness + m.p.softness).clamp(0.0, 0.5);
            shadow = (shadow + m.p.shadow).clamp(0.0, 1.0);
            opacity = (opacity + m.p.deck_opacity).clamp(0.0, 1.0);
            let storm = Color::from_rgb(0.36, 0.38, 0.44);
            deck = deck.lerp(storm.lightened(0.25), m.p.deck_darken as f64);
            deck_shade = deck_shade.lerp(storm.darkened(0.25), m.p.deck_darken as f64);
            let flash = Color::from_rgb(0.92, 0.94, 1.0);
            deck = deck.lerp(flash, (m.flash * 0.7) as f64);
            deck_shade = deck_shade.lerp(flash, (m.flash * 0.5) as f64);
            // Lightning briefly lifts the cloud shadows.
            shadow *= 1.0 - m.flash * 0.8;
        }
        let a = Vector4::new(coverage, softness, self.cloud_scale, shadow);
        let b = Vector4::new(self.offset.x, self.offset.y, self.cloud_height, self.bands.max(0) as f32);
        let c = Vector4::new(if self.clouds_enabled { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0);
        globals::set("pt_cloud_a", &a.to_variant());
        globals::set("pt_cloud_b", &b.to_variant());
        globals::set("pt_cloud_c", &c.to_variant());
        if let Some(m) = &mut self.deck_mat {
            m.set_shader_parameter("opacity", &opacity.to_variant());
            m.set_shader_parameter("cloud_color", &deck.to_variant());
            m.set_shader_parameter("shade_color", &deck_shade.to_variant());
        }
    }

    /// Creates or removes the internal deck mesh to match `deck_enabled`.
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
        // Color pass only: keep the deck off the metadata layer.
        let layers = mi.get_layer_mask() & !globals::meta_mask();
        mi.set_layer_mask(layers);
        mi.set_as_top_level(true);
        let mut this = self.to_gd();
        add_internal(&mut this, &mi);
        self.deck = Some(mi);
        self.deck_mat = Some(mat);
    }

    /// Centres the deck where the active camera's view ray meets the cloud plane.
    fn follow_camera(&mut self) {
        let Some(mut deck) = self.deck.clone().filter(|d| d.is_instance_valid()) else { return };
        // In the editor follow the 3D editor camera (the scene's own cameras are not current).
        let cam = if is_editor() {
            crate::render::editor_preview::EditorHost::find().and_then(|h| h.camera())
        } else {
            self.base().get_viewport().and_then(|vp| vp.get_camera_3d())
        };
        let centre = match cam {
            Some(c) => {
                let t = c.get_global_transform();
                let fwd = -t.basis.col_c();
                let hit = if fwd.y.abs() > 1e-3 { (self.cloud_height - t.origin.y) / fwd.y } else { -1.0 };
                let p = if hit > 0.0 { t.origin + fwd * hit.min(self.deck_size) } else { t.origin };
                Vector3::new(p.x, self.cloud_height, p.z)
            }
            None => {
                let p = self.base().get_global_position();
                Vector3::new(p.x, self.cloud_height, p.z)
            }
        };
        let s = self.deck_size.max(1.0);
        deck.set_global_transform(Transform3D::new(Basis::from_scale(Vector3::new(s, 1.0, s)), centre));
    }
}

#[godot_api]
impl PixelSky {
    #[func]
    fn set_deck_enabled(&mut self, v: bool) {
        self.deck_enabled = v;
        self.update_deck();
    }

    /// Current cloud noise offset (accumulated wind).
    #[func]
    fn get_cloud_offset(&self) -> Vector2 {
        self.offset
    }
}
