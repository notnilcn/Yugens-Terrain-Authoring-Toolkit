//! Builder keyboard shortcuts as Godot editor shortcuts (Editor Settings › Shortcuts ›
//! *Mst Builder*), so users can rebind them.
//!
//! Port of xogot-rust `xogot_modeler/src/shared/shortcuts.rs` (MIT): the entry table of ids
//! under one settings prefix, `register` (one `Shortcut` per entry in `EditorSettings`, kept when
//! the settings already have it so a user's binding survives), `matches` (the configured
//! shortcut's events, else the default) and `handle`'s "first matching entry wins" dispatch
//! (`PixelBuilderPlugin::on_key`). Gap fixed here: xogot-rust registered no default keys (only
//! Esc had one), so its shortcuts never fired; every entry below carries the builder's former
//! hard-coded key as its default.
//!
//! The context guards (E extrudes only in an element mode, J connects edges in Edge mode, …) stay
//! in the plugin; an entry whose guard refuses lets the key through to Godot.

use godot::classes::{EditorInterface, EditorSettings, InputEvent, InputEventKey, Shortcut};
use godot::global::Key;
use godot::prelude::*;

use super::ui::Action;

/// `EditorSettings` path prefix of every builder shortcut.
pub const PREFIX: &str = "mst_builder/";

/// Modifier bits of a default key.
pub const CTRL: u8 = 1;
pub const SHIFT: u8 = 2;
pub const ALT: u8 = 4;

/// One rebindable shortcut: settings id, display name, default key + modifiers and the toolbar
/// action it runs (for the bound key in tooltips; `None` for key-only commands).
pub struct Entry {
    pub id: &'static str,
    pub name: &'static str,
    pub key: Key,
    pub mods: u8,
    pub action: Option<Action>,
}

const fn e(id: &'static str, name: &'static str, key: Key, mods: u8, action: Option<Action>) -> Entry {
    Entry { id, name, key, mods, action }
}

/// Every builder shortcut, in dispatch order (the builder's former hard-coded keys).
pub const ENTRIES: &[Entry] = &[
    e("mode_object", "Object mode", Key::KEY_1, 0, Some(Action::ModeObject)),
    e("mode_face", "Face mode", Key::KEY_2, 0, Some(Action::ModeFace)),
    e("mode_edge", "Edge mode", Key::KEY_3, 0, Some(Action::ModeEdge)),
    e("mode_vertex", "Vertex mode", Key::KEY_4, 0, Some(Action::ModeVertex)),
    e("cycle_mode", "Cycle modes (Object, Vertex, Edge, Face)", Key::H, 0, None),
    e("tool_select", "Select / move tool", Key::W, 0, Some(Action::ToolSelect)),
    e("tool_shape", "Draw shape tool", Key::C, 0, Some(Action::ToolShape)),
    e("tool_bezier", "Bezier tool", Key::C, SHIFT, Some(Action::ToolBezier)),
    e("tool_paint", "Paint tool", Key::B, 0, Some(Action::ToolPaint)),
    e("tool_vertex_paint", "Vertex paint tool", Key::KEY_5, 0, Some(Action::ToolVertexPaint)),
    e("tool_place", "Place on cell tool", Key::P, 0, Some(Action::ToolPlace)),
    e("tool_knife", "Knife tool", Key::K, 0, Some(Action::ToolKnife)),
    e("gizmo_move", "Move handles", Key::W, SHIFT, Some(Action::GizmoMove)),
    e("gizmo_rotate", "Rotate handles", Key::E, SHIFT, Some(Action::GizmoRotate)),
    e("gizmo_scale", "Scale handles", Key::R, SHIFT, Some(Action::GizmoScale)),
    e("soft_select", "Soft selection", Key::O, 0, Some(Action::SoftSelect)),
    e("extrude", "Extrude", Key::E, 0, Some(Action::Extrude)),
    e("inset", "Inset", Key::I, 0, Some(Action::Inset)),
    e("bevel", "Bevel", Key::B, CTRL, Some(Action::Bevel)),
    e("insert_loop", "Insert edge loop", Key::R, CTRL, Some(Action::InsertLoop)),
    e("merge", "Merge", Key::M, 0, Some(Action::Merge)),
    e("connect", "Connect vertices / edges", Key::J, 0, Some(Action::Connect)),
    e("fill", "Fill hole", Key::F, 0, Some(Action::Fill)),
    e("triangulate", "Triangulate", Key::T, CTRL, Some(Action::Triangulate)),
    e("rotate", "Rotate 90°", Key::R, 0, Some(Action::Rotate)),
    e("rotate_back", "Rotate -90°", Key::R, ALT, Some(Action::RotateNeg)),
    e("duplicate", "Duplicate faces", Key::D, CTRL, Some(Action::Duplicate)),
    e("delete", "Delete", Key::DELETE, 0, Some(Action::Delete)),
    e("select_all", "Select all", Key::A, CTRL, Some(Action::SelectAll)),
    e("grow", "Grow selection", Key::G, 0, Some(Action::Grow)),
    e("shrink", "Shrink selection", Key::G, SHIFT, Some(Action::Shrink)),
    e("select_loop", "Select loop", Key::L, 0, Some(Action::SelectLoop)),
    e("select_ring", "Select ring", Key::L, ALT, Some(Action::SelectRing)),
    e("insert_vertex", "Insert vertex", Key::V, SHIFT, Some(Action::InsertVertex)),
    e("uv_editor", "UV editor", Key::U, 0, Some(Action::UvEditor)),
    e("grid_halve", "Halve grid step", Key::BRACKETLEFT, 0, None),
    e("grid_double", "Double grid step", Key::BRACKETRIGHT, 0, None),
];

