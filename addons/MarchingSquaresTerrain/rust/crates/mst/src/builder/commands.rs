//! The Xogot command table: Xogot Modeler's 45 mesh / object actions mapped onto builder
//! [`Action`]s, with Xogot's titles, selection modes and command sections. It drives the Xogot
//! layout's *Actions* menu (`xogot_ui/toolbar.rs`); builder commands the table does not cover are
//! listed after it (*Builder* section), so switching the layout never removes a command.
//!
//! Port of xogot-rust (MIT) `editor_core/src/action.rs` (`Action` order, `title`, `title_for`),
//! `editor_core/src/ux_presentation.rs` (`SUPPORTED_MODES`, `INTENTS`, `CommandIntent` order)
//! and `xogot_modeler/src/ui/toolbar.rs` `action_entries` (titled separators per section).
//! Plain data and pure functions only: the tables are unit-tested without an engine.

use super::ui::{self, Action, E, F, Mode, O, V};

/// `CommandIntent`, in menu order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Select,
    Build,
    Separate,
    Join,
    Normals,
    Move,
    Object,
}

impl Intent {
    pub const ALL: [Intent; 7] = [Intent::Select, Intent::Build, Intent::Separate, Intent::Join, Intent::Normals, Intent::Move, Intent::Object];
    pub fn title(self) -> &'static str {
        match self {
            Intent::Select => "Select",
            Intent::Build => "Build",
            Intent::Separate => "Separate",
            Intent::Join => "Join",
            Intent::Normals => "Normals",
            Intent::Move => "Move",
            Intent::Object => "Object",
        }
    }
}

/// `ModelerAction.supportedModes` by Xogot action index (0 object, 1 vertex, 2 edge, 3 face).
const SUPPORTED_MODES: [&[u8]; 45] = [
    &[0, 1, 2, 3], &[1, 2, 3], &[0, 1, 2, 3], &[1, 2, 3], &[1, 2, 3], &[2, 3], &[2, 3], &[1, 2], &[3],
    &[1, 2, 3], &[3], &[1], &[1], &[1], &[1], &[2], &[1, 2, 3], &[2, 3], &[2], &[2], &[2], &[2],
    &[1, 2, 3], &[1, 2], &[3], &[3], &[3], &[0, 3], &[0, 3], &[3], &[3], &[3], &[3], &[0, 3], &[0, 3],
    &[1, 2, 3], &[1, 2, 3], &[0], &[0], &[0], &[0], &[0], &[0], &[0], &[0],
];

/// `ModelerAction.intent` by Xogot action index (`Intent::ALL` index).
const INTENTS: [u8; 45] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 2, 1, 1, 1, 3, 3, 1, 1, 3, 1, 1, 1, 2, 4, 1, 3, 2, 2, 4, 4,
    1, 5, 6, 6, 6, 6, 3, 6, 6, 6, 6,
];

