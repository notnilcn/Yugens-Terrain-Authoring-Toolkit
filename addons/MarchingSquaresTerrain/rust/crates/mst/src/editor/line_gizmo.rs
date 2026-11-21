//! The line overlay that the terrain editor's region tools draw over the ground to show
//! what they will affect.
//!
//! The overlay node is created in code and parented to the edited terrain as an internal
//! child because it belongs to a tool session, not to the scene: it appears when a tool
//! first needs to show something and is released when the tool is put down, and it must
//! never be saved with the terrain.
use crate::node_children::add_internal;
use crate::terrain::MarchingSquaresTerrain;
use godot::classes::base_material_3d::{Flags, ShadingMode, Transparency};
use godot::classes::mesh::PrimitiveType;
use godot::classes::{ImmediateMesh, Material, MeshInstance3D, StandardMaterial3D};
use godot::prelude::*;
pub(super) struct LineGizmo {
    node: Gd<MeshInstance3D>,
    cage: Gd<ImmediateMesh>,
    mat: Gd<StandardMaterial3D>,
}
impl LineGizmo {
    pub(super) fn new(terrain: &mut Gd<MarchingSquaresTerrain>) -> LineGizmo {
        let mut node = MeshInstance3D::new_alloc();
        node.set_name("MarchingSquaresTerrainToolGizmo");
        let mesh = ImmediateMesh::new_gd();
        node.set_mesh(&mesh);
        node.set_cast_shadows_setting(
            godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF,
        );
        node.set_layer_mask(1);
        add_internal(terrain, &node);
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_shading_mode(ShadingMode::UNSHADED);
        mat.set_flag(Flags::DISABLE_DEPTH_TEST, true);
        mat.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
        mat.set_transparency(Transparency::ALPHA);
        mat.set_render_priority(14);
        LineGizmo {
            node,
            cage: mesh,
            mat,
        }
    }
    pub(super) fn valid(&self) -> bool {
        self.node.is_instance_valid()
    }
    pub(super) fn draw(&mut self, lines: &[(Vector3, Vector3, Color)]) {
        self.cage.clear_surfaces();
        if lines.is_empty() {
            return;
        }
        self.cage
            .surface_begin_ex(PrimitiveType::LINES)
            .material(&self.mat.clone().upcast::<Material>())
            .done();
        for (a, b, c) in lines {
            self.cage.surface_set_color(*c);
            self.cage.surface_add_vertex(*a);
            self.cage.surface_set_color(*c);
            self.cage.surface_add_vertex(*b);
        }
        self.cage.surface_end();
    }
    pub(super) fn assign_shown(&mut self, v: bool) {
        if self.valid() {
            self.node.set_visible(v);
        }
    }
    pub(super) fn free(mut self) {
        if self.valid() {
            self.node.queue_free();
        }
    }
}
