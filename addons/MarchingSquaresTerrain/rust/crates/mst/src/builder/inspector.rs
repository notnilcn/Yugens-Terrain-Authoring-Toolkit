//! Inspector "Editable Mesh" section (both layouts): mesh counts, the shape recipe, UV &
//! lightmap, collision and export / strip commands of a builder mesh, or *Make Editable* for a
//! plain MeshInstance3D / root CSG shape.
//!
//! Port of xogot-rust (MIT) `xogot_modeler/src/ui/inspector_plugin.rs` (`can_handle`,
//! `parse_begin` adds the section) and `inspector_section.rs` (title, the not-editable summary
//! + Make Editable, foldable Mesh / Shape / UV & Lightmap / Collision / Export & Data groups,
//! the "Strip Editable Data?" confirmation). Built from the node and its data only: it never
//! binds the builder plugin (Godot calls `parse_begin` from `edit()` paths); buttons call the
//! plugin's `on_action` through `Callable`s.
//!
//! The counts and states (mesh, slots, lightmap UVs, collision) live in a
//! [`PixelBuilderInspectorStats`] that refreshes itself on the data's `changed` signal
//! (deferred) and when the plugin reports a node-state change (`sync_ui`: entity type / Live UV2
//! changed); nothing is polled.

use godot::classes::control::SizeFlags;
use godot::classes::{
    Button, ConfirmationDialog, CsgShape3D, EditorInspectorPlugin, FoldableContainer, IEditorInspectorPlugin, Label, Object, VBoxContainer,
};
use godot::prelude::*;

use super::node::PixelBuilderMesh;
use super::ui::Action;

#[derive(GodotClass)]
#[class(tool, init, base = EditorInspectorPlugin)]
pub struct PixelBuilderInspector {
    /// The builder plugin (only used as a `Callable` target).
    pub plugin: Option<Gd<Object>>,
    /// The stats of the section built last (freed with the inspector; check validity).
    pub stats: Option<Gd<PixelBuilderInspectorStats>>,
    base: Base<EditorInspectorPlugin>,
}

/// The "Mesh" counts of the section plus its lightmap / collision status lines.
#[derive(GodotClass)]
#[class(tool, init, base = VBoxContainer, internal)]
pub struct PixelBuilderInspectorStats {
    node: Option<Gd<PixelBuilderMesh>>,
    counts: Option<Gd<Label>>,
    slots: Option<Gd<Label>>,
    uv2: Option<Gd<Label>>,
    collision: Option<Gd<Label>>,
    /// Refreshes so far (tests: change-driven).
    refreshes: i64,
    base: Base<VBoxContainer>,
}

#[godot_api]
impl PixelBuilderInspectorStats {
    /// Re-reads the node and its data into the labels.
    #[func]
    pub fn refresh(&mut self) {
        let Some(node) = self.node.clone().filter(|n| n.is_instance_valid()) else { return };
        self.refreshes += 1;
        let data = node.bind().data();
        if let Some(d) = &data {
            let db = d.bind();
            let m = db.mesh();
            let text = format!("{} vertices, {} edges, {} faces", m.positions.len(), m.edges().len(), m.faces.len());
            let n = db.material_slots().len();
            drop(db);
            set_label(&mut self.counts, &text);
            set_label(&mut self.slots, &format!("{n} material slot{}", if n == 1 { "" } else { "s" }));
        }
        let uv2 = node.get_mesh().is_some_and(|m| {
            m.get_surface_count() > 0 && m.clone().try_cast::<godot::classes::ArrayMesh>().is_ok_and(|a| (a.surface_get_format(0).ord() & godot::classes::mesh::ArrayFormat::TEX_UV2.ord()) != 0)
        });
        let live = node.get("live_uv2").try_to::<bool>().unwrap_or(false);
        set_label(&mut self.uv2, &format!("Lightmap UVs: {}", if uv2 || live { "present" } else { "none" }));
        let kind = match node.get("entity_type").try_to::<i32>().unwrap_or(0) {
            1 => "Collider",
            2 => "Trigger",
            _ => "Detail",
        };
        set_label(&mut self.collision, &format!("Current: {kind}"));
    }

    /// Test hook: `{counts, slots, uv2, collision, refreshes}`.
    #[func]
    pub fn debug_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let t = |l: &Option<Gd<Label>>| l.as_ref().filter(|l| l.is_instance_valid()).map(|l| l.get_text()).unwrap_or_default();
        d.set("counts", &t(&self.counts));
        d.set("slots", &t(&self.slots));
        d.set("uv2", &t(&self.uv2));
        d.set("collision", &t(&self.collision));
        d.set("refreshes", self.refreshes);
        d
    }
}

fn set_label(l: &mut Option<Gd<Label>>, text: &str) {
    if let Some(l) = l.as_mut().filter(|l| l.is_instance_valid())
        && l.get_text().to_string() != text
    {
        l.set_text(text);
    }
}

fn label(text: &str) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.set_text(text);
    l.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
    l
}

#[godot_api]
impl IEditorInspectorPlugin for PixelBuilderInspector {
    /// Mirrors the builder plugin's `handles` (minus the Pixel World nodes).
    fn can_handle(&self, object: Option<Gd<Object>>) -> bool {
        let Some(o) = object else { return false };
        if crate::editor::pixel_handles(&o) {
            return false;
        }
        let csg_root = o.clone().try_cast::<CsgShape3D>().is_ok_and(|c| c.is_root_shape());
        o.clone().try_cast::<PixelBuilderMesh>().is_ok() || o.is_class("MeshInstance3D") || csg_root
    }

