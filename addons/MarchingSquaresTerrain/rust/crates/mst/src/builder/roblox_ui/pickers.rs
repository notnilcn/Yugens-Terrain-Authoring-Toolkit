//! The ribbon's Material, Color and Align popups as seen from the builder: it takes their
//! declared controls from the ribbon scene, routes what the person does in them to the builder, and shows the
//! values the builder hands back. What each choice does to the scene is decided elsewhere.
use super::super::modeler_ui::toolbar::popup_below;
use super::super::ui;
use super::align::AlignMode;
use super::material_catalog;
use super::palette::Honeycomb;
use super::ribbon_root::PixelRobloxRibbon;
use godot::classes::image::Format;
use godot::classes::item_list::IconMode;
use godot::classes::object::ConnectFlags;
use godot::classes::{
    BaseButton, Button, ColorPickerButton, EditorInterface, Image, ImageTexture, ItemList, Label,
    Material, OptionButton, PopupPanel, TabBar, Texture2D, TextureRect, Window,
};
use godot::prelude::*;
const HEX_RADIUS: f32 = 12.0;
const GRID_COLUMN_WIDTH: i32 = 88;
const MARK_DISTANCE: f32 = 0.0004;
pub fn chip(c: Color) -> Option<Gd<Texture2D>> {
    let mut img = Image::create_empty(16, 16, false, Format::RGBA8)?;
    img.fill(Color::from_rgba(c.r, c.g, c.b, 1.0));
    ImageTexture::create_from_image(&img).map(|t| t.upcast())
}
pub fn material_name(m: &Gd<Material>) -> String {
    let path = m.get_path().to_string();
    let name = m.get_name().to_string();
    if !name.is_empty() {
        return name;
    }
    if !path.is_empty() && !path.contains("::") {
        return material_catalog::display_name(&path);
    }
    match ui::material_color(m) {
        Some(c) => format!("Material #{}", c.to_html_without_alpha()),
        None => "Material".to_string(),
    }
}
pub fn material_icon(m: &Gd<Material>) -> Option<Gd<Texture2D>> {
    ui::material_color(m).and_then(chip)
}
fn popup_near(mut w: Gd<Window>, anchor: Option<Gd<Button>>) {
    let Some(b) = anchor else {
        w.popup_centered();
        return;
    };
    let below = popup_below(&b.clone().upcast());
    w.reset_size();
    let size = w.get_size();
    let x = match b.get_window() {
        Some(host) => {
            let right = host.get_position().x + host.get_size().x;
            below.position.x.min(right - size.x).max(host.get_position().x)
        }
        None => below.position.x,
    };
    w.popup_ex()
        .rect(Rect2i::new(Vector2i::new(x, below.position.y), Vector2i::ZERO))
        .done();
}
pub struct Pickers {
    material_popup: Option<Gd<PopupPanel>>,
    shelves: Option<Gd<TabBar>>,
    items: Option<Gd<ItemList>>,
    empty: Option<Gd<Label>>,
    grid: Option<Gd<Button>>,
    colour_popup: Option<Gd<PopupPanel>>,
    honeycomb: Option<Gd<TextureRect>>,
    custom: Option<Gd<ColorPickerButton>>,
    comb: Honeycomb,
    marked: Option<usize>,
    align_popup: Option<Gd<PopupPanel>>,
    align_modes: Vec<(AlignMode, Gd<BaseButton>)>,
    align_axes: Vec<Gd<BaseButton>>,
    align_relative: Option<Gd<OptionButton>>,
    align_local: Option<Gd<Button>>,
}
fn connect<T: Inherits<Object>>(node: &Option<Gd<T>>, signal: &str, c: &Callable, deferred: bool) {
    if let Some(n) = node {
        let mut o = n.clone().upcast::<Object>();
        if deferred {
            o.connect_flags(signal, c, ConnectFlags::DEFERRED);
        } else {
            o.connect(signal, c);
        }
    }
}
impl Pickers {
    pub fn assemble(r: &PixelRobloxRibbon, addressee: &Gd<Object>) -> Pickers {
        let call = |m: &str| Callable::from_object_method(addressee, m);
        connect(&r.material_search, "text_changed", &call("on_material_search"), true);
        let grid = r.material_grid.clone();
        connect(&grid, "pressed", &call("on_material_view").bind(&[true.to_variant()]), true);
        connect(&r.material_list, "pressed", &call("on_material_view").bind(&[false.to_variant()]), true);
        let shelves = r.material_shelves.clone();
        connect(&shelves, "tab_changed", &call("on_material_shelf"), true);
        let items = r.material_items.clone();
        connect(&items, "item_clicked", &call("on_material_item"), true);
        let honeycomb = r.honeycomb.clone();
        connect(&honeycomb, "gui_input", &call("on_palette_input"), true);
        let custom = r.custom_colour.clone();
        connect(&custom, "color_changed", &call("on_color"), true);
        connect(&r.align_apply, "pressed", &call("on_align_apply"), true);
        let align_modes = [
            (&r.align_min, AlignMode::Min),
            (&r.align_center, AlignMode::Center),
            (&r.align_max, AlignMode::Max),
        ]
        .into_iter()
        .filter_map(|(b, m)| b.clone().map(|b| (m, b.upcast())))
        .collect();
        let align_axes = [&r.align_x, &r.align_y, &r.align_z]
            .into_iter()
            .filter_map(|b| b.clone().map(|b| b.upcast()))
            .collect();
        let scale = EditorInterface::singleton().get_editor_scale();
        if let Some(mut it) = items.clone() {
            let min = it.get_custom_minimum_size();
            it.set_custom_minimum_size(min * scale);
        }
        let comb = Honeycomb::new(HEX_RADIUS * scale);
        let mut p = Pickers {
            material_popup: r.material_popup.clone(),
            shelves,
            items,
            empty: r.material_empty.clone(),
            grid,
            colour_popup: r.color_popup.clone(),
            honeycomb,
            custom,
            comb,
            marked: None,
            align_popup: r.align_popup.clone(),
            align_modes,
            align_axes,
            align_relative: r.align_relative.clone(),
            align_local: r.align_local.clone(),
        };
        p.paint_honeycomb();
        p
    }
    fn paint_honeycomb(&mut self) {
        let Some(t) = &mut self.honeycomb else {
            return;
        };
        let px = self.comb.rasterize(self.marked);
        let Some(img) = Image::create_from_data(
            self.comb.width as i32,
            self.comb.height as i32,
            false,
            Format::RGBA8,
            &PackedByteArray::from(px.as_slice()),
        ) else {
            return;
        };
        if let Some(tex) = ImageTexture::create_from_image(&img) {
            t.set_texture(&tex);
        }
    }
    fn mark_colour(&mut self, c: Color) {
        let rgb = [c.r, c.g, c.b];
        let i = self.comb.nearest_colour(rgb);
        let w = self.comb.swatches[i].rgb;
        let d: f32 = (0..3).map(|k| (w[k] - rgb[k]) * (w[k] - rgb[k])).sum();
        let marked = (d < MARK_DISTANCE).then_some(i);
        if marked != self.marked {
            self.marked = marked;
            self.paint_honeycomb();
        }
    }
    pub fn palette_pick(&mut self, at: Vector2) -> Option<Color> {
        let i = self.comb.swatch_at(at.x, at.y)?;
        let [r, g, b] = self.comb.swatches[i].rgb;
        let c = Color::from_rgb(r, g, b);
        if let Some(cp) = &mut self.custom {
            cp.set_pick_color(c);
        }
        self.marked = None;
        self.mark_colour(c);
        Some(c)
    }
    pub fn show_colour(&mut self, current: Color) {
        if let Some(cp) = &mut self.custom {
            cp.set_pick_color(current);
        }
        self.mark_colour(current);
    }
    pub fn open_colour(&mut self, current: Color, anchor: Option<Gd<Button>>) {
        self.show_colour(current);
        if let Some(p) = self.colour_popup.clone() {
            popup_near(p.upcast(), anchor);
        }
    }
    pub fn open_material(&mut self, anchor: Option<Gd<Button>>) {
        if let Some(p) = self.material_popup.clone() {
            popup_near(p.upcast(), anchor);
        }
    }
    pub fn open_align(&mut self, anchor: Option<Gd<Button>>) {
        if let Some(p) = self.align_popup.clone() {
            popup_near(p.upcast(), anchor);
        }
    }
    pub fn set_grid(&mut self, grid: bool) {
        if let Some(it) = &mut self.items {
            it.set_max_columns(if grid { 0 } else { 1 });
            it.set_icon_mode(if grid { IconMode::TOP } else { IconMode::LEFT });
            it.set_fixed_column_width(if grid {
                (GRID_COLUMN_WIDTH as f32 * EditorInterface::singleton().get_editor_scale()) as i32
            } else {
                0
            });
            it.set_max_text_lines(if grid { 2 } else { 1 });
        }
    }
    pub fn is_grid(&self) -> bool {
        self.grid.as_ref().is_some_and(|g| g.is_pressed())
    }
    pub fn show_shelf(&mut self, tab: i64) {
        if let Some(t) = &mut self.shelves
            && t.get_current_tab() as i64 != tab
        {
            t.set_current_tab(tab as i32);
        }
    }
    pub fn show_materials(&mut self, rows: &[(String, Option<Gd<Texture2D>>)]) {
        let grid = self.is_grid();
        self.set_grid(grid);
        if let Some(it) = &mut self.items {
            it.clear();
            for (name, icon) in rows {
                match icon {
                    Some(t) => it.add_item_ex(name).icon(t).done(),
                    None => it.add_item(name),
                };
            }
        }
        if let Some(l) = &mut self.empty {
            l.set_visible(rows.is_empty());
        }
    }
    pub fn align_settings(&self) -> (AlignMode, [bool; 3], bool, bool) {
        let mode = self
            .align_modes
            .iter()
            .find(|(_, b)| b.is_pressed())
            .map(|(m, _)| *m)
            .unwrap_or(AlignMode::Center);
        let mut axes = [false; 3];
        for (k, b) in self.align_axes.iter().enumerate().take(3) {
            axes[k] = b.is_pressed();
        }
        let active = self
            .align_relative
            .as_ref()
            .is_some_and(|o| o.get_selected_id() == 1);
        let local = self.align_local.as_ref().is_some_and(|b| b.is_pressed());
        (mode, axes, active, local)
    }
}
