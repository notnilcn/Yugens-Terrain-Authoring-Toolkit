//! Right-click context menu on the current selection.
//!
//! Port of xogot-rust (MIT):
//! - `editor_core/src/viewport_menu.rs` (`ModelerViewportMenu::items`: one submenu per titled
//!   command section, untitled utilities inline after a separator; an item re-checks at
//!   activation that the mode, the mesh and the selection are still the ones the menu was built
//!   for and that the command can still run),
//! - `editor_core/src/ux_presentation.rs` (`CommandIntent` sections Select / Build / Separate /
//!   Join / Normals / Move / Object) and `editor_core/src/editor_state.rs` `action_requirement`
//!   (the reason an entry is disabled, shown as its tooltip),
//! - `xogot_modeler/src/plugin_bridge.rs` `handle_context_click` (a right press without
//!   modifiers on a selected element is recorded, a release within `CONTEXT_CLICK_SLOP` px opens
//!   the menu, a longer drag forgets it) and `xogot_modeler/src/shared/gizmo_plugin.rs`
//!   `context_menu_targets_selection` (the element under the pointer must be selected; Object
//!   mode: the edited mesh itself).
//!
//! Right-clicks anywhere else go to Godot (free-look).

use godot::classes::{Camera3D, EditorInterface, PopupMenu};
use godot::prelude::*;

use pt_core::build::pick;

use super::super::shortcuts;
use super::super::ui::{Action, Mode};
use super::PixelBuilderPlugin;

/// `CONTEXT_CLICK_SLOP`: px a right press may move and still open the menu.
pub const CLICK_SLOP: f32 = 4.0;

/// Menu-only commands (`ui::MENU_ACTIONS`; Select ring also has a toolbar button).
pub const SELECT_ALL: i64 = Action::SelectAll as i64;
pub const CLEAR_SELECTION: i64 = Action::ClearSelection as i64;
pub const SELECT_RING: i64 = Action::SelectRing as i64;

/// One menu entry.
#[derive(Clone, Debug)]
pub struct Item {
    pub id: i64,
    pub title: String,
    pub enabled: bool,
    /// Why it is disabled (tooltip).
    pub reason: String,
    pub shortcut: Option<&'static str>,
}

/// A titled submenu (`None` = the inline utilities after a separator).
pub struct Section {
    pub title: Option<&'static str>,
    pub items: Vec<Item>,
}

/// The sections and the commands each lists (`CommandIntent` order); menu-only ids are the
/// constants above. Entries are kept only when their toolbar button exists in the mode.
const SECTIONS: &[(&str, &[i64])] = &[
    (
        "Select",
        &[
            SELECT_ALL,
            CLEAR_SELECTION,
            Action::InvertSelection as i64,
            Action::Grow as i64,
            Action::Shrink as i64,
            Action::SelectLoop as i64,
            SELECT_RING,
            Action::Holes as i64,
            Action::Coplanar as i64,
            Action::Similar as i64,
            Action::SelectByColor as i64,
            Action::SelectBySmoothing as i64,
        ],
    ),
    (
        "Build",
        &[
            Action::Bevel as i64,
            Action::Extrude as i64,
            Action::Inset as i64,
            Action::Subdivide as i64,
            Action::SubdivideEdges as i64,
            Action::InsertLoop as i64,
            Action::Bridge as i64,
            Action::ConnectEdges as i64,
            Action::Connect as i64,
            Action::Fill as i64,
            Action::Triangulate as i64,
            Action::TrimWalls as i64,
            Action::ToolKnife as i64,
        ],
    ),
    ("Separate", &[Action::Delete as i64, Action::Detach as i64, Action::Duplicate as i64, Action::SplitVerts as i64]),
    ("Join", &[Action::Dissolve as i64, Action::Merge as i64, Action::Collapse as i64, Action::Weld as i64]),
    ("Paint", &[Action::Material as i64, Action::PaintColour as i64]),
    ("Normals", &[Action::Flip as i64, Action::FlipEdge as i64, Action::ConformNormals as i64]),
    ("Move", &[Action::OffsetElements as i64, Action::Rotate as i64, Action::RotateNeg as i64]),
    (
        "Object",
        &[
            Action::PivotToSelection as i64,
            Action::CenterPivot as i64,
            Action::FreezeTransform as i64,
            Action::Mirror as i64,
            Action::MergeObjects as i64,
            Action::SetCollider as i64,
            Action::SetTrigger as i64,
            Action::GenerateUv2 as i64,
            Action::ReusablePart as i64,
            Action::BakeMesh as i64,
        ],
    ),
];

