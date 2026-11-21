//! Brings together the builder's menu-bar layout and the dialogs it opens.
//!
//! The confirmation dialog and the file dialog are created the first time an action asks
//! for them, because they belong to the editor session rather than to any scene being
//! edited, and the pieces of the layout are freed together when the author switches to
//! another layout.
pub mod hud;
pub mod options;
pub mod paint;
pub mod toolbar;
use godot::classes::editor_plugin::CustomControlContainer;
use godot::classes::{
    ConfirmationDialog, Control, EditorFileDialog, EditorInterface, Material, Node,
};
use godot::prelude::*;
use super::forms::FieldForm;
use super::layout::Layout;
use super::ui::{SyncState, Ui, Values};
pub struct ModelerUi {
    pub toolbar: toolbar::Toolbar,
    pub coat: paint::Paint,
    pub choices: options::Options,
    pub hud: hud::Hud,
    pub files: Option<Gd<EditorFileDialog>>,
    pub file_purpose: (i64, i64),
    pub confirm: Option<Confirm>,
    target: Gd<Object>,
}
pub struct Confirm {
    pub dialog: Gd<ConfirmationDialog>,
    pub form: FieldForm,
    pub deed: Option<super::ui::Action>,
}
impl Confirm {
    pub fn assemble(addressee: &Gd<Object>) -> Confirm {
        let mut dialog = ConfirmationDialog::new_alloc();
        dialog.set_name("ModelerCoreConfirmation");
        dialog.set_title("Confirm");
        let form = FieldForm::new("Form");
        dialog.add_child(&form.base);
        if let Some(mut ok) = dialog.get_ok_button() {
            ok.set_text("Run");
        }
        dialog.connect(
            "confirmed",
            &Callable::from_object_method(addressee, "on_xg_confirmed"),
        );
        dialog.connect(
            "canceled",
            &Callable::from_object_method(addressee, "on_xg_confirm_canceled"),
        );
        if let Some(mut base) = EditorInterface::singleton().get_base_control() {
            base.add_child(&dialog);
        }
        Confirm {
            dialog,
            form,
            deed: None,
        }
    }
    pub fn unveil(
        &mut self,
        addressee: &Gd<Object>,
        a: super::ui::Action,
        title: &str,
        fields: &[super::ui::Field],
        v: &Values,
        st: &SyncState,
    ) {
        self.deed = Some(a);
        self.dialog.set_title(title);
        let rows: Vec<super::forms::Row> = fields
            .iter()
            .map(|f| super::forms::Row::Field(*f))
            .collect();
        self.form.set_rows(addressee, &rows, v, st);
        self.dialog.set_text(&if rows.is_empty() {
            format!("Run {title}?")
        } else {
            String::new()
        });
        self.dialog.reset_size();
        self.dialog.popup_centered();
    }
}
#[cfg(test)]
pub const TOOLBAR_ACTIONS: &[super::ui::Action] = {
    use super::ui::Action as A;
    &[
        A::ModeObject,
        A::ModeFace,
        A::ModeEdge,
        A::ModeVertex,
        A::ToolSelect,
        A::ToolShape,
        A::ToolBezier,
        A::Convert,
        A::SnapGrid,
    ]
};
#[cfg(test)]
pub const PAINT_ACTIONS: &[super::ui::Action] = {
    use super::ui::Action as A;
    &[
        A::ToolPaint,
        A::ToolVertexPaint,
        A::PaintColour,
        A::BlendMaterial,
        A::InvertAlpha,
    ]
};
#[cfg(test)]
pub const BUILDER_MENU_ACTIONS: &[super::ui::Action] = {
    use super::ui::Action as A;
    &[
        A::UvEditor,
        A::Preferences,
        A::BakeMesh,
        A::ExportObj,
        A::ExportGltf,
        A::FreezeTransform,
        A::CenterPivot,
        A::ExportStl,
        A::ExportPly,
        A::ExportOptions,
        A::ReusablePart,
        A::GenerateUv2,
        A::ExportRes,
    ]
};
#[cfg(test)]
pub const HIDDEN_FIELDS: &[super::ui::Field] = &[super::ui::Field::MirrorAxis];
pub const FILE_SLOT: i64 = 0;
pub const FILE_PALETTE_LOAD: i64 = 1;
pub const FILE_PALETTE_SAVE: i64 = 2;
impl ModelerUi {
    pub fn assemble(addressee: &Gd<Object>, v: &Values, st: &SyncState) -> ModelerUi {
        let tb = toolbar::assemble(addressee, v, st);
        let mut paint = paint::assemble(addressee);
        let mut tbx = tb.hbox.clone();
        tbx.add_child(&paint.popup);
        paint.page = 0;
        let options = options::assemble(addressee);
        if let Some(mut base) = EditorInterface::singleton().get_base_control() {
            base.add_child(&options.dialog);
        }
        let hud = hud::assemble(addressee);
        ModelerUi {
            toolbar: tb,
            coat: paint,
            choices: options,
            hud,
            files: None,
            file_purpose: (0, 0),
            confirm: None,
            target: addressee.clone(),
        }
    }
    pub fn sync(
        &mut self,
        v: &Values,
        st: &SyncState,
        materials: &[Option<Gd<Material>>],
        groups: &[(u16, usize)],
        hud: Option<hud::Content>,
    ) {
        let t = self.target.clone();
        self.toolbar.sync(&t, v, st);
        self.coat.renew(&t, v, st, materials, groups);
        if self.choices.dialog.is_visible() {
            self.choices.form.sync(v, st);
        }
        self.hud.show(hud, st);
        if self.hud.tool_options.is_visible() {
            self.hud.renew_instrument_choices(&t, v, st);
        }
        if let Some(c) = &mut self.confirm
            && c.dialog.is_visible()
        {
            c.form.sync(v, st);
        }
    }
    pub fn open_file(&mut self, purpose: i64, slot: i64) {
        use godot::classes::file_dialog::{Access, FileMode};
        if self.files.as_ref().is_none_or(|f| !f.is_instance_valid()) {
            let mut d = EditorFileDialog::new_alloc();
            d.set_name("ModelerCoreFiles");
            d.set_access(Access::RESOURCES);
            d.connect(
                "file_selected",
                &Callable::from_object_method(&self.target, "on_xg_file"),
            );
            if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                base.add_child(&d);
            }
            self.files = Some(d);
        }
        self.file_purpose = (purpose, slot);
        let Some(d) = &mut self.files else { return };
        d.clear_filters();
        match purpose {
            FILE_SLOT => {
                d.set_file_mode(FileMode::OPEN_FILE);
                d.set_title("Load Material");
                d.add_filter_ex("*.tres, *.res, *.material")
                    .description("Materials")
                    .done();
            }
            FILE_PALETTE_LOAD => {
                d.set_file_mode(FileMode::OPEN_FILE);
                d.set_title("Load Palette");
                d.add_filter_ex("*.json")
                    .description("Mesh palettes")
                    .done();
            }
            _ => {
                d.set_file_mode(FileMode::SAVE_FILE);
                d.set_title("Save Palette");
                d.add_filter_ex("*.json")
                    .description("Mesh palettes")
                    .done();
            }
        }
        d.popup_file_dialog();
    }
    fn free_nodes(&mut self, now: bool) {
        let mut nodes: Vec<Gd<Node>> = vec![
            self.toolbar.hbox.clone().upcast(),
            self.choices.dialog.clone().upcast(),
            self.hud.panel.clone().upcast(),
        ];
        if let Some(f) = self.files.take() {
            nodes.push(f.upcast());
        }
        if let Some(c) = self.confirm.take() {
            nodes.push(c.dialog.upcast());
        }
        for mut n in nodes {
            if n.is_instance_valid() {
                if now {
                    n.free();
                } else {
                    if let Some(mut p) = n.get_parent() {
                        p.remove_child(&n);
                    }
                    n.queue_free();
                }
            }
        }
    }
}
pub enum LayoutUi {
    Mst(Ui),
    Modeler(Box<ModelerUi>),
}
impl LayoutUi {
    pub fn assemble(
        layout: Layout,
        addressee: &Gd<Object>,
        v: &Values,
        st: &SyncState,
    ) -> LayoutUi {
        match layout {
            Layout::Mst => LayoutUi::Mst(super::ui::assemble(addressee, v)),
            Layout::Modeler => LayoutUi::Modeler(Box::new(ModelerUi::assemble(addressee, v, st))),
        }
    }
    pub fn layout(&self) -> Layout {
        match self {
            LayoutUi::Mst(_) => Layout::Mst,
            LayoutUi::Modeler(_) => Layout::Modeler,
        }
    }
    pub fn controls(&self) -> Vec<(CustomControlContainer, Gd<Control>)> {
        match self {
            LayoutUi::Mst(u) => {
                let (tb, strip, pal) = u.controls();
                vec![
                    (CustomControlContainer::SPATIAL_EDITOR_SIDE_LEFT, tb),
                    (CustomControlContainer::SPATIAL_EDITOR_BOTTOM, strip),
                    (CustomControlContainer::SPATIAL_EDITOR_SIDE_RIGHT, pal),
                ]
            }
            LayoutUi::Modeler(x) => vec![(
                CustomControlContainer::SPATIAL_EDITOR_MENU,
                x.toolbar.hbox.clone().upcast(),
            )],
        }
    }
    pub fn assign_shown(&mut self, shown: bool, has_target: bool) {
        match self {
            LayoutUi::Mst(u) => u.assign_shown(shown, has_target),
            LayoutUi::Modeler(x) => {
                x.toolbar.hbox.set_visible(shown);
                if !shown {
                    x.hud.panel.set_visible(false);
                    x.coat.popup.hide();
                    x.toolbar.handles_popup.hide();
                    x.toolbar.palette.hide();
                }
            }
        }
    }
    pub fn rebuild_slots(&mut self, materials: &[Option<Gd<Material>>], current: i64) {
        match self {
            LayoutUi::Mst(u) => u.rebuild_slots(materials, current),
            LayoutUi::Modeler(x) => x.coat.invalidate(),
        }
    }
    pub fn assign_pat(&mut self, i: usize, c: Color) {
        match self {
            LayoutUi::Mst(u) => u.assign_pat(i, c),
            LayoutUi::Modeler(x) => x.coat.invalidate(),
        }
    }
    pub fn refresh_tooltips(&mut self) {
        if let LayoutUi::Mst(u) = self {
            u.refresh_tooltips();
        }
    }
    pub fn free(self, now: bool) {
        match self {
            LayoutUi::Mst(u) => {
                let (tb, strip, pal) = u.controls();
                for mut c in [tb, strip, pal] {
                    if c.is_instance_valid() {
                        if now {
                            c.free();
                        } else {
                            c.queue_free();
                        }
                    }
                }
            }
            LayoutUi::Modeler(mut x) => x.free_nodes(now),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::commands;
    use super::super::forms::Row;
    use super::super::ui::{self, Action, Field, Mode, UvField};
    use super::*;
    fn all_rows() -> Vec<Row> {
        let mut rows: Vec<Row> = toolbar::HANDLES_ROWS.to_vec();
        rows.extend(paint::PAINT_ROWS.iter().copied());
        rows.extend(paint::SMOOTHING_ROWS.iter().copied());
        for c in options::CATEGORIES {
            rows.extend(c.3.iter().copied());
        }
        rows
    }
    fn in_menu(a: Action) -> bool {
        [Mode::Object, Mode::Face, Mode::Edge, Mode::Vertex]
            .iter()
            .any(|m| {
                commands::actions_menu(*m, |x, mask| {
                    mask & m.bit() != 0 || x == Action::Preferences
                })
                .iter()
                .any(|s| s.entries.iter().any(|e| e.deed == a))
            })
    }
    #[test]
    fn every_builder_action_has_a_modeler_home() {
        let rows = all_rows();
        let toggled = |a: Action| {
            rows.iter()
                .any(|r| matches!(r, Row::Toggle(x, _) if *x == a))
        };
        for (a, _) in ui::toolbar_actions().chain(ui::MENU_ACTIONS.iter().map(|m| (m.0, m.2))) {
            let home = in_menu(a)
                || TOOLBAR_ACTIONS.contains(&a)
                || PAINT_ACTIONS.contains(&a)
                || BUILDER_MENU_ACTIONS.contains(&a)
                || toggled(a);
            assert!(home, "{a:?} has no home in the Modeler layout");
        }
        for a in commands::OWNED {
            assert!(
                TOOLBAR_ACTIONS.contains(a)
                    || PAINT_ACTIONS.contains(a)
                    || BUILDER_MENU_ACTIONS.contains(a)
                    || toggled(*a)
                    || in_menu(*a),
                "{a:?} owned by nothing"
            );
        }
    }
    #[test]
    fn every_field_has_a_modeler_home() {
        let rows = all_rows();
        for f in Field::ALL {
            let home =
                rows.contains(&Row::Field(f)) || ui::is_tool_field(f) || HIDDEN_FIELDS.contains(&f);
            assert!(home, "{f:?} has no home in the Modeler layout");
        }
        assert!(
            hud::tool_rows().contains(&Row::ShapeSmooth)
                && hud::tool_rows().contains(&Row::ShapeArched)
        );
        for u in UvField::ALL {
            assert!(rows.contains(&Row::Uv(u)), "{u:?} missing from Options");
        }
    }
}