/// Xogot action index → (Xogot title, builder action, extra mode restriction). Row 16 (*Bevel
/// Mode*) is the builder's vertex bevel (Edge mode lists *Bevel Edges*, row 17, instead).
pub const XG_COMMANDS: [(&str, Action, u8); 45] = [
    ("Select All Elements", Action::SelectAll, ui::ALL),
    ("Clear Element Selection", Action::ClearSelection, ui::ALL),
    ("Invert Selection", Action::InvertSelection, ui::ALL),
    ("Grow Selection", Action::Grow, ui::ALL),
    ("Shrink Selection", Action::Shrink, ui::ALL),
    ("Select Loop", Action::SelectLoop, ui::ALL),
    ("Select Ring", Action::SelectRing, ui::ALL),
    ("Select Holes", Action::Holes, ui::ALL),
    ("Select by Material", Action::Similar, ui::ALL),
    ("Select by Color", Action::SelectByColor, ui::ALL),
    ("Select Smoothing Group", Action::SelectBySmoothing, ui::ALL),
    ("Connect Vertices", Action::Connect, ui::ALL),
    ("Collapse Vertices", Action::Collapse, ui::ALL),
    ("Weld Vertices", Action::Weld, ui::ALL),
    ("Split Vertices", Action::SplitVerts, ui::ALL),
    ("Extrude Edges", Action::Extrude, ui::ALL),
    ("Bevel Mode", Action::Bevel, V),
    ("Bevel Edges", Action::Bevel, E),
    ("Bridge Edges", Action::Bridge, ui::ALL),
    ("Connect Edges", Action::ConnectEdges, ui::ALL),
    ("Insert Edge Loop", Action::InsertLoop, ui::ALL),
    ("Subdivide Edges", Action::SubdivideEdges, ui::ALL),
    ("Dissolve", Action::Dissolve, ui::ALL),
    ("Fill Hole", Action::Fill, ui::ALL),
    ("Extrude Faces", Action::Extrude, ui::ALL),
    ("Inset Faces", Action::Inset, ui::ALL),
    ("Delete Faces", Action::Delete, ui::ALL),
    ("Flip Faces", Action::Flip, ui::ALL),
    ("Subdivide Faces", Action::Subdivide, ui::ALL),
    ("Merge Faces", Action::Merge, ui::ALL),
    ("Detach Faces", Action::Detach, ui::ALL),
    ("Duplicate Faces", Action::Duplicate, ui::ALL),
    ("Turn Face Edges", Action::FlipEdge, ui::ALL),
    ("Conform Normals", Action::ConformNormals, ui::ALL),
    ("Triangulate Faces", Action::Triangulate, ui::ALL),
    ("Offset Elements", Action::OffsetElements, ui::ALL),
    ("Set Pivot to Selection", Action::PivotToSelection, ui::ALL),
    ("Center Pivot", Action::CenterPivot, ui::ALL),
    ("Freeze Transform", Action::FreezeTransform, ui::ALL),
    ("Mirror Objects", Action::Mirror, ui::ALL),
    ("Merge Objects", Action::MergeObjects, ui::ALL),
    ("Set Collider", Action::SetCollider, ui::ALL),
    ("Set Trigger", Action::SetTrigger, ui::ALL),
    ("Generate Lightmap UVs", Action::GenerateUv2, ui::ALL),
    ("Create Reusable Part", Action::ReusablePart, ui::ALL),
];

/// The Export Mesh submenu (Xogot: one dialog with a format choice).
pub const EXPORTS: [(Action, &str); 6] = [
    (Action::ExportObj, "OBJ…"),
    (Action::ExportGltf, "glTF…"),
    (Action::ExportStl, "STL…"),
    (Action::ExportPly, "PLY…"),
    (Action::ExportRes, "Godot Mesh…"),
    (Action::ExportOptions, "Export Options…"),
];
/// Xogot's scene-graph confirmation (`Action::requires_scene_graph_confirmation`, mask
/// `0x180_c000_0000`: Detach Faces, Duplicate Faces, Mirror Objects, Merge Objects): the dialog
/// title and the option rows it shows (an empty list shows "Run <title>?"). Detach / Duplicate
/// only ask when they create a new object (*To new object*); in place they are plain mesh edits.
/// Port of xogot-rust `editor_core/src/ux_presentation.rs` (`SCENE_GRAPH_CONFIRMATION_MASK`) and
/// `xogot_modeler/src/ui/action_confirmation.rs` (`update_from`: title + "Run %s?" + the
/// action's parameter form).
pub fn confirmation(a: Action, mode: Mode, to_new_object: bool) -> Option<(&'static str, Vec<ui::Field>)> {
    use ui::Field as Fd;
    match a {
        Action::Detach if to_new_object && mode == Mode::Face => Some(("Detach Faces", vec![Fd::ToNewObject])),
        Action::Duplicate if to_new_object && mode == Mode::Face => Some(("Duplicate Faces", vec![Fd::ToNewObject])),
        Action::Mirror if mode == Mode::Object => Some(("Mirror Objects", vec![Fd::MirrorX, Fd::MirrorY, Fd::MirrorZ, Fd::MirrorDup])),
        Action::MergeObjects => Some(("Merge Objects", Vec::new())),
        _ => None,
    }
}

/// The Boolean submenu (Xogot: the Boolean dialog).
pub const BOOLEANS: [(Action, &str); 3] = [(Action::BoolUnion, "Union"), (Action::BoolSubtract, "Subtract"), (Action::BoolIntersect, "Intersect")];

/// Builder actions owned by other Xogot-layout controls (toolbar, Selection & Handles, Paint
/// popover, Builder menu), so they are not repeated in the Actions menu's *Builder* section.
pub const OWNED: &[Action] = &[
    Action::ModeObject,
    Action::ModeFace,
    Action::ModeEdge,
    Action::ModeVertex,
    Action::ToolSelect,
    Action::ToolShape,
    Action::ToolBezier,
    Action::ToolPaint,
    Action::ToolVertexPaint,
    Action::ToolKnife,
    Action::SnapGrid,
    Action::Convert,
    Action::GizmoMove,
    Action::GizmoRotate,
    Action::GizmoScale,
    Action::SoftSelect,
    Action::SnapVertex,
    Action::IndividualOrigins,
    Action::UvLock,
    Action::SnapCell,
    Action::SurfaceDraw,
    Action::XRay,
    Action::Preferences,
    Action::UvEditor,
];