/// Utilities, inline after a separator, and the Export submenu.
const UTILITIES: &[i64] = &[Action::UvEditor as i64];
const EXPORTS: &[i64] = &[Action::ExportObj as i64, Action::ExportGltf as i64, Action::ExportStl as i64, Action::ExportPly as i64, Action::ExportRes as i64, Action::ExportOptions as i64];

fn need(ok: bool, reason: &'static str) -> Option<&'static str> {
    (!ok).then_some(reason)
}

impl PixelBuilderPlugin {
    fn selected_count(&self) -> usize {
        match self.mode {
            Mode::Face => self.sel_faces.len(),
            Mode::Edge => self.sel_edges.len(),
            Mode::Vertex => self.sel_verts.len(),
            Mode::Object => 0,
        }
    }

    /// `action_requirement`: why command `id` can't run now (`None` = it can).
    pub(super) fn requirement(&self, id: i64) -> Option<&'static str> {
        use Mode::{Edge, Face, Object, Vertex};
        let Some(d) = self.data() else { return Some("Select a builder mesh.") };
        let (face_count, vert_count) = {
            let b = d.bind();
            (b.mesh().faces.len(), b.mesh().positions.len())
        };
        let mode = self.mode;
        let selected = self.selected_count();
        let (nf, ne, nv) = (self.sel_faces.len(), self.sel_edges.len(), self.sel_verts.len());
        let element_mode = || if mode == Object { Some("Select a mesh element mode.") } else { need(selected > 0, "Select one or more mesh elements.") };
        let face_only = |min: usize, reason: &'static str| if mode != Face { Some("Use face selection mode.") } else { need(nf >= min, reason) };
        let object_or_face = || match mode {
            Object => need(face_count > 0, "The mesh has no faces."),
            Face => need(nf > 0, "Select one or more faces."),
            _ => Some("Use object or face selection mode."),
        };
        let object_only = || if mode != Object { Some("Use object selection mode.") } else { need(face_count > 0, "The mesh has no faces.") };
        let edge_only = |min: usize, reason: &'static str| if mode != Edge { Some("Use edge selection mode.") } else { need(ne >= min, reason) };
        let vertex_only = |min: usize, reason: &'static str| if mode != Vertex { Some("Use vertex selection mode.") } else { need(nv >= min, reason) };
        match id {
            SELECT_ALL => need(face_count > 0 || vert_count > 0, "The mesh has no elements to select.").or(if mode == Object { Some("Select a mesh element mode.") } else { None }),
            CLEAR_SELECTION => element_mode(),
            SELECT_RING => {
                if !matches!(mode, Edge | Face) {
                    return Some("Use edge or face selection mode.");
                }
                need(selected > 0, "Select one or more edges or faces.")
            }
            _ => {
                let a = Action::from_i64(id)?;
                match a {
                    Action::InvertSelection => {
                        if mode == Object {
                            return Some("Select a mesh element mode.");
                        }
                        need(face_count > 0, "The mesh has no elements to select.")
                    }
                    Action::Grow | Action::Shrink | Action::SelectByColor | Action::PivotToSelection => element_mode(),
                    Action::Similar | Action::SelectBySmoothing | Action::Coplanar | Action::Detach | Action::Duplicate | Action::FlipEdge | Action::TrimWalls => {
                        face_only(1, "Select one or more faces.")
                    }
                    Action::Flip | Action::Subdivide | Action::Triangulate => object_or_face(),
                    Action::ConformNormals | Action::Mirror | Action::CenterPivot | Action::FreezeTransform | Action::SetCollider | Action::SetTrigger | Action::ReusablePart | Action::BakeMesh => {
                        object_only()
                    }
                    Action::MergeObjects => {
                        if mode != Object {
                            return Some("Use object selection mode.");
                        }
                        let n = EditorInterface::singleton().get_selection().map(|s| s.get_selected_nodes().len()).unwrap_or(0);
                        need(n >= 2, "Select at least two objects.")
                    }
                    Action::Bevel => {
                        if !matches!(mode, Edge | Vertex) {
                            return Some("Use edge or vertex selection mode.");
                        }
                        if selected == 0 {
                            return Some("Select one or more edges or vertices.");
                        }
                        need(self.bevel.is_finite() && self.bevel > 0.0, "Set a finite bevel amount greater than zero.")
                    }
                    Action::Dissolve | Action::Delete | Action::OffsetElements | Action::Rotate | Action::RotateNeg => element_mode(),
                    Action::GenerateUv2 => object_only(),
                    Action::PaintColour | Action::Material => object_or_face(),
                    Action::SelectLoop => {
                        if !matches!(mode, Edge | Face) {
                            return Some("Use edge or face selection mode.");
                        }
                        need(selected > 0, "Select one or more edges or faces.")
                    }
                    Action::Connect | Action::Collapse => vertex_only(2, "Select at least two vertices."),
                    Action::Merge => match mode {
                        Face => need(nf >= 2, "Select at least two faces."),
                        Vertex => need(nv >= 2, "Select at least two vertices."),
                        _ => Some("Use face or vertex selection mode."),
                    },
                    Action::Weld => match mode {
                        Object => need(face_count > 0, "The mesh has no faces."),
                        Vertex => {
                            if nv < 2 {
                                return Some("Select at least two vertices.");
                            }
                            need(self.xg.weld_dist.is_finite() && self.xg.weld_dist >= 0.0, "Set a finite, nonnegative weld distance.")
                        }
                        _ => Some("Use object or vertex selection mode."),
                    },
                    Action::SplitVerts => vertex_only(1, "Select one or more vertices."),
                    Action::Extrude => {
                        if !matches!(mode, Edge | Face) {
                            return Some("Use edge or face selection mode.");
                        }
                        if selected == 0 {
                            return Some(if mode == Face { "Select one or more faces." } else { "Select one or more edges." });
                        }
                        need(self.distance.is_finite() && self.distance != 0.0, "Set a finite, nonzero extrusion distance.")
                    }
                    Action::Inset => {
                        if mode != Face {
                            return Some("Use face selection mode.");
                        }
                        if nf == 0 {
                            return Some("Select one or more faces.");
                        }
                        need(self.inset.is_finite() && self.inset > 0.0, "Set a finite inset width greater than zero.")
                    }
                    Action::Bridge => edge_only(2, "Select two edges."),
                    Action::ConnectEdges => edge_only(2, "Select at least two edges."),
                    Action::InsertLoop | Action::SubdivideEdges => edge_only(1, "Select one or more edges."),
                    Action::Fill => {
                        if !matches!(mode, Edge | Vertex) {
                            return Some("Use edge or vertex selection mode.");
                        }
                        need(selected > 0, "Select one or more boundary elements.")
                    }
                    Action::Holes => need(matches!(mode, Edge | Vertex), "Use edge or vertex selection mode."),
                    Action::ToolKnife => need(mode != Object, "Select a mesh element mode."),
                    _ => None,
                }
            }
        }
    }

    fn item_title(&self, id: i64) -> String {
        match id {
            SELECT_ALL => "Select all".into(),
            CLEAR_SELECTION => "Clear selection".into(),
            SELECT_RING => "Select ring".into(),
            _ => match Action::from_i64(id) {
                Some(Action::Bevel) => if self.mode == Mode::Vertex { "Bevel vertices" } else { "Bevel edges" }.into(),
                Some(Action::Dissolve) => format!("Dissolve {}", match self.mode {
                    Mode::Face => "faces",
                    Mode::Edge => "edges",
                    _ => "vertices",
                }),
                Some(Action::ExportObj) => "OBJ…".into(),
                Some(Action::ExportGltf) => "glTF…".into(),
                Some(Action::ExportStl) => "STL…".into(),
                Some(Action::ExportPly) => "PLY…".into(),
                Some(Action::ExportRes) => "Godot Mesh…".into(),
                Some(Action::ExportOptions) => "Options…".into(),
                Some(Action::UvEditor) => "UV Editor…".into(),
                Some(a) => a.title(),
                None => String::new(),
            },
        }
    }

    fn item_shortcut(id: i64) -> Option<&'static str> {
        match id {
            SELECT_ALL => Some("select_all"),
            SELECT_RING => Some("select_ring"),
            _ => Action::from_i64(id).and_then(shortcuts::entry_of_action).map(|e| e.id),
        }
    }

    /// Whether menu command `id` belongs in the current mode (its toolbar button is there).
    fn item_in_mode(&self, id: i64) -> bool {
        match id {
            SELECT_ALL | CLEAR_SELECTION => self.mode != Mode::Object,
            SELECT_RING => matches!(self.mode, Mode::Edge | Mode::Face),
            _ => Action::from_i64(id).is_some_and(|a| a.mask() & self.mode.bit() != 0),
        }
    }

    fn item(&self, id: i64) -> Item {
        let reason = self.requirement(id);
        Item { id, title: self.item_title(id), enabled: reason.is_none(), reason: reason.unwrap_or("").to_string(), shortcut: Self::item_shortcut(id) }
    }

    /// The menu model for the current mode (`shared_command_sections` + `items`).
    pub(super) fn context_menu_model(&self) -> Vec<Section> {
        let mut out = Vec::new();
        for (title, ids) in SECTIONS {
            let items: Vec<Item> = ids.iter().copied().filter(|id| self.item_in_mode(*id)).map(|id| self.item(id)).collect();
            if !items.is_empty() {
                out.push(Section { title: Some(*title), items });
            }
        }
        out.push(Section { title: None, items: UTILITIES.iter().map(|id| self.item(*id)).collect() });
        let exports: Vec<Item> = EXPORTS.iter().copied().filter(|id| self.item_in_mode(*id)).map(|id| self.item(id)).collect();
        if !exports.is_empty() {
            out.push(Section { title: Some("Export"), items: exports });
        }
        out
    }

    /// `context_menu_targets_selection`: the element under `pos` is selected (Object mode: the
    /// edited mesh is under it).
    pub(super) fn context_targets_selection(&self, cam: &Gd<Camera3D>, pos: Vector2) -> bool {
        if self.mode == Mode::Object {
            let Some(d) = self.data() else { return false };
            let (o, dir) = self.local_ray(cam, pos);
            return pick::intersect_ray_closest(d.bind().mesh(), o, dir, true).is_some();
        }
        match self.element_at(cam, pos) {
            Some(super::Elem::Face(f)) => self.sel_faces.contains(&f),
            Some(super::Elem::Edge(e)) => self.sel_edges.contains(&e),
            Some(super::Elem::Vert(v)) => self.sel_verts.contains(&v),
            None => false,
        }
    }

    /// The menu may open: a builder mesh in the Select tool, no drag / box / modal tool running.
    pub(super) fn context_menu_available(&self) -> bool {
        self.target.is_some()
            && self.tool == super::Tool::Select
            && self.drag.is_none()
            && self.marquee.is_none()
            && !self.insert_vert
            && self.shape_drag.is_none()
            && self.paint.is_none()
    }

    /// Fills and shows the popup at viewport point `pos` of `cam`'s viewport.
    pub(super) fn open_context_menu(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        // An open adjustment is committed first (the menu acts on the result).
        self.adjust_commit();
        let model = self.context_menu_model();
        if self.ctx_menu.as_ref().is_none_or(|m| !m.is_instance_valid()) {
            let mut m = PopupMenu::new_alloc();
            m.set_name("PixelBuilderContextMenu");
            m.connect("id_pressed", &self.to_gd().callable("on_context_menu_id"));
            if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                base.add_child(&m);
            }
            self.ctx_menu = Some(m);
        }
        let Some(mut menu) = self.ctx_menu.clone() else { return };
        menu.clear_ex().free_submenus(true).done();
        let table = self.shortcut_table.clone();
        let fill = |m: &mut Gd<PopupMenu>, items: &[Item]| {
            for it in items {
                let idx = m.get_item_count();
                m.add_item_ex(&it.title).id(it.id as i32).done();
                if let Some(sc) = it.shortcut.and_then(shortcuts::index).and_then(|i| table.get(i).cloned().flatten()) {
                    m.set_item_shortcut_ex(idx, &sc).global(false).done();
                }
                m.set_item_disabled(idx, !it.enabled);
                if !it.enabled {
                    m.set_item_tooltip(idx, &it.reason);
                }
            }
        };
        let gd = self.to_gd();
        for sec in &model {
            match sec.title {
                Some(t) => {
                    let mut sub = PopupMenu::new_alloc();
                    sub.set_name(t);
                    fill(&mut sub, &sec.items);
                    sub.connect("id_pressed", &gd.callable("on_context_menu_id"));
                    menu.add_submenu_node_item(t, &sub);
                }
                None => {
                    if menu.get_item_count() > 0 {
                        menu.add_separator();
                    }
                    fill(&mut menu, &sec.items);
                }
            }
        }
        self.ctx_state = Some((self.mode, self.target.as_ref().filter(|t| t.is_instance_valid()).map(|t| t.instance_id()), self.selected_count()));
        let screen = cam
            .get_viewport()
            .and_then(|v| v.get_parent())
            .and_then(|p| p.try_cast::<godot::classes::Control>().ok())
            .map(|c| c.get_screen_position())
            .unwrap_or(Vector2::ZERO);
        let at = screen + pos;
        menu.reset_size();
        menu.popup_ex().rect(Rect2i::new(Vector2i::new(at.x as i32, at.y as i32), Vector2i::ZERO)).done();
    }
}

