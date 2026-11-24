//! Shafts of sunlight that stand in the air and are cut by the sun's shadows.
//!
//! Intent: a scene author adds one under a canopy or among ruins and gets visible beams of
//! sunlight where the sun shines through gaps, in the banded pixel-art style, following the
//! camera if asked.
//!
//! Responsibility: it owns the beams' look and keeps them aligned with the sun it is given.
//! It does not move the sun or decide when it casts shadows; it asks the scene author to
//! switch shadows on when they are off. It follows the project switch for its kind of
//! effect.
//!
//! Why a node: it has a place in the world, it follows a particular sun in its scene, and
//! it needs a per-frame callback to stay aligned with it.
//!
//! Why a new type: no built-in node draws light shafts shaped by a light's shadow map in
//! this project's style.
//!
//! Created in code: the beams are drawn by one internal instanced mesh that holds the
//! beam instances the node generates from the sun and its own settings. How many beams exist
//! and where they stand is recomputed whenever the sun turns or the beam count changes, so
//! the drawing part carries nothing a scene author could set.
//!
//! Not exported: the alignment it last built, which is derived from the sun and its
//! settings, and handles to the parts it builds.

use super::camera_view::{active_camera, camera_focus};
use super::feature_gate::{gate, gate_warnings};
use crate::features::Feature;
use crate::globals;
use crate::internal_children::add_internal;
use crate::shader_materials::shader_material;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    DirectionalLight3D, INode3D, MultiMesh, MultiMeshInstance3D, Node, Node3D, QuadMesh,
    ShaderMaterial,
};
use godot::prelude::*;