/// Builder mode bit of a Xogot mode index.
fn mode_bit(xogot: u8) -> u8 {
    match xogot {
        0 => O,
        1 => V,
        2 => E,
        _ => F,
    }
}

/// Builder mode bits Xogot command `i` is listed in: Xogot's modes ∩ the builder action's
/// modes ∩ the row's restriction (the builder has no face bevel, no object-mode selection, …).
pub fn command_modes(i: usize) -> u8 {
    let xogot = SUPPORTED_MODES[i].iter().fold(0, |m, x| m | mode_bit(*x));
    let (_, a, only) = XG_COMMANDS[i];
    xogot & a.mask() & only
}

pub fn command_intent(i: usize) -> Intent {
    Intent::ALL[INTENTS[i] as usize]
}

/// `title_for`: Dissolve names the elements it removes.
pub fn command_title(i: usize, mode: Mode) -> &'static str {
    let (t, a, _) = XG_COMMANDS[i];
    if a != Action::Dissolve {
        return t;
    }
    match mode {
        Mode::Vertex => "Dissolve Vertices",
        Mode::Edge => "Dissolve Edges",
        Mode::Face => "Dissolve Faces",
        Mode::Object => t,
    }
}

/// One Actions-menu entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub action: Action,
    pub title: String,
}

/// A menu section: a titled separator (`None` = untitled) or a submenu.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub title: Option<&'static str>,
    pub submenu: bool,
    pub entries: Vec<Entry>,
}

