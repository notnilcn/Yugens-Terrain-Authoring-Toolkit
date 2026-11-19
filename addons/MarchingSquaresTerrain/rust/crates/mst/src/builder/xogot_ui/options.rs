//! Xogot layout: the *Modeler Options* dialog (category list + form) with a search box.
//!
//! Port of xogot-rust (MIT) `xogot_modeler/src/ui/options_dialog.rs` (AcceptDialog "Modeler
//! Options", OK "Close", *Reset to Defaults* as a custom action, an ItemList of categories beside
//! a scrolled form, 680 × 440 × scale) and `editor_core/src/options.rs` (`OptionsCategory`
//! ids, titles, `consumer_copy`, `search_terms`). Rows are the builder's settings
//! (docs/port/07-xogot-ui.md §4.7); *Trim Walls*, *Face UV* and *Vertex Paint* are builder
//! categories. The search box is an addition (Xogot has none): it filters categories by
//! `search_terms` plus their row labels, and the shown category's rows by label.

use godot::classes::control::SizeFlags;
use godot::classes::{AcceptDialog, HSplitContainer, ItemList, LineEdit, ScrollContainer, VBoxContainer};
use godot::prelude::*;

use super::super::forms::{FieldForm, MeshProp, Row, row_label};
use super::super::prefs::Key;
use super::super::ui::{Action, Field, SyncState, UvField, Values};
use crate::editor::ui_kit;

/// (id, title, consumer copy, rows).
pub type Category = (&'static str, &'static str, &'static str, &'static [Row]);

pub const CATEGORIES: &[Category] = &[
    (
        "snapping",
        "Snapping",
        "Used by drawing and transform tools.",
        &[
            Row::Toggle(Action::SnapGrid, "Snap New Shapes to Grid"),
            Row::Field(Field::GridStep),
            Row::Toggle(Action::SnapCell, "Snap to Terrain Cells"),
            Row::Toggle(Action::SurfaceDraw, "Draw on Surface"),
            Row::Toggle(Action::SnapVertex, "Vertex Snap"),
            Row::Field(Field::RotSnap),
            Row::Pref(Key::SnapGrid),
            Row::Pref(Key::GridStep),
            Row::Pref(Key::RotSnap),
        ],
    ),
    ("softSelection", "Soft Selection", "Used by vertex move, rotate, and scale.", &[Row::Toggle(Action::SoftSelect, "Enabled"), Row::Field(Field::SoftRadius), Row::Field(Field::Falloff)]),
    ("extrude", "Extrude", "Used by Extrude Edges and Extrude Faces.", &[Row::Field(Field::Distance), Row::Field(Field::ExtrudeMethod), Row::Field(Field::EdgesGroup)]),
    ("inset", "Inset", "Used by Inset Faces.", &[Row::Field(Field::Inset), Row::Field(Field::InsetMode), Row::Field(Field::InsetIndividual)]),
    (
        "bevel",
        "Bevel",
        "Used by Bevel Mode and Bevel Edges.",
        &[
            Row::Field(Field::Bevel),
            Row::Field(Field::BevelAmountType),
            Row::Field(Field::BevelSegs),
            Row::Field(Field::BevelProfile),
            Row::Field(Field::BevelClamp),
            Row::Field(Field::BevelHarden),
        ],
    ),
    ("weld", "Weld", "Used by Weld Vertices and Collapse Vertices.", &[Row::Field(Field::WeldDist), Row::Field(Field::CollapseFirst)]),
    ("subdivide", "Subdivide", "Used by Subdivide Edges.", &[Row::Field(Field::Cuts), Row::Field(Field::SubdivSegs)]),
    ("fillHole", "Fill Hole", "Used by Fill Hole.", &[Row::Field(Field::FillEntire)]),
    ("detachAndDuplicate", "Detach & Duplicate", "Used by Detach Faces and Duplicate Faces.", &[Row::Field(Field::ToNewObject)]),
    (
        "offset",
        "Offset",
        "Used by Offset Elements.",
        &[Row::Field(Field::OffsetX), Row::Field(Field::OffsetY), Row::Field(Field::OffsetZ), Row::Field(Field::OffsetSpace), Row::Field(Field::OffsetPerFace)],
    ),
    ("mirror", "Mirror", "Used by Mirror Objects.", &[Row::Field(Field::MirrorX), Row::Field(Field::MirrorY), Row::Field(Field::MirrorZ), Row::Field(Field::MirrorDup)]),
    ("growSelection", "Grow Selection", "Used by Grow Selection and Shrink Selection.", &[Row::Field(Field::GrowAngle)]),
    (
        "viewportFeedback",
        "Viewport Feedback",
        "Used by viewport selection feedback.",
        &[Row::Pref(Key::ShowSceneInfo), Row::Pref(Key::ShowDimensions), Row::Pref(Key::Hover), Row::Pref(Key::AdjustPanel), Row::Pref(Key::ShiftMode), Row::Pref(Key::RectMode)],
    ),
    (
        "generatedData",
        "Generated Data",
        "Used when Modeler generates mesh data.",
        &[Row::Mesh(MeshProp::LiveUv2, "Auto Lightmap UVs"), Row::Pref(Key::LightmapTexel), Row::Mesh(MeshProp::ClearGrass, "Clear Terrain Grass")],
    ),
    (
        "appearanceAndLogging",
        "Appearance & Logging",
        "Used by Modeler display and logging.",
        &[
            Row::Layout,
            Row::Pref(Key::VertexSize),
            Row::Pref(Key::HandleScale),
            Row::Pref(Key::ColorEdge),
            Row::Pref(Key::ColorEdgeSelected),
            Row::Pref(Key::ColorEdgeHover),
            Row::Pref(Key::ColorVertex),
            Row::Pref(Key::ColorVertexSelected),
            Row::Pref(Key::ColorVertexHover),
            Row::Pref(Key::ColorFaceSelected),
            Row::Pref(Key::ColorFaceHover),
        ],
    ),
    // Builder categories.
    ("trimWalls", "Trim Walls", "Used by Trim Walls (builder).", &[Row::Field(Field::Profile), Row::Field(Field::TrimPlace), Row::Field(Field::TrimDepth), Row::Field(Field::TrimHeight)]),
    (
        "faceUV",
        "Face UV",
        "Automatic face UVs of the selected faces (builder).",
        &[Row::Uv(UvField::OffsetU), Row::Uv(UvField::OffsetV), Row::Uv(UvField::ScaleU), Row::Uv(UvField::ScaleV), Row::Uv(UvField::Rotation), Row::UvWorld, Row::UvButtons],
    ),
];