#[godot_api(secondary)]
impl PixelBuilderPlugin {
    /// A context menu entry: re-checked against the state the menu was built for.
    #[func]
    fn on_context_menu_id(&mut self, id: i64) {
        let still = self.ctx_state.take().is_some_and(|(mode, target, n)| {
            mode == self.mode && target == self.target.as_ref().filter(|t| t.is_instance_valid()).map(|t| t.instance_id()) && n == self.selected_count()
        });
        if !still || !self.context_menu_available() || self.requirement(id).is_some() {
            return;
        }
        self.run_menu_command(id);
    }

    /// Test / scripting hook: the context menu model of the current mode, as
    /// `[{section, id, title, enabled, reason, shortcut}]`.
    #[func]
    fn debug_context_menu(&self) -> VarArray {
        let mut out = VarArray::new();
        for sec in self.context_menu_model() {
            for it in sec.items {
                let mut d = VarDictionary::new();
                d.set("section", sec.title.unwrap_or(""));
                d.set("id", it.id);
                d.set("title", it.title.as_str());
                d.set("enabled", it.enabled);
                d.set("reason", it.reason.as_str());
                d.set("shortcut", it.shortcut.map(shortcuts::key_text).unwrap_or_default().as_str());
                out.push(&d.to_variant());
            }
        }
        out
    }