/// The Actions menu of `mode`. `visible(a, mask)` is the MST toolbar's visibility rule (the
/// builder-extras section shows what that toolbar would show).
pub fn actions_menu(mode: Mode, visible: impl Fn(Action, u8) -> bool) -> Vec<Section> {
    let bit = mode.bit();
    let mut out = Vec::new();
    let mut listed: Vec<Action> = Vec::new();
    for intent in Intent::ALL {
        let mut entries = Vec::new();
        for i in 0..XG_COMMANDS.len() {
            if command_intent(i) != intent || command_modes(i) & bit == 0 {
                continue;
            }
            let a = XG_COMMANDS[i].1;
            if listed.contains(&a) {
                continue;
            }
            listed.push(a);
            entries.push(Entry { action: a, title: command_title(i, mode).to_string() });
        }
        // Cut: Xogot's knife, listed with the Build commands.
        if intent == Intent::Build && Action::ToolKnife.mask() & bit != 0 {
            entries.push(Entry { action: Action::ToolKnife, title: "Cut".into() });
        }
        if !entries.is_empty() {
            out.push(Section { title: Some(intent.title()), submenu: false, entries });
        }
    }
    // Utilities (untitled), then the Export and Boolean submenus.
    out.push(Section { title: None, submenu: false, entries: vec![Entry { action: Action::UvEditor, title: "UV Editor…".into() }] });
    let ex: Vec<Entry> = EXPORTS.iter().filter(|(a, _)| a.mask() & bit != 0).map(|(a, t)| Entry { action: *a, title: t.to_string() }).collect();
    if !ex.is_empty() {
        out.push(Section { title: Some("Export Mesh"), submenu: true, entries: ex });
    }
    if mode == Mode::Object {
        out.push(Section { title: Some("Boolean"), submenu: true, entries: BOOLEANS.iter().map(|(a, t)| Entry { action: *a, title: t.to_string() }).collect() });
    }
    // Builder extras: every builder command of the mode not listed above.
    let mut extras = Vec::new();
    for (a, mask) in ui::toolbar_actions() {
        if listed.contains(&a) || OWNED.contains(&a) || EXPORTS.iter().any(|e| e.0 == a) || BOOLEANS.iter().any(|b| b.0 == a) || !visible(a, mask) {
            continue;
        }
        extras.push(Entry { action: a, title: a.title() });
        if a == Action::Rotate {
            extras.push(Entry { action: Action::RotateNeg, title: Action::RotateNeg.title() });
        }
    }
    if !extras.is_empty() {
        out.push(Section { title: Some("Builder"), submenu: false, entries: extras });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modes_of(a: &str) -> u8 {
        let i = XG_COMMANDS.iter().position(|c| c.0 == a).unwrap();
        command_modes(i)
    }

    #[test]
    fn the_table_has_the_45_xogot_actions_in_order() {
        assert_eq!(XG_COMMANDS.len(), 45);
        assert_eq!(XG_COMMANDS[0].0, "Select All Elements");
        assert_eq!(XG_COMMANDS[11].0, "Connect Vertices");
        assert_eq!(XG_COMMANDS[44].0, "Create Reusable Part");
        assert_eq!(command_intent(11), Intent::Join);
        assert_eq!(command_intent(35), Intent::Move);
        assert_eq!(command_intent(44), Intent::Object);
    }

    #[test]
    fn modes_translate_from_xogot_indices() {
        // Xogot vertex index 1 → builder Vertex.
        assert_eq!(modes_of("Connect Vertices"), V);
        assert_eq!(modes_of("Select by Material"), F);
        assert_eq!(modes_of("Flip Faces"), O | F);
        // Builder restrictions: no object-mode invert, no face bevel, whole-mesh conform.
        assert_eq!(modes_of("Invert Selection"), F | E | V);
        assert_eq!(modes_of("Bevel Edges"), E);
        assert_eq!(modes_of("Bevel Mode"), V);
        assert_eq!(modes_of("Conform Normals"), O);
        assert_eq!(modes_of("Select Holes"), E);
    }

    fn titles(mode: Mode) -> Vec<String> {
        actions_menu(mode, |a, m| a.mask() & m & mode.bit() != 0 || a == Action::Preferences).into_iter().flat_map(|s| s.entries).map(|e| e.title).collect()
    }

    #[test]
    fn face_mode_lists_the_face_commands() {
        let t = titles(Mode::Face);
        for want in ["Select All Elements", "Select Ring", "Extrude Faces", "Inset Faces", "Dissolve Faces", "Cut", "UV Editor…", "OBJ…"] {
            assert!(t.iter().any(|x| x == want), "{want} missing from {t:?}");
        }
        assert!(!t.iter().any(|x| x == "Bevel Edges" || x == "Connect Vertices"));
        // Builder extras: Rotate ±90°, Trim walls, Coplanar.
        assert!(t.iter().any(|x| x == "Rotate 90°"));
        assert!(t.iter().any(|x| x == "Rotate -90°"));
        assert!(t.iter().any(|x| x == "Trim walls"));
    }

    #[test]
    fn delete_and_merge_stay_reachable_in_every_element_mode() {
        for (m, want) in [(Mode::Edge, "Delete"), (Mode::Vertex, "Delete"), (Mode::Vertex, "Merge")] {
            assert!(titles(m).iter().any(|x| x == want), "{want} in {m:?}");
        }
    }

    #[test]
    fn no_entry_is_listed_twice() {
        for m in [Mode::Object, Mode::Face, Mode::Edge, Mode::Vertex] {
            let mut seen = Vec::new();
            for s in actions_menu(m, |a, mask| a.mask() & mask & m.bit() != 0) {
                for e in s.entries {
                    assert!(!seen.contains(&e.action), "{:?} twice in {m:?}", e.action);
                    seen.push(e.action);
                }
            }
        }
    }

    #[test]
    fn menu_only_ids_resolve_and_old_ids_are_stable() {
        assert_eq!(Action::ModeObject as i64, 0);
        assert_eq!(Action::ExportOptions as i64, 80);
        for (a, id) in [(Action::SelectAll, 81), (Action::ClearSelection, 82), (Action::SelectRing, 83), (Action::GenerateUv2, 84), (Action::RotateNeg, 85), (Action::PaintColour, 86), (Action::ExportRes, 87)] {
            assert_eq!(a as i64, id);
            assert_eq!(Action::from_i64(id), Some(a));
            assert!(a.mask() != 0, "{a:?} has modes");
        }
        // Scene-graph confirmations: Detach / Duplicate only to a new object.
        assert!(confirmation(Action::Detach, Mode::Face, false).is_none());
        assert_eq!(confirmation(Action::Detach, Mode::Face, true).map(|c| c.0), Some("Detach Faces"));
        assert_eq!(confirmation(Action::Mirror, Mode::Object, false).map(|c| c.1.len()), Some(4));
        assert_eq!(confirmation(Action::MergeObjects, Mode::Object, false).map(|c| c.1.is_empty()), Some(true));
        assert!(confirmation(Action::Extrude, Mode::Face, true).is_none());
        // The Godot Mesh export is in the Export submenu (Object mode).
        let ex = actions_menu(Mode::Object, |_, _| true);
        assert!(ex.iter().any(|s| s.submenu && s.entries.iter().any(|e| e.action == Action::ExportRes && e.title == "Godot Mesh…")));
    }
}