/// Index of entry `id`.
pub fn index(id: &str) -> Option<usize> {
    ENTRIES.iter().position(|e| e.id == id)
}

/// The entry whose key a toolbar action shows (Connect edges shares Connect's key).
pub fn entry_of_action(a: Action) -> Option<&'static Entry> {
    let a = if a == Action::ConnectEdges { Action::Connect } else { a };
    ENTRIES.iter().find(|e| e.action == Some(a))
}

fn settings() -> Option<Gd<EditorSettings>> {
    if !godot::classes::Engine::singleton().is_editor_hint() {
        return None;
    }
    EditorInterface::singleton().get_editor_settings()
}

pub fn path(id: &str) -> String {
    format!("{PREFIX}{id}")
}

/// The default key event of an entry (Ctrl is Cmd on macOS).
pub fn default_event(e: &Entry) -> Gd<InputEventKey> {
    let mut k = InputEventKey::new_gd();
    k.set_keycode(e.key);
    if e.mods & CTRL != 0 {
        k.set_command_or_control_autoremap(true);
    }
    k.set_shift_pressed(e.mods & SHIFT != 0);
    k.set_alt_pressed(e.mods & ALT != 0);
    k
}

fn default_events(e: &Entry) -> VarArray {
    let mut a = VarArray::new();
    a.push(&default_event(e).to_variant());
    a
}

/// `register()`: one `Shortcut` per entry under `mst_builder/<id>`. An entry the settings already
/// hold (a binding the user changed, loaded from the editor settings) is kept; new ones get the
/// default key. Returns the shortcuts, index-aligned with [`ENTRIES`].
pub fn register() -> Vec<Option<Gd<Shortcut>>> {
    let Some(mut es) = settings() else { return ENTRIES.iter().map(|_| None).collect() };
    ENTRIES
        .iter()
        .map(|e| {
            let p = path(e.id);
            let mut sc = if es.has_shortcut(&p) {
                es.get_shortcut(&p)
            } else {
                None
            }
            .unwrap_or_else(|| {
                let mut sc = Shortcut::new_gd();
                sc.set_events(&default_events(e));
                es.add_shortcut(&p, &sc);
                // `add_shortcut` may store its own copy: hand back the one the settings hold.
                es.get_shortcut(&p).unwrap_or(sc)
            });
            if sc.get_name().is_empty() {
                sc.set_name(e.name);
            }
            // The default the Shortcuts tab reverts to (what Godot's ED_SHORTCUT stores).
            if !sc.has_meta("original") {
                sc.set_meta("original", &default_events(e).to_variant());
            }
            Some(sc)
        })
        .collect()
}

/// The current shortcuts from the settings (after a rebinding), index-aligned with [`ENTRIES`].
pub fn reload() -> Vec<Option<Gd<Shortcut>>> {
    let Some(es) = settings() else { return ENTRIES.iter().map(|_| None).collect() };
    ENTRIES.iter().map(|e| es.get_shortcut(&path(e.id))).collect()
}

/// Whether `event` triggers entry `i`: its configured shortcut, else (outside the editor /
/// before registration) its default key.
pub fn matches(table: &[Option<Gd<Shortcut>>], i: usize, event: &Gd<InputEventKey>) -> bool {
    let ev: Gd<InputEvent> = event.clone().upcast();
    match table.get(i).and_then(|s| s.as_ref()) {
        Some(sc) => sc.matches_event(&ev),
        None => ENTRIES.get(i).is_some_and(|e| default_event(e).upcast::<InputEvent>().is_match(&ev)),
    }
}

/// Display text of entry `id`'s current binding ("Ctrl+B"), "" when unbound.
pub fn key_text(id: &str) -> String {
    let Some(e) = ENTRIES.iter().find(|e| e.id == id) else { return String::new() };
    let sc = settings().and_then(|es| es.get_shortcut(&path(e.id)));
    match sc {
        Some(sc) => {
            if sc.has_valid_event() {
                sc.get_as_text().to_string()
            } else {
                String::new()
            }
        }
        None => default_event(e).as_text().to_string(),
    }
}

/// The bound key of a toolbar action, if it has an entry.
pub fn action_key_text(a: Action) -> Option<String> {
    entry_of_action(a).map(|e| key_text(e.id))
}
