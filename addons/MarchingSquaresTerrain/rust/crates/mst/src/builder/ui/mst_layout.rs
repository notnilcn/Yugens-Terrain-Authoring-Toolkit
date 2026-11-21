//! Builds the builder's side-panel layout: a tool column, a strip of shape and tool
//! settings, and a material palette.
//!
//! The layout is built when the author picks it and freed when they switch away, because
//! it lives in the editor's own panels rather than in any scene being edited. Its tool
//! buttons and setting fields are generated from the builder's command and field tables,
//! and the rows for material slots and smoothing groups are rebuilt from the selected
//! mesh, whose number of slots and groups is only known while editing.
use super::*;
pub struct Ui {
    pub toolbar: Gd<VFlowContainer>,
    pub peel: Gd<HFlowContainer>,
    pub palette: Gd<ScrollContainer>,
    pub buttons: Vec<(Action, Gd<Button>, u8)>,
    sections: Vec<(usize, Gd<HSeparator>, Gd<Label>)>,
    pub fields: Vec<(Field, Gd<HBoxContainer>, Gd<SpinBox>)>,
    pub choices: Vec<(Field, Gd<HBoxContainer>, Gd<OptionButton>)>,
    pub checks: Vec<(Field, Gd<CheckBox>)>,
    pub arched: Gd<CheckBox>,
    pub kind_box: Gd<HBoxContainer>,
    pub sort: Gd<OptionButton>,
    pub glide: Gd<CheckBox>,
    pub tint: Gd<ColorPickerButton>,
    pub facts: Gd<Label>,
    pub slots: Gd<VBoxContainer>,
    pub slot_buttons: Vec<Gd<Button>>,
    pub slot_group: Gd<ButtonGroup>,
    pub uv_fields: Vec<(UvField, Gd<SpinBox>)>,
    pub uv_world: Gd<CheckBox>,
    pub chips: Gd<HFlowContainer>,
    pub chip_buttons: Vec<(u16, Gd<Button>)>,
    pub chip_hint: Gd<Label>,
    pub chip_menu: Gd<PopupMenu>,
    pub attr_fields: Vec<(Field, FieldCtl)>,
    pub swatches: Vec<Gd<Button>>,
    pub hover: Gd<Label>,
    target: Gd<Object>,
}
const CELL_PX: f32 = 36.0;
fn label(caption: &str) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.set_text(caption);
    l
}
fn spin(min: f64, max: f64, step: f64, value: f64, tip: &str, width: f32) -> Gd<SpinBox> {
    let mut sb = SpinBox::new_alloc();
    sb.set_min(min);
    sb.set_max(max);
    sb.set_step(step);
    sb.set_value_no_signal(value);
    sb.set_tooltip_text(tip);
    sb.set_custom_minimum_size(Vector2::new(width, 0.0));
    sb
}
pub enum FieldCtl {
    Spin(Gd<HBoxContainer>, Gd<SpinBox>),
    Option(Gd<HBoxContainer>, Gd<OptionButton>),
    Check(Gd<CheckBox>),
}
impl FieldCtl {
    pub fn widget(&self) -> Gd<Control> {
        match self {
            FieldCtl::Spin(hb, _) | FieldCtl::Option(hb, _) => hb.clone().upcast(),
            FieldCtl::Check(cb) => cb.clone().upcast(),
        }
    }
    pub fn sync(&mut self, f: Field, v: &Values) {
        match self {
            FieldCtl::Spin(_, sb) => sb.set_value_no_signal(v.fetch(f)),
            FieldCtl::Option(_, ob) => ob.select(v.fetch(f) as i32),
            FieldCtl::Check(cb) => cb.set_pressed_no_signal(v.fetch(f) >= 0.5),
        }
    }
}
pub fn make_field(addressee: &Gd<Object>, f: Field, v: &Values) -> FieldCtl {
    let (text, min, max, step, tip) = f.portray();
    if f.is_check() {
        let mut cb = CheckBox::new_alloc();
        cb.set_text(text);
        cb.set_tooltip_text(tip);
        cb.set_focus_mode(FocusMode::NONE);
        cb.set_pressed_no_signal(v.fetch(f) >= 0.5);
        cb.connect(
            "toggled",
            &Callable::from_object_method(addressee, "on_field_check")
                .bind(&[(f as i64).to_variant()]),
        );
        return FieldCtl::Check(cb);
    }
    if let Some(choices) = f.choices() {
        let mut hb = HBoxContainer::new_alloc();
        let mut l = label(text);
        l.set_tooltip_text(tip);
        hb.add_child(&l);
        let mut ob = OptionButton::new_alloc();
        for c in choices {
            ob.add_item(*c);
        }
        ob.select(v.fetch(f) as i32);
        ob.set_tooltip_text(tip);
        ob.set_focus_mode(FocusMode::NONE);
        ob.connect(
            "item_selected",
            &Callable::from_object_method(addressee, "on_field_option")
                .bind(&[(f as i64).to_variant()]),
        );
        hb.add_child(&ob);
        return FieldCtl::Option(hb, ob);
    }
    let mut hb = HBoxContainer::new_alloc();
    let mut l = label(text);
    l.set_tooltip_text(tip);
    hb.add_child(&l);
    let mut sb = spin(min, max, step, v.fetch(f), tip, 64.0);
    sb.set_allow_greater(matches!(
        f,
        Field::SizeX | Field::SizeY | Field::SizeZ | Field::Distance | Field::Bevel
    ));
    sb.connect(
        "value_changed",
        &Callable::from_object_method(addressee, "on_field").bind(&[(f as i64).to_variant()]),
    );
    hb.add_child(&sb);
    FieldCtl::Spin(hb, sb)
}
pub fn assemble(addressee: &Gd<Object>, v: &Values) -> Ui {
    let mut toolbar = VFlowContainer::new_alloc();
    toolbar.set_name("PixelBuilderToolbar");
    toolbar.set_custom_minimum_size(ui_kit::sz(CELL_PX, CELL_PX));
    toolbar.set_alignment(godot::classes::flow_container::AlignmentMode::BEGIN);
    let mut buttons = Vec::new();
    let mut sections = Vec::new();
    let groups = [
        ButtonGroup::new_gd(),
        ButtonGroup::new_gd(),
        ButtonGroup::new_gd(),
    ];
    let mut last_section = None;
    let mut ordered: Vec<&(Action, &str, &str, u8, u8)> = BUTTONS.iter().collect();
    ordered.sort_by_key(|b| section_of(b.0, b.3));
    for (a, ic, tip, group, mask) in ordered {
        let sec = section_of(*a, *group);
        if last_section != Some(sec) {
            let mut sep = HSeparator::new_alloc();
            sep.set_name(&format!("{}Separator", SECTIONS[sec].0));
            toolbar.add_child(&sep);
            let mut head = Label::new_alloc();
            head.set_name(&format!("{}Header", SECTIONS[sec].0));
            head.set_text(SECTIONS[sec].0);
            head.set_tooltip_text(SECTIONS[sec].1);
            head.set_mouse_filter(godot::classes::control::MouseFilter::PASS);
            head.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
            head.add_theme_font_size_override("font_size", (11.0 * ui_kit::editor_scale()) as i32);
            head.add_theme_color_override("font_color", Color::from_rgb(0.6205, 0.7805, 0.9505));
            head.set_custom_minimum_size(ui_kit::sz(CELL_PX * 2.0, 0.0));
            toolbar.add_child(&head);
            sections.push((sec, sep, head));
            last_section = Some(sec);
        }
        let mut b = ui_kit::instrument_clicker(
            &format!("{a:?}"),
            &format_tooltip(*a, tip, *mask),
            glyph(ic),
        );
        b.set_custom_minimum_size(ui_kit::sz(CELL_PX - 2.0, CELL_PX - 2.0));
        b.set_expand_icon(false);
        for state in [
            "normal",
            "hover",
            "pressed",
            "hover_pressed",
            "disabled",
            "focus",
        ] {
            let theme = godot::classes::EditorInterface::singleton().get_editor_theme();
            let src = theme.and_then(|t| t.get_stylebox(state, "Button"));
            if let Some(sb) = src.map(|x| x.duplicate_resource()) {
                let mut sb = sb;
                sb.set_content_margin_all(2.0 * ui_kit::editor_scale());
                b.add_theme_stylebox_override(state, &sb);
            }
        }
        if *group == 1 || *group == 2 || *group == 7 {
            b.set_button_group(&groups[if *group == 7 { 2 } else { *group as usize - 1 }]);
        } else if *group != 3 {
            b.set_toggle_mode(false);
        }
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "upon_deed").bind(&[(*a as i64).to_variant()]),
        );
        b.connect(
            "mouse_entered",
            &Callable::from_object_method(addressee, "on_button_hover")
                .bind(&[(*a as i64).to_variant()]),
        );
        b.connect(
            "mouse_exited",
            &Callable::from_object_method(addressee, "on_button_hover")
                .bind(&[(-1i64).to_variant()]),
        );
        toolbar.add_child(&ui_kit::midpoint(ui_kit::sz(CELL_PX, CELL_PX), &b));
        buttons.push((*a, b, *mask));
    }
    let mut strip = HFlowContainer::new_alloc();
    strip.set_name("PixelBuilderStrip");
    strip.set_h_size_flags(SizeFlags::EXPAND_FILL);
    let mut hover = Label::new_alloc();
    hover.set_name("PixelBuilderHover");
    hover.add_theme_color_override("font_color", Color::from_rgb(0.6205, 0.7805, 0.9505));
    hover.set_custom_minimum_size(Vector2::new(150.0 * ui_kit::editor_scale(), 0.0));
    hover.set_tooltip_text("Name and shortcut of the builder button under the mouse");
    strip.add_child(&hover);
    let mut kind_box = HBoxContainer::new_alloc();
    kind_box.add_child(&label("Shape"));
    let mut kind = OptionButton::new_alloc();
    for k in ShapeKind::ALL {
        match glyph(shape_icon(k)) {
            Some(t) => kind.add_icon_item(&t, k.moniker()),
            None => kind.add_item(k.moniker()),
        }
    }
    kind.select(v.spec.sort as i32);
    kind.set_focus_mode(FocusMode::NONE);
    kind.connect(
        "item_selected",
        &Callable::from_object_method(addressee, "on_kind"),
    );
    kind_box.add_child(&kind);
    strip.add_child(&kind_box);
    let mut smooth = CheckBox::new_alloc();
    smooth.set_text("Smooth");
    smooth.set_pressed_no_signal(v.spec.glide);
    smooth.set_tooltip_text("Smooth shading for round shapes");
    smooth.connect(
        "toggled",
        &Callable::from_object_method(addressee, "on_smooth"),
    );
    let mut color = ColorPickerButton::new_alloc();
    color.set_custom_minimum_size(Vector2::new(28.0, 0.0));
    color.set_pick_color(v.tint);
    color.set_tooltip_text("Face color (multiplies the slot material)");
    color.connect(
        "color_changed",
        &Callable::from_object_method(addressee, "on_color"),
    );
    let mut arched = CheckBox::new_alloc();
    arched.set_text("Arched");
    arched.set_pressed_no_signal(v.spec.arched);
    arched.set_tooltip_text("Round-topped doorway");
    arched.connect(
        "toggled",
        &Callable::from_object_method(addressee, "on_arched"),
    );
    let mut swatch_box = HBoxContainer::new_alloc();
    swatch_box.add_theme_constant_override("separation", 2);
    let mut swatches = Vec::new();
    for (i, c) in v.swatches.iter().enumerate() {
        let mut b = Button::new_alloc();
        b.set_custom_minimum_size(ui_kit::sz(14.0, 20.0));
        b.set_focus_mode(FocusMode::NONE);
        b.set_tooltip_text("Color swatch: click to use it, Ctrl+click to store the current color in it, Alt+click to select the faces painted with it");
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "on_swatch").bind(&[(i as i64).to_variant()]),
        );
        style_swatch(&mut b, *c);
        swatch_box.add_child(&b);
        swatches.push(b);
    }
    let mut fields = Vec::new();
    let mut options = Vec::new();
    let mut checks = Vec::new();
    for f in Field::ALL {
        match make_field(addressee, f, v) {
            FieldCtl::Check(cb) => {
                strip.add_child(&cb);
                checks.push((f, cb));
            }
            FieldCtl::Option(hb, ob) => {
                strip.add_child(&hb);
                options.push((f, hb, ob));
            }
            FieldCtl::Spin(hb, sb) => {
                if f == Field::Slot {
                    strip.add_child(&VSeparator::new_alloc());
                }
                strip.add_child(&hb);
                if f == Field::Slot {
                    let mut cl = label("Color");
                    cl.set_tooltip_text("Paint color (picker) and swatches: click a swatch to use it, Ctrl+click to store the current color in it");
                    strip.add_child(&cl);
                    strip.add_child(&color);
                    strip.add_child(&swatch_box);
                    strip.add_child(&VSeparator::new_alloc());
                }
                if f == Field::Rings {
                    strip.add_child(&smooth);
                }
                if f == Field::DoorHeight {
                    strip.add_child(&arched);
                }
                fields.push((f, hb, sb));
            }
        }
    }
    let mut info = Label::new_alloc();
    info.add_theme_font_size_override("font_size", 12);
    info.set_v_size_flags(SizeFlags::SHRINK_CENTER);
    strip.add_child(&info);
    let mut palette = ScrollContainer::new_alloc();
    palette.set_name("PixelBuilderPalette");
    palette.set_custom_minimum_size(Vector2::new(210.0, 0.0));
    palette.set_horizontal_scroll_mode(godot::classes::scroll_container::ScrollMode::DISABLED);
    let mut col = VBoxContainer::new_alloc();
    col.set_h_size_flags(SizeFlags::EXPAND_FILL);
    let mut title = label("Materials");
    title.set_tooltip_text("Material slots of the selected builder mesh.\nClick a slot to make it current (and assign it to the selected faces).\nDrop or load any Material into a slot's picker.");
    col.add_child(&title);
    let slots = VBoxContainer::new_alloc();
    col.add_child(&slots);
    let mut add = Button::new_alloc();
    add.set_text("+ Slot");
    add.set_tooltip_text("Add a material slot (a vertex-color StandardMaterial3D you can replace)");
    add.set_focus_mode(FocusMode::NONE);
    add.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_add_slot"),
    );
    let mut paint_row = HFlowContainer::new_alloc();
    paint_row.set_name("PaintRow");
    paint_row.add_child(&add);
    let mut presets = MenuButton::new_alloc();
    presets.set_name("Presets");
    presets.set_text("+ Preset");
    presets.set_flat(false);
    presets.set_tooltip_text("Add a slot with a built-in material: Grid, Checker (patterned, vertex colour),\nNo Draw (transparent), Collider (red), Trigger (orange)");
    if let Some(mut p) = presets.get_popup() {
        for (i, pr) in pt_core::build::xg_paint::Preset::ALL.iter().enumerate() {
            p.add_item_ex(pr.heading()).id(i as i32).done();
        }
        p.connect(
            "id_pressed",
            &Callable::from_object_method(addressee, "on_preset_menu"),
        );
    }
    paint_row.add_child(&presets);
    let mut pal = MenuButton::new_alloc();
    pal.set_name("PaletteMenu");
    pal.set_text("Palette");
    pal.set_flat(false);
    pal.set_tooltip_text("Palette files: the slots' materials and the colour swatches.\nSave / Load as JSON (Modeler Modeler's palette JSON) or as a .tres resource (Modeler's palette resource)");
    if let Some(mut p) = pal.get_popup() {
        p.add_item_ex("Load Palette…")
            .id(PALETTE_LOAD as i32)
            .done();
        p.add_item_ex("Save Palette…")
            .id(PALETTE_SAVE as i32)
            .done();
        p.add_item_ex("Reset Colours")
            .id(PALETTE_RESET as i32)
            .done();
        p.set_item_tooltip(2, "Swatches back to the default colours (the slots stay)");
        p.connect(
            "id_pressed",
            &Callable::from_object_method(addressee, "on_palette_menu"),
        );
    }
    paint_row.add_child(&pal);
    let mut sample = Button::new_alloc();
    sample.set_name("Sample");
    sample.set_text("Sample");
    sample.set_tooltip_text("Sample from selection: the slot, colour and smoothing group most selected faces use\nbecome current (all faces in Object mode)");
    sample.set_focus_mode(FocusMode::NONE);
    sample.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_paint_sample"),
    );
    paint_row.add_child(&sample);
    col.add_child(&paint_row);
    col.add_child(&HSeparator::new_alloc());
    let mut smt = label("Smoothing");
    smt.set_tooltip_text("Smoothing groups: faces in one group shade smooth across their shared edges; group 0 is faceted.\nThe actions use the selected faces (all faces in Object mode).");
    col.add_child(&smt);
    let mut srow = HFlowContainer::new_alloc();
    srow.set_name("SmoothingRow");
    for (text, cmd, tip) in [
        (
            "Smooth",
            0i64,
            "Smooth: the selected faces join the current group (the outlined chip), else a new one",
        ),
        (
            "Faceted",
            1,
            "Faceted: flat shading for the selected faces (group 0)",
        ),
        (
            "New group",
            2,
            "New group from selection: the selected faces get the next free group (1-23)",
        ),
    ] {
        let mut b = Button::new_alloc();
        b.set_text(text);
        b.set_tooltip_text(tip);
        b.set_focus_mode(FocusMode::NONE);
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "on_xg_smoothing")
                .bind(&[cmd.to_variant(), 0i64.to_variant()]),
        );
        srow.add_child(&b);
    }
    col.add_child(&srow);
    let mut chips = HFlowContainer::new_alloc();
    chips.set_name("Chips");
    col.add_child(&chips);
    let mut chip_hint = label("No smoothing groups in this mesh.");
    chip_hint.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.6005));
    col.add_child(&chip_hint);
    let mut chip_menu = PopupMenu::new_alloc();
    chip_menu.set_name("ChipMenu");
    chip_menu.connect(
        "id_pressed",
        &Callable::from_object_method(addressee, "on_xg_paint_menu"),
    );
    col.add_child(&chip_menu);
    let mut attr_row = HFlowContainer::new_alloc();
    attr_row.set_name("SmoothingDisplay");
    let mut attr_fields = Vec::new();
    for f in [Field::SmoothTint, Field::ShowNormals, Field::NormalLength] {
        let c = make_field(addressee, f, v);
        attr_row.add_child(&c.widget());
        attr_fields.push((f, c));
    }
    col.add_child(&attr_row);
    col.add_child(&HSeparator::new_alloc());
    let mut uvt = label("Face UV");
    uvt.set_tooltip_text("Automatic box projection per face: every face is mapped along its dominant axis.\nWorld aligned keeps textures continuous across meshes.\nSelecting one face loads its values; Apply writes them to the selected faces.");
    col.add_child(&uvt);
    let mut grid = GridContainer::new_alloc();
    grid.set_columns(2);
    let mut uv_fields = Vec::new();
    for (i, f) in UvField::ALL.iter().enumerate() {
        let (text, min, max, step) = f.portray();
        grid.add_child(&label(text));
        let mut sb = spin(min, max, step, v.tex_coord[i] as f64, text, 90.0);
        sb.set_allow_greater(true);
        sb.set_allow_lesser(true);
        sb.connect(
            "value_changed",
            &Callable::from_object_method(addressee, "on_uv_field")
                .bind(&[(*f as i64).to_variant()]),
        );
        grid.add_child(&sb);
        uv_fields.push((*f, sb));
    }
    col.add_child(&grid);
    let mut uv_world = CheckBox::new_alloc();
    uv_world.set_text("World aligned");
    uv_world.set_pressed_no_signal(v.uv_world);
    uv_world.connect(
        "toggled",
        &Callable::from_object_method(addressee, "on_uv_world"),
    );
    col.add_child(&uv_world);
    let mut row = HBoxContainer::new_alloc();
    for (text, method, tip) in [
        (
            "Apply",
            "on_uv_apply",
            "Apply the UV values to the selected faces (all faces in Object mode)",
        ),
        ("Reset", "on_uv_reset", "Reset the UV values (and apply)"),
        (
            "↻ 90°",
            "on_uv_rotate",
            "Rotate the selected faces' UVs by 90°",
        ),
    ] {
        let mut b = Button::new_alloc();
        b.set_text(text);
        b.set_tooltip_text(tip);
        b.set_focus_mode(FocusMode::NONE);
        b.connect("pressed", &Callable::from_object_method(addressee, method));
        row.add_child(&b);
    }
    col.add_child(&row);
    palette.add_child(&col);
    let mut ui = Ui {
        toolbar,
        peel: strip,
        palette,
        buttons,
        sections,
        fields,
        choices: options,
        checks,
        arched,
        kind_box,
        sort: kind,
        glide: smooth,
        tint: color,
        facts: info,
        swatches,
        hover,
        slots,
        slot_buttons: Vec::new(),
        slot_group: ButtonGroup::new_gd(),
        uv_fields,
        uv_world,
        chips,
        chip_buttons: Vec::new(),
        chip_hint,
        chip_menu,
        attr_fields,
        target: addressee.clone(),
    };
    ui.show_fields(v, &SyncState::default());
    ui
}
impl Ui {
    pub fn assign_pat(&mut self, i: usize, c: Color) {
        if let Some(b) = self.swatches.get_mut(i) {
            style_swatch(b, c);
        }
    }
    pub fn show_fields(&mut self, v: &Values, st: &SyncState) {
        let shape = st.shape_fields();
        for (f, hb, _) in &mut self.fields {
            hb.set_visible(field_visible(*f, v, st));
        }
        for (f, hb, _) in &mut self.choices {
            hb.set_visible(field_visible(*f, v, st));
        }
        for (f, cb) in &mut self.checks {
            cb.set_visible(field_visible(*f, v, st));
        }
        let (_, _, _, smooth, _) = v.spec.sort.uses();
        let (_, door, _, _) = v.spec.sort.uses_extra();
        let band = v.spec.sort.uses_modeler().arch_band && v.spec.arch_band;
        self.kind_box.set_visible(shape);
        self.glide.set_visible((smooth || band) && shape);
        self.arched.set_visible(door && shape);
    }
    pub fn sync(&mut self, v: &Values, st: &SyncState) {
        let mode = st.mode;
        for (f, _, sb) in &mut self.fields {
            sb.set_value_no_signal(v.fetch(*f));
        }
        for (f, _, ob) in &mut self.choices {
            ob.select(v.fetch(*f) as i32);
        }
        for (f, cb) in &mut self.checks {
            cb.set_pressed_no_signal(v.fetch(*f) >= 0.5);
        }
        self.arched.set_pressed_no_signal(v.spec.arched);
        self.sort.select(v.spec.sort as i32);
        self.glide.set_pressed_no_signal(v.spec.glide);
        self.tint.set_pick_color(v.tint);
        self.show_fields(v, st);
        for (i, (_, sb)) in self.uv_fields.iter_mut().enumerate() {
            sb.set_value_no_signal(v.tex_coord[i] as f64);
        }
        self.uv_world.set_pressed_no_signal(v.uv_world);
        for (f, c) in &mut self.attr_fields {
            c.sync(*f, v);
            if *f == Field::NormalLength {
                c.widget().set_visible(v.xg.show_normals);
            }
        }
        for (a, b, mask) in &mut self.buttons {
            let visible = action_visible(*a, *mask, st);
            b.set_visible(visible);
            if *a == Action::Dissolve {
                let what = match mode {
                    Mode::Face => {
                        "Dissolve faces: merge the selected faces into one n-gon (their inner edges and vertices go)"
                    }
                    Mode::Edge => {
                        "Dissolve edges: remove the selected edges, joining the two faces of each"
                    }
                    _ => {
                        "Dissolve vertices: remove the selected vertices, merging the faces around each into one"
                    }
                };
                b.set_tooltip_text(&format_tooltip(*a, what, *mask));
            }
            if let Some(mut cell) = b.get_parent().and_then(|p| p.try_cast::<Control>().ok()) {
                cell.set_visible(visible);
            }
            let Some(on) = action_pressed(*a, v, st) else {
                continue;
            };
            b.set_pressed_no_signal(on);
        }
        for (sec, sep, head) in &mut self.sections {
            let any = self.buttons.iter().any(|(a, b, _)| {
                b.is_visible()
                    && BUTTONS
                        .iter()
                        .find(|x| x.0 == *a)
                        .is_some_and(|x| section_of(*a, x.3) == *sec)
            });
            sep.set_visible(any);
            head.set_visible(any);
        }
        for (i, b) in self.slot_buttons.iter_mut().enumerate() {
            b.set_pressed_no_signal(i as i64 == v.pocket);
        }
    }
    pub fn rebuild_slots(&mut self, materials: &[Option<Gd<Material>>], current: i64) {
        for mut c in self.slots.get_children().iter_shared() {
            self.slots.remove_child(&c);
            c.queue_free();
        }
        self.slot_buttons.clear();
        self.slot_group = ButtonGroup::new_gd();
        for (i, m) in materials.iter().enumerate() {
            let mut row = HBoxContainer::new_alloc();
            let mut b = Button::new_alloc();
            b.set_text(&format!("{i}"));
            b.set_toggle_mode(true);
            b.set_button_group(&self.slot_group);
            b.set_custom_minimum_size(Vector2::new(28.0, 28.0));
            b.set_focus_mode(FocusMode::NONE);
            b.set_tooltip_text(&format!(
                "Slot {i}: make current; assigns it to the selected faces"
            ));
            b.set_pressed_no_signal(i as i64 == current);
            if let Some(c) = m.as_ref().and_then(material_color) {
                b.add_theme_color_override(
                    "font_color",
                    if c.luminance() > 0.5 {
                        Color::BLACK
                    } else {
                        Color::WHITE
                    },
                );
                b.add_theme_color_override(
                    "font_pressed_color",
                    Color::from_rgb(1.0, 0.7005, 0.2005),
                );
                let mut sb = godot::classes::StyleBoxFlat::new_gd();
                sb.set_bg_color(c);
                b.add_theme_stylebox_override("normal", &sb);
            }
            b.connect(
                "pressed",
                &Callable::from_object_method(&self.target, "on_slot_pick")
                    .bind(&[(i as i64).to_variant()]),
            );
            row.add_child(&b);
            let mut picker = EditorResourcePicker::new_alloc();
            picker.set_base_type("Material");
            picker.set_h_size_flags(SizeFlags::EXPAND_FILL);
            if let Some(m) = m {
                picker.set_edited_resource(m);
            }
            picker.connect(
                "resource_changed",
                &Callable::from_object_method(&self.target, "on_slot_material")
                    .bind(&[(i as i64).to_variant()]),
            );
            row.add_child(&picker);
            let mut sel = Button::new_alloc();
            sel.set_text("Sel");
            sel.set_flat(true);
            sel.set_focus_mode(FocusMode::NONE);
            sel.set_tooltip_text(&format!("Select every face using slot {i} (Face mode)"));
            sel.connect(
                "pressed",
                &Callable::from_object_method(&self.target, "on_select_slot")
                    .bind(&[(i as i64).to_variant()]),
            );
            row.add_child(&sel);
            self.slots.add_child(&row);
            self.slot_buttons.push(b);
        }
    }
    pub fn rebuild_chips(&mut self, groups: &[(u16, usize)], current: u16) {
        for mut c in self.chips.get_children().iter_shared() {
            self.chips.remove_child(&c);
            c.queue_free();
        }
        self.chip_buttons.clear();
        for (g, n) in groups {
            let mut b = Button::new_alloc();
            b.set_text(&format!("{g} ({n})"));
            b.set_toggle_mode(true);
            b.set_pressed_no_signal(*g == current);
            b.set_focus_mode(FocusMode::NONE);
            b.set_tooltip_text(&format!(
                "Smoothing group {g}, {n} face{}.\nClick: select its faces (and make it current for Smooth).\nRight-click: select, merge into the selected faces' group, or clear",
                if *n == 1 { "" } else { "s" }
            ));
            let c = pt_core::build::xg_paint::gliding_cluster_tint(*g);
            let col = Color::from_rgba(c[0], c[1], c[2], 1.0);
            let style = |border: Color, width: i32| {
                let mut sb = godot::classes::StyleBoxFlat::new_gd();
                sb.set_bg_color(col.darkened(0.5505));
                sb.set_border_color(border);
                sb.set_border_width_all(width);
                sb.set_corner_radius_all(4);
                sb.set_content_margin_all(3.0);
                sb
            };
            let (normal, on) = (
                style(col, 1),
                style(Color::from_rgb(1.0, 0.7005, 0.2005), 2),
            );
            b.add_theme_stylebox_override("normal", &normal);
            b.add_theme_stylebox_override("hover", &normal);
            b.add_theme_stylebox_override("pressed", &on);
            b.add_theme_stylebox_override("hover_pressed", &on);
            b.add_theme_color_override("font_color", col.lightened(0.3005));
            b.add_theme_color_override("font_pressed_color", Color::WHITE);
            b.connect(
                "pressed",
                &Callable::from_object_method(&self.target, "on_xg_smoothing")
                    .bind(&[3i64.to_variant(), (*g as i64).to_variant()]),
            );
            b.connect(
                "gui_input",
                &Callable::from_object_method(&self.target, "on_chip_input")
                    .bind(&[(*g as i64).to_variant()]),
            );
            self.chips.add_child(&b);
            self.chip_buttons.push((*g, b));
        }
        self.chips.set_visible(!groups.is_empty());
        self.chip_hint.set_visible(groups.is_empty());
    }
    pub fn sync_chips(&mut self, current: u16) {
        for (g, b) in &mut self.chip_buttons {
            b.set_pressed_no_signal(*g == current);
        }
    }
    pub fn open_chip_menu(&mut self, g: i64, at: Vector2) {
        use super::super::modeler_ui::paint::menu as m;
        self.chip_menu.clear();
        for (t, cmd) in [
            ("Select Faces", m::GROUP_SELECT),
            ("Merge into Selection's Group", m::GROUP_MERGE),
            ("Clear Group (faceted)", m::GROUP_CLEAR),
        ] {
            self.chip_menu
                .add_item_ex(t)
                .id((cmd * 1000 + g) as i32)
                .done();
        }
        self.chip_menu.reset_size();
        self.chip_menu
            .popup_ex()
            .rect(Rect2i::new(
                Vector2i::new(at.x as i32, at.y as i32),
                Vector2i::ZERO,
            ))
            .done();
    }
    pub fn refresh_tooltips(&mut self) {
        for (a, b, mask) in &mut self.buttons {
            if *a == Action::Dissolve {
                continue;
            }
            if let Some(row) = BUTTONS.iter().find(|x| x.0 == *a) {
                b.set_tooltip_text(&format_tooltip(*a, row.2, *mask));
            }
        }
    }
    pub fn assign_shown(&mut self, v: bool, palette: bool) {
        self.toolbar.set_visible(v);
        self.peel.set_visible(v);
        self.palette.set_visible(v && palette);
    }
    pub fn controls(&self) -> (Gd<Control>, Gd<Control>, Gd<Control>) {
        (
            self.toolbar.clone().upcast(),
            self.peel.clone().upcast(),
            self.palette.clone().upcast(),
        )
    }
}