use crate::node_setters;
use crate::shader_param_cache::ShaderParams;
pub fn sheet_transforms(sun_basis: Basis, count: i32, depth: f32) -> Vec<Transform3D> {
    let up = sun_basis.col_b().normalized();
    let fwd = -sun_basis.col_c().normalized();
    let x = fwd.cross(up).normalized();
    let basis = Basis::from_cols(x, fwd, up);
    let n = count.max(1);
    (0..n)
        .map(|i| {
            let t = (i as f32 + 0.5) / n as f32 - 0.5;
            Transform3D::new(basis, -up * (t * depth))
        })
        .collect()
}
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelSunShafts {
    #[export]
    #[var(get = get_sun, set = set_sun)]
    sun: Option<Gd<DirectionalLight3D>>,
    #[export(range = (1.0, 16.0, 1.0))]
    #[var(set = assign_tally)]
    count: i32,
    #[export(range = (0.0, 200.0, 0.1005, or_greater))]
    #[var(set = set_depth)]
    depth: f32,
    #[export(range = (1.0, 1000.0, 1.0, or_greater))]
    #[var(set = assign_measure)]
    size: f32,
    #[export]
    #[var(set = set_follow_camera)]
    follow_camera: bool,
    #[export]
    #[var(set = assign_beam_tint)]
    beam_tint: Color,
    #[export(range = (0.0, 1.0, 0.0015))]
    #[var(set = set_alpha)]
    alpha: f32,
    #[export(range = (0.0, 100.0, 0.1005, or_greater))]
    #[var(set = set_start_depth)]
    start_depth: f32,
    #[export(range = (0.0, 8.0, 1.0))]
    #[var(set = set_shaft_steps)]
    shaft_steps: i32,
    #[export(range = (0.0, 1.0, 0.0105))]
    #[var(set = set_dither_width)]
    dither_width: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    #[var(set = set_edge_fade)]
    edge_fade: f32,
    #[export(range = (0.0, 100.0, 0.1005, or_greater))]
    #[var(set = set_fade_height)]
    fade_height: f32,
    #[export(range = (0.0, 10.0, 0.0505, or_greater))]
    #[var(set = set_surface_fade)]
    surface_fade: f32,
    #[export_group(name = "Camera Follow")]
    #[export(range = (0.0, 1.0, 0.0005))]
    follow_snap_fraction: f32,
    sheets: Option<Gd<MultiMeshInstance3D>>,
    multimesh: Option<Gd<MultiMesh>>,
    quad: Option<Gd<QuadMesh>>,
    mat: Option<Gd<ShaderMaterial>>,
    built: Option<(Basis, i32, f32)>,
    base: Base<Node3D>,
}
#[godot_api]
impl INode3D for PixelSunShafts {
    fn init(base: Base<Node3D>) -> Self {
        PixelSunShafts {
            sun: None,
            count: 12,
            depth: 18.0,
            size: 200.0,
            follow_camera: true,
            beam_tint: Color::from_rgb(1.0, 0.9155, 0.2705),
            alpha: 0.0105,
            start_depth: 14.0,
            shaft_steps: 3,
            dither_width: 0.5,
            edge_fade: 0.3505,
            fade_height: 12.0,
            surface_fade: 1.5005,
            follow_snap_fraction: 0.0505,
            sheets: None,
            multimesh: None,
            quad: None,
            mat: None,
            built: None,
            base,
        }
    }
    fn ready(&mut self) {
        let mat = shader_material("sun_shafts.gdshader");
        let mut quad = QuadMesh::new_gd();
        quad.set_material(&mat);
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_mesh(&quad);
        let mut mmi = MultiMeshInstance3D::new_alloc();
        mmi.set_name("Sheets");
        mmi.set_multimesh(&mm);
        mmi.set_as_top_level(true);
        mmi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        let layers = mmi.get_layer_mask() & !globals::meta_mask();
        mmi.set_layer_mask(layers);
        let mut this = self.to_gd();
        add_internal(&mut this, &mmi);
        self.sheets = Some(mmi);
        self.multimesh = Some(mm);
        self.quad = Some(quad);
        self.mat = Some(mat);
        self.built = None;
        self.enact();
        crate::features::observe(&self.to_gd());
        self._on_pixel_features_changed();
    }
    fn process(&mut self, delta: f64) {
        crate::clock::tick(delta);
        self.sync();
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = gate_warnings(Feature::SunShafts);
        match self.get_sun() {
            None => w.push("Assign the sun light (a DirectionalLight3D) to Sun: the shafts follow it."),
            Some(s) if !s.has_shadow() => {
                w.push("Turn on shadows on the sun: the shafts are cut by its shadow map.")
            }
            _ => {}
        }
        w
    }
}
impl PixelSunShafts {
    fn resolve_sun(&self) -> Option<Gd<DirectionalLight3D>> {
        self.get_sun().filter(|s| s.is_inside_tree())
    }
    fn sync(&mut self) {
        let Some(sun) = self.resolve_sun() else {
            return;
        };
        let basis = sun.get_global_basis().orthonormalized();
        let key = (basis, self.count, self.depth);
        if self.built != Some(key) {
            self.built = Some(key);
            if let Some(mm) = &mut self.multimesh {
                let xf = sheet_transforms(basis, self.count, self.depth);
                if mm.get_instance_count() != xf.len() as i32 {
                    mm.set_instance_count(xf.len() as i32);
                }
                for (i, t) in xf.iter().enumerate() {
                    mm.set_instance_transform(i as i32, *t);
                }
                let half = self.size * 0.5;
                let r = Vector3::splat(half + self.depth);
                mm.set_custom_aabb(Aabb::new(-r, r * 2.0));
            }
            if let Some(m) = &mut self.mat {
                m.param("sun_height", &basis.col_c().y.clamp(0.0, 1.0).to_variant());
            }
        }
        let mut centre = self.base().get_global_position();
        if self.follow_camera {
            let node: Gd<Node> = self.to_gd().upcast();
            if let Some(cam) = active_camera(&node) {
                let up = basis.col_b().normalized();
                let d = camera_focus(&cam, centre.y) - centre;
                let step = (self.size * self.follow_snap_fraction).max(1.0);
                let inplane = d - up * d.dot(up);
                let snapped = Vector3::new(
                    (inplane.x / step).round() * step,
                    (inplane.y / step).round() * step,
                    (inplane.z / step).round() * step,
                );
                centre += snapped - up * snapped.dot(up);
            }
        }
        let base_y = self.base().get_global_position().y;
        if let Some(m) = &mut self.mat {
            m.param("base_height", &base_y.to_variant());
        }
        if let Some(s) = &mut self.sheets {
            node_setters::set_global_transform(s, Transform3D::new(Basis::IDENTITY, centre));
        }
    }
    fn enact(&mut self) {
        if let Some(q) = &mut self.quad
            && q.get_size() != Vector2::splat(self.size)
        {
            q.set_size(Vector2::splat(self.size));
        }
        if let Some(m) = &mut self.mat {
            m.param("ray_color", &self.beam_tint.to_variant());
            m.param("alpha", &self.alpha.to_variant());
            m.param("start_depth", &self.start_depth.to_variant());
            m.param("shaft_steps", &self.shaft_steps.to_variant());
            m.param("dither_width", &self.dither_width.to_variant());
            m.param("edge_fade", &self.edge_fade.to_variant());
            m.param("fade_height", &self.fade_height.to_variant());
            m.param("surface_fade", &self.surface_fade.to_variant());
        }
        self.built = None;
        if self.sheets.is_some() {
            self.sync();
        }
    }
}
#[godot_api]
impl PixelSunShafts {
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if gate(&self.to_gd(), Feature::SunShafts) {
            self.enact();
        }
    }
    #[func]
    fn get_sun(&self) -> Option<Gd<DirectionalLight3D>> {
        self.sun.clone().filter(|s| s.is_instance_valid())
    }
    #[func]
    fn set_sun(&mut self, v: Option<Gd<DirectionalLight3D>>) {
        self.sun = v;
        self.enact();
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    fn assign_tally(&mut self, v: i32) {
        self.count = v.clamp(1, 64);
        self.enact();
    }
    #[func]
    fn set_depth(&mut self, v: f32) {
        self.depth = v.max(0.0);
        self.enact();
    }
    #[func]
    fn assign_measure(&mut self, v: f32) {
        self.size = v.max(1.0);
        self.enact();
    }
    #[func]
    fn set_follow_camera(&mut self, v: bool) {
        self.follow_camera = v;
        self.enact();
    }
    #[func]
    fn assign_beam_tint(&mut self, v: Color) {
        self.beam_tint = v;
        self.enact();
    }
    #[func]
    fn set_alpha(&mut self, v: f32) {
        self.alpha = v.clamp(0.0, 1.0);
        self.enact();
    }
    #[func]
    fn set_start_depth(&mut self, v: f32) {
        self.start_depth = v.max(0.0);
        self.enact();
    }
    #[func]
    fn set_shaft_steps(&mut self, v: i32) {
        self.shaft_steps = v.clamp(0, 8);
        self.enact();
    }
    #[func]
    fn set_dither_width(&mut self, v: f32) {
        self.dither_width = v.clamp(0.0, 1.0);
        self.enact();
    }
    #[func]
    fn set_edge_fade(&mut self, v: f32) {
        self.edge_fade = v.clamp(0.0, 1.0);
        self.enact();
    }
    #[func]
    fn set_surface_fade(&mut self, v: f32) {
        self.surface_fade = v.max(0.0);
        self.enact();
    }
    #[func]
    fn set_fade_height(&mut self, v: f32) {
        self.fade_height = v.max(0.0);
        self.enact();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sheets_contain_the_sun_direction_and_span_the_depth() {
        let sun = Basis::from_euler(EulerOrder::YXZ, Vector3::new(-0.9, 0.5, 0.0));
        let xf = sheet_transforms(sun, 4, 40.0);
        assert_eq!(xf.len(), 4);
        let fwd = -sun.col_c();
        let up = sun.col_b();
        for t in &xf {
            assert!((t.basis.col_c() - up).length() < 1e-4);
            assert!(t.basis.col_c().dot(fwd).abs() < 1e-4);
        }
        let offs: Vec<f32> = xf.iter().map(|t| -t.origin.dot(up)).collect();
        for (o, e) in offs.iter().zip([-15.0, -5.0, 5.0, 15.0]) {
            assert!((o - e).abs() < 1e-3, "{offs:?}");
        }
    }
}
