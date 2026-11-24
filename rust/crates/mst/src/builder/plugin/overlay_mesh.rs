//! Builds the see-through mesh nodes the builder draws its own feedback into while a mesh
//! is being edited: the selection gizmo and the smoothing and normals overlay.
//!
//! They are created in code because they exist only for an edit session. They appear when
//! a builder mesh is picked for editing, hang off that mesh as internal children so they
//! follow its transform, and are freed when editing stops. They are never saved, so no
//! scene could declare them.
use super::super::node::PixelBuilderMesh;
use crate::internal_children::add_internal;
use godot::classes::base_material_3d::{CullMode, Flags, ShadingMode, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::{ImmediateMesh, MeshInstance3D, StandardMaterial3D};
use godot::prelude::*;
pub(super) fn attach_overlay_mesh(
    target: &mut Gd<PixelBuilderMesh>,
    name: &str,
    render_priority: i32,
) -> Gd<MeshInstance3D> {
    let mut mi = MeshInstance3D::new_alloc();
    mi.set_name(name);
    mi.set_mesh(&ImmediateMesh::new_gd());
    let mut mat = StandardMaterial3D::new_gd();
    mat.set_shading_mode(ShadingMode::UNSHADED);
    mat.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
    mat.set_transparency(Transparency::ALPHA);
    mat.set_cull_mode(CullMode::DISABLED);
    mat.set_render_priority(render_priority);
    mi.set_material_override(&mat);
    mi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    mi.set_layer_mask(1);
    mi.add_to_group("pixel_ignore");
    add_internal(target, &mi);
    mi
}
