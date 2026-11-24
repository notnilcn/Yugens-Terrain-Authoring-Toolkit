//! Adds the builder's own sections to the inspector of meshes the builder can work on.
//!
//! The sections are instantiated from their scenes in code because the inspector discards
//! and rebuilds its controls for every object it shows, so fresh sections are needed each
//! time a mesh is inspected, and which sections appear depends on the inspected object and
//! on the builder layout the person editing has chosen. The scenes decide what each section
//! contains; this file only decides which sections appear, which of their parts apply to
//! the inspected object, and wires their controls to the builder plugin that is handed in
//! at runtime.

mod section;

use super::inspector_stats::PixelBuilderInspectorStats;
use super::layout::Layout;
use super::node::PixelBuilderMesh;
use super::roblox_ui::part_inspector::PixelPartInspector;
use super::ui::Action;
use godot::classes::{
    BaseButton, CsgShape3D, EditorInspectorPlugin, IEditorInspectorPlugin, Node, Object,
    PackedScene,
};
use godot::obj::Inherits;
use godot::prelude::*;
pub use section::PixelBuilderInspectorSection;

const SECTION_SCENE: &str =
    "res://addons/MarchingSquaresTerrain/editor/builder/editable_mesh_inspector.tscn";
const PART_SCENE: &str = "res://addons/MarchingSquaresTerrain/editor/builder/part_inspector.tscn";

#[derive(GodotClass)]
#[class(tool, init, base = EditorInspectorPlugin)]
pub struct PixelBuilderInspector {
    pub plugin: Option<Gd<Object>>,
    pub stats: Option<Gd<PixelBuilderInspectorStats>>,
    base: Base<EditorInspectorPlugin>,
}

fn discard<T: Inherits<Node>>(part: Option<Gd<T>>) {
    if let Some(n) = part {
        n.upcast::<Node>().free();
    }
}

fn instantiate<T: Inherits<Node>>(path: &str) -> Option<Gd<T>> {
    let scene = try_load::<PackedScene>(path)
        .inspect_err(|e| godot_warn!("Pixel Builder inspector: cannot load {path}: {e}"))
        .ok()?;
    match scene.instantiate()?.try_cast::<T>() {
        Ok(root) => Some(root),
        Err(n) => {
            godot_warn!("Pixel Builder inspector: {path} has an unexpected root type");
            n.free();
            None
        }
    }
}

fn wire<T: Inherits<BaseButton>>(button: Option<Gd<T>>, plugin: &Gd<Object>, a: Action) {
    if let Some(b) = button {
        b.upcast::<BaseButton>().connect(
            "pressed",
            &Callable::from_object_method(plugin, "upon_deed").bind(&[(a as i64).to_variant()]),
        );
    }
}

fn part_section(nodes: Vec<Gd<PixelBuilderMesh>>, plugin: &Gd<Object>) -> Option<Gd<PixelPartInspector>> {
    let mut part = instantiate::<PixelPartInspector>(PART_SCENE)?;
    part.bind_mut().attach(nodes);
    part.bind().wire(plugin);
    Some(part)
}

fn selected_parts(entity: &Gd<Object>) -> Option<Vec<Gd<PixelBuilderMesh>>> {
    if !entity.is_class("MultiNodeEdit") || Layout::ingest() != Layout::Roblox {
        return None;
    }
    let selection = godot::classes::EditorInterface::singleton().get_selection()?;
    let nodes = selection.get_selected_nodes();
    let parts: Vec<Gd<PixelBuilderMesh>> = nodes
        .iter_shared()
        .filter_map(|n| n.try_cast::<PixelBuilderMesh>().ok())
        .collect();
    (!parts.is_empty() && parts.len() == nodes.len()).then_some(parts)
}

