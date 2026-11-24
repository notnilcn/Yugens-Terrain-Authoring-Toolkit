//! The live summary at the top of an editable mesh's inspector section.
//!
//! Intent: an author inspecting an editable mesh should see at a glance how big it is,
//! how many materials it uses, whether it carries lightmap coordinates and what role it
//! plays in the game, and should keep seeing the truth while they model, without
//! reselecting the mesh to refresh the panel.
//!
//! Responsibility: it owns keeping those read-outs current. It reads the mesh and its
//! editable data and rewrites the captions when they change. It does not lay out the
//! section, own the captions' look, or perform any of the actions offered beside it;
//! the inspector section declares the layout and the builder plugin runs the actions.
//!
//! Why a node: the summary lives exactly as long as the inspector section it belongs to,
//! and the inspector discards that section whenever the selection changes. Living in the
//! tree ties the refresh to that lifetime, so the change notifications it listens to die
//! with the panel instead of outliving it.
//!
//! Why a new type: a built-in container can hold the captions but cannot answer the
//! mesh data's change notification by recomputing them, and the builder plugin needs one
//! object it can ask to refresh after edits that do not pass through that notification.
//!
//! Not exported: the mesh it reports on, which the inspector hands it each time the
//! section is built, and a running count of refreshes that only exists to observe it at
//! run time.

use super::node::PixelBuilderMesh;
use godot::classes::{ArrayMesh, IVBoxContainer, Label, VBoxContainer};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(tool, init, base = VBoxContainer)]
pub struct PixelBuilderInspectorStats {
    node: Option<Gd<PixelBuilderMesh>>,
    #[export]
    counts_caption: Option<Gd<Label>>,
    #[export]
    slots_caption: Option<Gd<Label>>,
    #[export]
    uv2_caption: Option<Gd<Label>>,
    #[export]
    collision_caption: Option<Gd<Label>>,
    refreshes: i64,
    base: Base<VBoxContainer>,
}

impl PixelBuilderInspectorStats {
    pub fn attach(&mut self, node: Gd<PixelBuilderMesh>) {
        self.node = Some(node);
    }
}

#[godot_api]
impl IVBoxContainer for PixelBuilderInspectorStats {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        [
            (
                &self.counts_caption,
                "Assign the caption that shows the element counts.",
            ),
            (
                &self.slots_caption,
                "Assign the caption that shows the material slots.",
            ),
            (
                &self.uv2_caption,
                "Assign the caption that shows the lightmap UVs.",
            ),
            (
                &self.collision_caption,
                "Assign the caption that shows the collision role.",
            ),
        ]
        .into_iter()
        .filter(|(caption, _)| caption.is_none())
        .map(|(_, warning)| GString::from(warning))
        .collect()
    }
}

#[godot_api]
impl PixelBuilderInspectorStats {
    #[func]
    pub fn renew(&mut self) {
        let Some(node) = self.node.clone().filter(|n| n.is_instance_valid()) else {
            return;
        };
        self.refreshes += 1;
        let data = node.bind().contents();
        if let Some(d) = &data {
            let db = d.bind();
            let m = db.cage();
            let text = format!(
                "{} vertices, {} edges, {} faces",
                m.positions.len(),
                m.ridges().len(),
                m.facets.len()
            );
            let n = db.stuff_pockets().len();
            drop(db);
            assign_caption(&mut self.counts_caption, &text);
            assign_caption(
                &mut self.slots_caption,
                &format!("{n} material slot{}", if n == 1 { "" } else { "s" }),
            );
        }
        let uv2 = node.get_mesh().is_some_and(|m| {
            m.get_surface_count() > 0
                && m.clone().try_cast::<ArrayMesh>().is_ok_and(|a| {
                    (a.surface_get_format(0).ord()
                        & godot::classes::mesh::ArrayFormat::TEX_UV2.ord())
                        != 0
                })
        });
        let live = node.get("live_uv2").try_to::<bool>().unwrap_or(false);
        assign_caption(
            &mut self.uv2_caption,
            &format!(
                "Lightmap UVs: {}",
                if uv2 || live { "present" } else { "none" }
            ),
        );
        let kind = match node.get("entity_type").try_to::<i32>().unwrap_or(0) {
            1 => "Collider",
            2 => "Trigger",
            _ => "Detail",
        };
        assign_caption(&mut self.collision_caption, &format!("Current: {kind}"));
    }

    #[func]
    pub fn debug_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let t = |l: &Option<Gd<Label>>| {
            l.as_ref()
                .filter(|l| l.is_instance_valid())
                .map(|l| l.get_text())
                .unwrap_or_default()
        };
        d.set("counts", &t(&self.counts_caption));
        d.set("slots", &t(&self.slots_caption));
        d.set("uv2", &t(&self.uv2_caption));
        d.set("collision", &t(&self.collision_caption));
        d.set("refreshes", self.refreshes);
        d
    }
}

fn assign_caption(l: &mut Option<Gd<Label>>, caption: &str) {
    if let Some(l) = l.as_mut().filter(|l| l.is_instance_valid())
        && l.get_text().to_string() != caption
    {
        l.set_text(caption);
    }
}