/// `search_terms` (title, consumer copy, id) plus the row labels.
pub fn search_terms(c: &Category) -> Vec<String> {
    let mut t = vec![c.1.to_string(), c.2.to_string(), c.0.to_string()];
    t.extend(c.3.iter().map(row_label));
    t
}

/// Categories matching `query` (case-insensitive substring of a search term; empty = all).
pub fn matching(query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    (0..CATEGORIES.len()).filter(|i| q.is_empty() || search_terms(&CATEGORIES[*i]).iter().any(|t| t.to_lowercase().contains(&q))).collect()
}

/// `hud_logic::options_category`: the category of an adjustable action.
pub fn category_of(a: Action) -> Option<usize> {
    let id = match a {
        Action::Extrude => "extrude",
        Action::Inset => "inset",
        Action::Bevel => "bevel",
        Action::Weld | Action::Collapse => "weld",
        Action::SubdivideEdges | Action::Subdivide | Action::InsertLoop => "subdivide",
        Action::Fill => "fillHole",
        Action::Detach | Action::Duplicate => "detachAndDuplicate",
        Action::OffsetElements => "offset",
        Action::Mirror => "mirror",
        Action::Grow | Action::Shrink => "growSelection",
        Action::TrimWalls => "trimWalls",
        _ => return None,
    };
    CATEGORIES.iter().position(|c| c.0 == id)
}

pub struct Options {
    pub dialog: Gd<AcceptDialog>,
    pub search: Gd<LineEdit>,
    pub list: Gd<ItemList>,
    pub form: FieldForm,
    /// Category indices shown in the list (filtered by the search).
    pub shown: Vec<usize>,
    pub current: usize,
}

