//! The Studio-style Part section at the top of an editable mesh's inspector in the Roblox
//! layout.
//!
//! Intent: someone used to Studio expects to select a part, or several, and read or type
//! colour, material, size, position, orientation and part flags in one familiar block, and
//! to see those values follow along while they drag handles or undo, without reselecting.
//! With several parts selected the block shows the first one and every edit applies to all.
//!
//! Responsibility: it keeps the displayed values true to the inspected meshes, and decides
//! which of them can be edited for them, such as a size that only a mesh still made
//! from its recipe can take. It never changes the mesh itself: every edit typed into it is
//! handed to the builder, which records it as an undoable step.
//!
//! Why a node: the section lives exactly as long as the inspector shows that mesh, and the
//! inspector throws its controls away on every new selection. Being in the tree ties the
//! refresh to that lifetime, including the periodic look at transforms that change without
//! any notification from the mesh data.
//!
//! Why a new type: a built-in container can hold the fields but cannot read a builder mesh
//! and its recipe to fill them, and the existing mesh summary in the inspector reports on
//! the mesh as a whole rather than offering per-part fields.
//!
//! Not exported: the meshes it shows, which the inspector hands it each time the section is
//! built, because they are the inspected selection and not part of the authored section.
use super::super::data::PixelBuilderMeshData;
use super::super::node::PixelBuilderMesh;
use super::super::plugin::{EDIT_LOCK_META, PLUGIN_LOCK_META};
use super::super::ui::Action;
use super::part_rules::{self, PartField};
use godot::classes::{
    Button, CheckBox, ColorPickerButton, IVBoxContainer, Label, OptionButton, SpinBox, Timer,
    VBoxContainer,
};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = VBoxContainer)]
pub struct PixelPartInspector {
    nodes: Vec<Gd<PixelBuilderMesh>>,
    #[export]
    refresh: Option<Gd<Timer>>,
    #[export_group(name = "Appearance")]
    #[export]
    part_color: Option<Gd<ColorPickerButton>>,
    #[export]
    part_material: Option<Gd<Button>>,
    #[export]
    part_transparency: Option<Gd<SpinBox>>,
    #[export_group(name = "Transform")]
    #[export]
    size_x: Option<Gd<SpinBox>>,
    #[export]
    size_y: Option<Gd<SpinBox>>,
    #[export]
    size_z: Option<Gd<SpinBox>>,
    #[export]
    position_x: Option<Gd<SpinBox>>,
    #[export]
    position_y: Option<Gd<SpinBox>>,
    #[export]
    position_z: Option<Gd<SpinBox>>,
    #[export]
    orientation_x: Option<Gd<SpinBox>>,
    #[export]
    orientation_y: Option<Gd<SpinBox>>,
    #[export]
    orientation_z: Option<Gd<SpinBox>>,
    #[export]
    pivot_x: Option<Gd<SpinBox>>,
    #[export]
    pivot_y: Option<Gd<SpinBox>>,
    #[export]
    pivot_z: Option<Gd<SpinBox>>,
    #[export_group(name = "Part")]
    #[export]
    part_shape: Option<Gd<Label>>,
    #[export]
    part_locked: Option<Gd<CheckBox>>,
    #[export]
    part_anchored: Option<Gd<CheckBox>>,
    #[export_group(name = "Behavior")]
    #[export]
    part_collision: Option<Gd<OptionButton>>,
    base: Base<VBoxContainer>,
}
fn typing_in(s: &Gd<SpinBox>) -> bool {
    s.get_line_edit().is_some_and(|l| l.has_focus())
}
fn show_value(s: &mut Gd<SpinBox>, v: f32) {
    if !typing_in(s) && (s.get_value() - v as f64).abs() > 1e-4 {
        s.set_value_no_signal(v as f64);
    }
}
impl PixelPartInspector {
    pub fn attach(&mut self, nodes: Vec<Gd<PixelBuilderMesh>>) {
        self.nodes = nodes;
    }
    fn shown(&self) -> Option<Gd<PixelBuilderMesh>> {
        self.nodes.iter().find(|n| n.is_instance_valid()).cloned()
    }
    fn spins(&self) -> Vec<(PartField, Gd<SpinBox>)> {
        [
            (PartField::SizeX, &self.size_x),
            (PartField::SizeY, &self.size_y),
            (PartField::SizeZ, &self.size_z),
            (PartField::PositionX, &self.position_x),
            (PartField::PositionY, &self.position_y),
            (PartField::PositionZ, &self.position_z),
            (PartField::OrientationX, &self.orientation_x),
            (PartField::OrientationY, &self.orientation_y),
            (PartField::OrientationZ, &self.orientation_z),
            (PartField::Transparency, &self.part_transparency),
        ]
        .into_iter()
        .filter_map(|(f, s)| s.clone().map(|s| (f, s)))
        .collect()
    }
    pub fn wire(&self, plugin: &Gd<Object>) {
        let on = |m: &str| Callable::from_object_method(plugin, m);
        let parts: Array<Gd<PixelBuilderMesh>> = self.nodes.iter().cloned().collect();
        let parts = parts.to_variant();
        for (f, mut s) in self.spins() {
            s.connect(
                "value_changed",
                &on("on_part_field").bind(&[(f as i64).to_variant(), parts.clone()]),
            );
        }
        if let Some(mut c) = self.part_color.clone() {
            c.connect("color_changed", &on("on_part_color").bind(std::slice::from_ref(&parts)));
        }
        if let Some(mut o) = self.part_collision.clone() {
            o.connect("item_selected", &on("on_part_collision").bind(std::slice::from_ref(&parts)));
        }
        let deed = |a: Action| on("upon_deed").bind(&[(a as i64).to_variant()]);
        if let Some(mut b) = self.part_material.clone() {
            b.connect("pressed", &deed(Action::MaterialPicker));
        }
        if let Some(mut b) = self.part_locked.clone() {
            b.connect("pressed", &deed(Action::ToggleLock));
        }
        if let Some(mut b) = self.part_anchored.clone() {
            b.connect("pressed", &deed(Action::ToggleAnchor));
        }
    }
    fn data(&self) -> Option<Gd<PixelBuilderMeshData>> {
        self.shown().and_then(|n| n.bind().contents())
    }
}
#[godot_api]
impl IVBoxContainer for PixelPartInspector {
    fn ready(&mut self) {
        if let Some(mut t) = self.refresh.clone() {
            t.connect("timeout", &self.to_gd().callable("renew"));
        }
        self.renew();
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        crate::editor::scene_refs::unassigned_warnings(&self.to_gd().upcast())
    }
}
#[godot_api]
impl PixelPartInspector {
    #[func]
    pub fn renew(&mut self) {
        let Some(node) = self.shown() else {
            return;
        };
        let data = self.data();
        let (spec, extent, first) = data
            .as_ref()
            .map(|d| {
                let b = d.bind();
                let m = b.cage();
                (
                    b.shape_spec().filter(|s| part_rules::regenerable(s, m)),
                    m.extent(),
                    m.facets.first().map(|f| (f.stuff, f.tint)),
                )
            })
            .unwrap_or((None, None, None));
        let size = match (&spec, extent) {
            (Some(s), _) => [s.size.x, s.size.y, s.size.z],
            (None, Some((lo, hi))) => part_rules::extent_size(lo, hi),
            _ => [0.0; 3],
        };
        let pivot = extent
            .map(|(lo, hi)| {
                let c = part_rules::centre(lo, hi);
                [-c.x, -c.y, -c.z]
            })
            .unwrap_or([0.0; 3]);
        let pos = node.get_position();
        let rot = node.get_rotation_degrees();
        let transparency = node.get_transparency();
        for (f, mut s) in self.spins() {
            let v = match f {
                PartField::SizeX | PartField::SizeY | PartField::SizeZ => {
                    s.set_editable(spec.is_some());
                    s.set_tooltip_text(if spec.is_some() {
                        "Size of the part's recipe: editing it rebuilds the mesh"
                    } else {
                        "Read only: this mesh is no longer its recipe's shape, so the size is its bounding box"
                    });
                    size[f.axis()]
                }
                PartField::PositionX | PartField::PositionY | PartField::PositionZ => {
                    [pos.x, pos.y, pos.z][f.axis()]
                }
                PartField::OrientationX | PartField::OrientationY | PartField::OrientationZ => {
                    [rot.x, rot.y, rot.z][f.axis()]
                }
                PartField::Transparency => transparency,
            };
            show_value(&mut s, v);
        }
        for (k, s) in [&self.pivot_x, &self.pivot_y, &self.pivot_z].into_iter().enumerate() {
            if let Some(mut s) = s.clone() {
                show_value(&mut s, pivot[k]);
            }
        }
        if let Some((slot, tint)) = first {
            if let Some(c) = &mut self.part_color
                && !c.get_popup().is_some_and(|p| p.is_visible())
            {
                c.set_pick_color(Color::from_rgba8(tint[0], tint[1], tint[2], tint[3]));
            }
            if let Some(b) = &mut self.part_material {
                let name = data
                    .as_ref()
                    .and_then(|d| d.bind().stuff_pockets().get(slot as usize).flatten())
                    .map(|m| super::pickers::material_name(&m))
                    .unwrap_or_else(|| "(default)".to_string());
                if b.get_text().to_string() != name {
                    b.set_text(&name);
                }
            }
        }
        if let Some(l) = &mut self.part_shape {
            let text = data
                .as_ref()
                .and_then(|d| d.bind().shape_spec())
                .map(|s| s.sort.moniker().to_string())
                .unwrap_or_else(|| "Mesh".to_string());
            if l.get_text().to_string() != text {
                l.set_text(&text);
            }
        }
        let locked = node.has_meta(EDIT_LOCK_META) && !node.has_meta(PLUGIN_LOCK_META);
        if let Some(b) = &mut self.part_locked {
            b.set_pressed_no_signal(locked);
        }
        if let Some(b) = &mut self.part_anchored {
            b.set_pressed_no_signal(node.get("anchored").try_to::<bool>().unwrap_or(false));
        }
        if let Some(o) = &mut self.part_collision {
            let kind = node.get("entity_type").try_to::<i32>().unwrap_or(0);
            if o.get_selected() != kind {
                o.select(kind);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    const SCENE: &str = include_str!("../../../../../../editor/builder/part_inspector.tscn");
    #[test]
    fn every_assigned_control_is_declared() {
        assert!(SCENE.contains("type=\"PixelPartInspector\""));
        assert_eq!(crate::editor::scene_refs::unresolved_root_refs(SCENE), Vec::<String>::new());
    }
}