    fn parse_begin(&mut self, object: Option<Gd<Object>>) {
        let (Some(o), Some(plugin)) = (object, self.plugin.clone()) else { return };
        let action = |text: &str, a: Action, tip: &str| {
            let mut b = Button::new_alloc();
            b.set_text(text);
            b.set_tooltip_text(tip);
            b.connect("pressed", &Callable::from_object_method(&plugin, "on_action").bind(&[(a as i64).to_variant()]));
            b
        };
        let mut root = VBoxContainer::new_alloc();
        root.set_name("XogotModelerInspectorSection");
        let mut title = label("Editable Mesh");
        if let Some(f) = godot::classes::EditorInterface::singleton().get_editor_theme().and_then(|t| t.get_font("bold", "EditorFonts")) {
            title.add_theme_font_override("font", &f);
        }
        root.add_child(&title);
        let mut content = VBoxContainer::new_alloc();
        content.set_name("Content");
        let fold = |name: &str| {
            let mut f = FoldableContainer::new_alloc();
            f.set_name(name);
            f.set_title(name);
            let col = VBoxContainer::new_alloc();
            f.add_child(&col);
            (f, col)
        };
        match o.clone().try_cast::<PixelBuilderMesh>() {
            Err(_) => {
                content.add_child(&label("This mesh is not editable yet."));
                content.add_child(&action("Make Editable", Action::Convert, "Convert the selected mesh or CSG shape to an editable mesh."));
            }
            Ok(node) => {
                let data = node.bind().data();
                let (mut fm, mut cm) = fold("Mesh");
                let mut stats = PixelBuilderInspectorStats::new_alloc();
                stats.set_name("Stats");
                let (counts, slots) = (label(""), label(""));
                stats.add_child(&counts);
                stats.add_child(&slots);
                cm.add_child(&stats);
                content.add_child(&fm);
                fm.set_folded(false);
                let recipe = data.as_ref().map(|d| (d.bind().shape_spec().is_some(), d.bind().bezier_recipe().is_some())).unwrap_or((false, false));
                if recipe.0 || recipe.1 {
                    let (f, mut c) = fold(if recipe.1 { "Bezier" } else { "Shape" });
                    c.add_child(&label("Rebuilding replaces the element edits. Undo can restore them."));
                    let text = if recipe.1 { "Edit Bezier Shape" } else { "Edit Shape" };
                    c.add_child(&action(text, Action::ShapeParams, "Reopen the recipe this mesh was drawn from (Object mode): the shape fields rebuild it live"));
                    content.add_child(&f);
                }
                let (f, mut c) = fold("UV & Lightmap");
                let uv2 = label("");
                c.add_child(&uv2);
                c.add_child(&action("Open UV Editor", Action::UvEditor, "Open the builder's UV editor for this mesh"));
                c.add_child(&action("Generate Lightmap UVs", Action::GenerateUv2, "Unwrap lightmap UV2 (Live UV2 on, at the Lightmap texel of the preferences)"));
                content.add_child(&f);
                let (f, mut c) = fold("Collision");
                let collision = label("");
                c.add_child(&collision);
                {
                    let mut sb = stats.bind_mut();
                    sb.node = Some(node.clone());
                    sb.counts = Some(counts);
                    sb.slots = Some(slots);
                    sb.uv2 = Some(uv2);
                    sb.collision = Some(collision);
                }
                stats.bind_mut().refresh();
                // Edits, undo / redo and scripts change the data: refresh then (deferred: the
                // signal fires inside the plugin's edits).
                if let Some(mut d) = data.clone() {
                    d.connect_flags("changed", &stats.callable("refresh"), godot::classes::object::ConnectFlags::DEFERRED);
                }
                self.stats = Some(stats.clone());
                c.add_child(&action("Set Collider", Action::SetCollider, "Toggle: an invisible static collider in the game"));
                c.add_child(&action("Set Trigger", Action::SetTrigger, "Toggle: an invisible Area3D trigger volume in the game"));
                content.add_child(&f);
                let (f, mut c) = fold("Export & Data");
                c.add_child(&action("Export Native Mesh…", Action::ExportRes, "Export the compiled mesh with its materials as a Godot mesh resource (.res / .tres)"));
                let mut strip = Button::new_alloc();
                strip.set_text("Strip Editable Data…");
                strip.set_tooltip_text("Replace the builder mesh by a plain MeshInstance3D (keeps the mesh, drops the editable data; undoable)");
                let mut confirm = ConfirmationDialog::new_alloc();
                confirm.set_title("Strip Editable Data?");
                confirm.set_text("The mesh stays, the editable builder data is removed (undoable).");
                confirm.set_ok_button_text("Strip");
                confirm.connect("confirmed", &Callable::from_object_method(&plugin, "on_action").bind(&[(Action::BakeMesh as i64).to_variant()]));
                strip.connect("pressed", &Callable::from_object_method(&confirm, "popup_centered").bind(&[Vector2i::ZERO.to_variant()]));
                c.add_child(&strip);
                c.add_child(&confirm);
                content.add_child(&f);
            }
        }
        content.set_h_size_flags(SizeFlags::EXPAND_FILL);
        root.add_child(&content);
        root.add_child(&godot::classes::HSeparator::new_alloc());
        self.base_mut().add_custom_control(&root);
    }
}
