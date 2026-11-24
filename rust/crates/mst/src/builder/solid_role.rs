//! The roles a builder mesh can take in solid modelling, kept as node metadata so a part
//! stays an ordinary mesh that the scene saves and the undo history restores like any
//! other property.
use super::solid_rules::PartRole;
use godot::classes::base_material_3d::{CullMode, ShadingMode, Transparency};
use godot::classes::{Material, StandardMaterial3D};
use godot::prelude::*;
pub const NEGATE_META: &str = "mst_negate";
pub const OPERAND_META: &str = "mst_operand";
pub const DISPLAY_TINT_META: &str = "pt_entity_tint";
pub fn role_of(node: &Gd<Node>) -> PartRole {
    PartRole {
        negated: node.has_meta(NEGATE_META),
        operand: node.has_meta(OPERAND_META),
    }
}
pub fn is_negated(node: &Gd<Node>) -> bool {
    role_of(node).negated
}
pub fn is_operand(node: &Gd<Node>) -> bool {
    role_of(node).operand
}
pub fn takes_no_part_in_world(node: &Gd<Node>) -> bool {
    role_of(node).takes_no_part_in_world()
}
pub fn ghost_material(fill_tint: Color, edge_tint: Color, edge_grow: f32) -> Gd<Material> {
    let mut edge = StandardMaterial3D::new_gd();
    edge.set_shading_mode(ShadingMode::UNSHADED);
    edge.set_transparency(Transparency::ALPHA);
    edge.set_cull_mode(CullMode::FRONT);
    edge.set_grow_enabled(true);
    edge.set_grow(edge_grow);
    edge.set_albedo(edge_tint);
    let mut fill = StandardMaterial3D::new_gd();
    fill.set_transparency(Transparency::ALPHA);
    fill.set_cull_mode(CullMode::DISABLED);
    fill.set_albedo(fill_tint);
    fill.set_next_pass(&edge);
    fill.set_meta(DISPLAY_TINT_META, &true.to_variant());
    fill.upcast()
}