    /// Test / scripting hook: right-click (press + release) at `pos` of 3D viewport 0; returns
    /// whether the context menu opened.
    #[func]
    fn debug_right_click(&mut self, pos: Vector2) -> bool {
        use godot::classes::InputEventMouseButton;
        use godot::global::MouseButton;
        let mut ev = InputEventMouseButton::new_gd();
        ev.set_button_index(MouseButton::RIGHT);
        ev.set_position(pos);
        ev.set_pressed(true);
        // Both events pass through to Godot (free-look); the release schedules the menu.
        self.debug_input(ev.clone().upcast());
        let mut up = InputEventMouseButton::new_gd();
        up.set_button_index(MouseButton::RIGHT);
        up.set_position(pos);
        up.set_pressed(false);
        self.debug_input(up.upcast());
        self.open_context_menu_pending();
        self.ctx_menu.as_ref().is_some_and(|m| m.is_instance_valid() && m.is_visible())
    }

    /// Test / scripting hook: right press at `pos`, `travel` px of (relative) motion, release;
    /// returns `{open, press_consumed, release_consumed}` (a drag free-looks instead of opening it).
    #[func]
    fn debug_right_drag(&mut self, pos: Vector2, travel: f32) -> VarDictionary {
        use godot::classes::{InputEventMouseButton, InputEventMouseMotion};
        use godot::global::MouseButton;
        if let Some(m) = &mut self.ctx_menu
            && m.is_instance_valid()
        {
            m.hide();
        }
        let mut ev = InputEventMouseButton::new_gd();
        ev.set_button_index(MouseButton::RIGHT);
        ev.set_position(pos);
        ev.set_pressed(true);
        let press_consumed = self.debug_input(ev.upcast());
        let mut mm = InputEventMouseMotion::new_gd();
        mm.set_position(pos);
        mm.set_relative(Vector2::new(travel, 0.0));
        self.debug_input(mm.upcast());
        let mut up = InputEventMouseButton::new_gd();
        up.set_button_index(MouseButton::RIGHT);
        up.set_position(pos);
        up.set_pressed(false);
        let release_consumed = self.debug_input(up.upcast());
        self.open_context_menu_pending();
        let mut d = VarDictionary::new();
        d.set("open", self.ctx_menu.as_ref().is_some_and(|m| m.is_instance_valid() && m.is_visible()));
        d.set("press_consumed", press_consumed);
        d.set("release_consumed", release_consumed);
        d
    }

    /// Opens the menu of a right click recorded by `forward_3d_gui_input` (deferred from the
    /// release so Godot ends its free-look first).
    #[func]
    fn open_context_menu_pending(&mut self) {
        let Some(p) = self.ctx_pending.take() else { return };
        let Some(cam) = self.camera.clone().filter(|c| c.is_instance_valid()) else { return };
        self.open_context_menu(&cam, p);
    }

    /// Test / scripting hook: the popup (if built) — its items and submenus as shown.
    #[func]
    fn debug_context_popup(&self) -> Option<Gd<PopupMenu>> {
        self.ctx_menu.clone().filter(|m| m.is_instance_valid())
    }
}

impl PixelBuilderPlugin {
    /// Runs a menu command (toolbar action or a menu-only one).
    pub(super) fn run_menu_command(&mut self, id: i64) {
        // Select All / Clear / Ring are `Action`s too: `run_action` commits an open adjustment,
        // then syncs the UI (counts, Actions menu state) and redraws like every other command.
        self.on_action(id);
    }
}