pub fn build(target: &Gd<Object>) -> Options {
    let s = ui_kit::editor_scale();
    let mut dialog = AcceptDialog::new_alloc();
    dialog.set_name("XogotModelerOptionsDialog");
    dialog.set_title("Modeler Options");
    dialog.set_ok_button_text("Close");
    dialog.add_button_ex("Reset to Defaults").right(true).action("reset").done();
    dialog.connect("custom_action", &Callable::from_object_method(target, "on_xg_options_action"));
    let mut col = VBoxContainer::new_alloc();
    let mut search = LineEdit::new_alloc();
    search.set_name("Search");
    search.set_placeholder("Search");
    search.set_clear_button_enabled(true);
    search.connect("text_changed", &Callable::from_object_method(target, "on_xg_options_search"));
    col.add_child(&search);
    let mut split = HSplitContainer::new_alloc();
    split.set_name("Split");
    split.set_v_size_flags(SizeFlags::EXPAND_FILL);
    let mut list = ItemList::new_alloc();
    list.set_name("Categories");
    list.set_custom_minimum_size(Vector2::new(200.0 * s, 0.0));
    list.connect("item_selected", &Callable::from_object_method(target, "on_xg_options_category"));
    split.add_child(&list);
    let mut scroll = ScrollContainer::new_alloc();
    scroll.set_name("Scroll");
    scroll.set_horizontal_scroll_mode(godot::classes::scroll_container::ScrollMode::DISABLED);
    scroll.set_h_size_flags(SizeFlags::EXPAND_FILL);
    let form = FieldForm::new("Form");
    scroll.add_child(&form.root);
    split.add_child(&scroll);
    col.add_child(&split);
    dialog.add_child(&col);
    Options { dialog, search, list, form, shown: (0..CATEGORIES.len()).collect(), current: 0 }
}

impl Options {
    /// Refills the category list for the search text (`ItemList.select` does not emit).
    pub fn fill_list(&mut self) {
        let q = self.search.get_text().to_string();
        self.shown = matching(&q);
        self.list.clear();
        for i in &self.shown {
            self.list.add_item(CATEGORIES[*i].1);
        }
        if !self.shown.contains(&self.current)
            && let Some(first) = self.shown.first()
        {
            self.current = *first;
        }
        if let Some(pos) = self.shown.iter().position(|i| *i == self.current) {
            self.list.select(pos as i32);
        }
    }

    /// Shows category `current`'s rows (filtered by the search).
    pub fn show_current(&mut self, target: &Gd<Object>, v: &Values, st: &SyncState) {
        let rows = CATEGORIES[self.current].3;
        self.form.set_rows(target, rows, v, st);
        let q = self.search.get_text().to_string();
        // A query matching the category itself shows all its rows.
        let c = &CATEGORIES[self.current];
        let whole = [c.0, c.1, c.2].iter().any(|t| t.to_lowercase().contains(&q.trim().to_lowercase()));
        self.form.filter(if whole { "" } else { &q });
    }

    pub fn open(&mut self, target: &Gd<Object>, v: &Values, st: &SyncState, category: Option<usize>) {
        if let Some(c) = category {
            self.current = c;
        }
        self.fill_list();
        self.show_current(target, v, st);
        let s = ui_kit::editor_scale();
        self.dialog.popup_centered_ex().minsize(Vector2i::new((680.0 * s) as i32, (440.0 * s) as i32)).done();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_terms_are_title_copy_id_and_rows() {
        let inset = CATEGORIES.iter().find(|c| c.0 == "inset").unwrap();
        let t = search_terms(inset);
        assert_eq!(&t[..3], &["Inset".to_string(), "Used by Inset Faces.".to_string(), "inset".to_string()]);
        assert!(t.iter().any(|x| x == "Inset Width"));
    }

    #[test]
    fn search_filters_categories() {
        let weld: Vec<&str> = matching("weld").iter().map(|i| CATEGORIES[*i].1).collect();
        assert_eq!(weld, vec!["Weld"]);
        assert_eq!(matching("").len(), CATEGORIES.len());
        // Row labels match too: "Harden" is a Bevel row.
        assert!(matching("harden").iter().any(|i| CATEGORIES[*i].0 == "bevel"));
        assert!(matching("zzz-nothing").is_empty());
        assert_eq!(category_of(Action::Collapse).map(|i| CATEGORIES[i].0), Some("weld"));
    }
}
