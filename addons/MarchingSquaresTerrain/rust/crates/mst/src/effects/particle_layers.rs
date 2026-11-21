//! Draws a field of particles as stacked animated sheets that follow the camera.
//!
//! The sheets are one internal mesh node the owning effect builds for itself when it enters
//! the tree and drops when it leaves, so the effect's scene only records the effect and
//! never its drawing parts.

use super::camera_view::{active_camera, camera_focus};
use crate::globals;
use crate::node_children::{add_internal, free_node};
use crate::shader_materials::shader_material;
use crate::writes::ShaderParams;
use godot::classes::camera_3d::ProjectionType;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::image::Format;
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::{
    ArrayMesh, Camera3D, Image, ImageTexture, MeshInstance3D, Node, ShaderMaterial,
};
use godot::prelude::*;
use pt_core::flipbook;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Projection {
    Ground = 0,
    Upright = 1,
}
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub projection: Projection,
    pub layers: i32,
    pub spacing: f32,
    pub texel: f32,
    pub extent: f32,
    pub base_y: f32,
    pub stature: f32,
    pub fall_speed: f32,
    pub wind: Vector2,
    pub slant: f32,
    pub gust: f32,
    pub band: f32,
    pub fps: f32,
    pub tint: Color,
    pub color_b: Color,
    pub brightness: f32,
    pub amount: f32,
    pub lit: f32,
    pub beam: f32,
    pub shade: f32,
    pub night: f32,
    pub focus_radius: f32,
    pub fade_center: Vector3,
}
#[derive(Default)]
pub struct ParticleLayers {
    mi: Option<Gd<MeshInstance3D>>,
    mat: Option<Gd<ShaderMaterial>>,
    tex: Option<Gd<ImageTexture>>,
    baked: Option<flipbook::Params>,
    mesh_layers: i32,
}
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
pub fn sheet_texture(b: &flipbook::Flipbook) -> Option<Gd<ImageTexture>> {
    let img = Image::create_from_data(
        b.breadth() as i32,
        b.stature() as i32,
        false,
        Format::RGBA8,
        &PackedByteArray::from(b.rgba.as_slice()),
    )?;
    ImageTexture::create_from_image(&img)
}
impl ParticleLayers {
    pub fn link(&mut self, parent: &mut Gd<Node>, name: &str) {
        if self.mi.as_ref().is_some_and(|m| m.is_instance_valid()) {
            return;
        }
        let mut mi = MeshInstance3D::new_alloc();
        mi.set_name(name);
        mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        let layers = mi.get_layer_mask() & !globals::meta_mask();
        mi.set_layer_mask(layers);
        mi.set_as_top_level(true);
        crate::writes::set_global_transform(&mut mi, Transform3D::IDENTITY);
        mi.set_ignore_occlusion_culling(true);
        mi.set_custom_aabb(Aabb::new(Vector3::splat(-1.0e5), Vector3::splat(2.0e5)));
        let mat = self
            .mat
            .get_or_insert_with(|| shader_material("particle_field.gdshader"))
            .clone();
        mi.set_material_override(&mat);
        mi.set_visible(false);
        add_internal(parent, &mi);
        self.mi = Some(mi);
        self.mesh_layers = 0;
    }
    pub fn unlink(&mut self) {
        free_node(self.mi.take());
        self.mesh_layers = 0;
    }
    pub fn set_flipbook(&mut self, p: &flipbook::Params) {
        if self.baked.as_ref() == Some(p) && self.tex.is_some() {
            return;
        }
        let b = flipbook::bake(p);
        self.tex = sheet_texture(&b);
        self.baked = Some(*p);
        let mat = self
            .mat
            .get_or_insert_with(|| shader_material("particle_field.gdshader"));
        if let Some(t) = &self.tex {
            mat.param("sheet", &t.to_variant());
        }
        mat.param(
            "sheet_grid",
            &Vector2i::new(b.cols as i32, b.ranks as i32).to_variant(),
        );
        mat.param("tile", &(b.size as i32).to_variant());
        mat.param("frames", &(b.frames as i32).to_variant());
    }
    pub fn texture(&self) -> Option<Gd<ImageTexture>> {
        self.tex.clone()
    }
    pub fn assign_shown(&mut self, on: bool) {
        if let Some(mi) = &mut self.mi
            && mi.is_instance_valid()
            && mi.is_visible() != on
        {
            mi.set_visible(on);
        }
    }
    pub fn update(&mut self, node: &Gd<Node>, look: &Look) {
        let Some(mut mi) = self.mi.clone().filter(|m| m.is_instance_valid()) else {
            return;
        };
        let on = look.amount > 0.0015 && look.layers > 0;
        self.assign_shown(on);
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
        let focus = cam
            .as_ref()
            .map(|c| camera_focus(c, plane_y))
            .unwrap_or(Vector3::new(0.0, plane_y, 0.0));
        let mut extent = if look.extent > 0.0 {
            look.extent
        } else {
            cam.as_ref().map(auto_extent).unwrap_or(40.0)
        };
        let mut focus = focus;
        if look.focus_radius > 0.0 {
            extent = extent.min(look.focus_radius * 1.1005 + 1.0);
            focus = Vector3::new(look.fade_center.x, focus.y, look.fade_center.z);
        }
        let Some(m) = &mut self.mat else { return };
        let set = |m: &mut Gd<ShaderMaterial>, k: &'static str, v: Variant| m.param(k, &v);
        set(m, "mode", (look.projection as i32).to_variant());
        set(m, "layers", n.to_variant());
        set(m, "spacing", look.spacing.max(0.0105).to_variant());
        set(m, "texel_size", look.texel.max(0.0015).to_variant());
        set(m, "extent", extent.to_variant());
        set(m, "base_y", look.base_y.to_variant());
        set(m, "height", look.stature.max(0.1005).to_variant());
        set(m, "focus", focus.to_variant());
        set(m, "fall_speed", look.fall_speed.to_variant());
        set(m, "wind", look.wind.to_variant());
        set(m, "slant", look.slant.to_variant());
        set(m, "gust", look.gust.to_variant());
        set(m, "band", look.band.max(0.0).to_variant());
        set(m, "fps", look.fps.max(0.0).to_variant());
        set(m, "color", look.tint.to_variant());
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
fn auto_extent(cam: &Gd<Camera3D>) -> f32 {
    let aspect = cam
        .get_viewport()
        .map(|v| v.get_visible_rect().size)
        .map(|s| s.x / s.y.max(1.0))
        .unwrap_or(16.0 / 9.0)
        .max(1.0);
    let e = if cam.get_projection() == ProjectionType::ORTHOGONAL {
        cam.get_size() * 0.5 * aspect * 1.6005
    } else {
        let t = cam.get_global_transform();
        let fwd = -t.basis.col_c();
        let d = if fwd.y < -0.0505 {
            (20.0 / -fwd.y).max(10.0) + t.origin.y.abs() / -fwd.y
        } else {
            60.0
        };
        d * (cam.get_fov().to_radians() * 0.5).tan() * aspect * 1.8005 + 4.0
    };
    e.clamp(4.0, 250.0)
}