#[godot_api]
impl IEditorInspectorPlugin for PixelBuilderInspector {
    fn can_handle(&self, entity: Option<Gd<Object>>) -> bool {
        let Some(o) = entity else { return false };
        if crate::editor::pixel_handles(&o) {
            return false;
        }
        let csg_root = o
            .clone()
            .try_cast::<CsgShape3D>()
            .is_ok_and(|c| c.is_root_shape());
        o.clone().try_cast::<PixelBuilderMesh>().is_ok()
            || o.is_class("MeshInstance3D")
            || csg_root
            || selected_parts(&o).is_some()
    }
    fn parse_begin(&mut self, entity: Option<Gd<Object>>) {
        let (Some(o), Some(plugin)) = (entity, self.plugin.clone()) else {
            return;
        };
        if let Some(parts) = selected_parts(&o) {
            if let Some(part) = part_section(parts, &plugin) {
                self.base_mut().add_custom_control(&part);
            }
            return;
        }
        let Some(root) = instantiate::<PixelBuilderInspectorSection>(SECTION_SCENE) else {
            return;
        };
        let s = root.bind();
        if let Some(mut title) = s.title.clone()
            && let Some(f) = godot::classes::EditorInterface::singleton()
                .get_editor_theme()
                .and_then(|t| t.get_font("bold", "EditorFonts"))
        {
            title.add_theme_font_override("font", &f);
        }
        match o.clone().try_cast::<PixelBuilderMesh>() {
            Err(_) => {
                discard(s.mesh_fold.clone());
                discard(s.recipe_fold.clone());
                discard(s.uv_lightmap_fold.clone());
                discard(s.collision_fold.clone());
                discard(s.export_data_fold.clone());
                wire(s.make_editable.clone(), &plugin, Action::Convert);
            }
            Ok(node) => {
                if Layout::ingest() == Layout::Roblox
                    && let Some(part) = part_section(vec![node.clone()], &plugin)
                {
                    self.base_mut().add_custom_control(&part);
                }
                discard(s.not_editable.clone());
                let data = node.bind().contents();
                let (shape, bezier) = data
                    .as_ref()
                    .map(|d| {
                        (
                            d.bind().shape_spec().is_some(),
                            d.bind().bezier_recipe().is_some(),
                        )
                    })
                    .unwrap_or((false, false));
                if shape || bezier {
                    if bezier {
                        if let Some(mut f) = s.recipe_fold.clone() {
                            f.set_title("Bezier");
                        }
                        if let Some(mut b) = s.edit_recipe.clone() {
                            b.set_text("Edit Bezier Shape");
                        }
                    }
                    wire(s.edit_recipe.clone(), &plugin, Action::ShapeParams);
                } else {
                    discard(s.recipe_fold.clone());
                }
                for (b, a) in [
                    (s.open_uv_editor.clone(), Action::UvEditor),
                    (s.generate_uv2.clone(), Action::GenerateUv2),
                    (s.set_collider.clone(), Action::SetCollider),
                    (s.set_trigger.clone(), Action::SetTrigger),
                    (s.export_native.clone(), Action::ExportRes),
                ] {
                    wire(b, &plugin, a);
                }
                if let Some(mut confirm) = s.strip_confirm.clone() {
                    confirm.connect(
                        "confirmed",
                        &Callable::from_object_method(&plugin, "upon_deed")
                            .bind(&[(Action::BakeMesh as i64).to_variant()]),
                    );
                    if let Some(mut strip) = s.strip.clone() {
                        strip.connect(
                            "pressed",
                            &Callable::from_object_method(&confirm, "popup_centered")
                                .bind(&[Vector2i::ZERO.to_variant()]),
                        );
                    }
                }
                if let Some(mut stats) = s.stats.clone() {
                    {
                        let mut sb = stats.bind_mut();
                        sb.attach(node.clone());
                        sb.renew();
                    }
                    if let Some(mut d) = data {
                        d.connect_flags(
                            "changed",
                            &stats.callable("renew"),
                            godot::classes::object::ConnectFlags::DEFERRED,
                        );
                    }
                    self.stats = Some(stats);
                }
            }
        }
        drop(s);
        self.base_mut().add_custom_control(&root);
    }
}
