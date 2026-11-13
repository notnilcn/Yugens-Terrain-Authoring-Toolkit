//! Terrain plugin UI for the overlays: the always-available grid toggle in the left toolbar
//! and the "Auto Texture" rules section at the bottom of the Vertex Painting texture panel
//! (edits the preset's `MarchingSquaresAutoRules`, every change undoable).

use godot::classes::{Button, CenterContainer, CheckBox, HSeparator, SpinBox, VBoxContainer};
use godot::prelude::*;

use super::plugin::MarchingSquaresTerrainPlugin;
use super::ui_kit::{self, sz};
use crate::terrain::textures::MarchingSquaresAutoRules;

pub(super) const GRID_BUTTON: &str = "GridOverlay";

#[derive(Clone, Copy)]
enum RKind {
    Bool,
    /// Slot index or None (-1).
    Slot,
    /// "Unpainted" slot: writes `base_slots = 1 << slot`.
    BaseSlot,
    Float(f64, f64, f64),
}

const RULES: [(&str, &str, RKind, &str); 18] = [
    ("Auto Texture", "enabled", RKind::Bool, "Texture unpainted ground by rules (manual paint wins)."),
    ("Unpainted", "base_slots", RKind::BaseSlot, "The slot the rules replace (the default floor)."),
    ("Noise", "noise_scale", RKind::Float(0.01, 4.0, 0.01), "Frequency of the jitter roughening every threshold."),
    ("Filler", "filler_slot", RKind::Slot, "Variant of the base slot mixed in by noise (breaks tiling)."),
    ("Filler Amount", "filler_amount", RKind::Float(0.0, 1.0, 0.01), "Share of ground covered by fillers (also rock / shore fillers)."),
    ("Rock", "rock_slot", RKind::Slot, "Slot on steep slopes."),
    ("Rock Slope", "rock_slope", RKind::Float(0.0, 90.0, 0.5), "Slopes steeper than this (degrees) become rock."),
    ("Rock Filler", "rock_filler_slot", RKind::Slot, "Variant mixed into rock."),
    ("Cliff Lip", "lip_slot", RKind::Slot, "Slot on the band along cliff tops."),
    ("Lip Width", "lip_threshold", RKind::Float(0.0, 1.0, 0.01), "Lower = wider cliff-top band."),
    ("Shore", "shore_slot", RKind::Slot, "Slot near the water line."),
    ("Water Level", "water_level", RKind::Float(-1000.0, 1000.0, 0.05), "Terrain-local water height."),
    ("Shore Height", "shore_height", RKind::Float(0.0, 20.0, 0.05), "Shore band height above the water."),
    ("Shore Filler", "shore_filler_slot", RKind::Slot, "Variant mixed into the shore (pebbles)."),
    ("Snow", "snow_slot", RKind::Slot, "Slot above the snow line."),
    ("Snow Height", "snow_height", RKind::Float(-1000.0, 1000.0, 0.1), "Terrain-local snow line."),
    ("Snow Noise", "snow_noise", RKind::Float(0.0, 50.0, 0.1), "How far the snow line wanders up and down."),
    ("Snow Slope", "snow_max_slope", RKind::Float(0.0, 90.0, 0.5), "No snow on slopes steeper than this (degrees)."),
];

impl MarchingSquaresTerrainPlugin {
    /// Grid toggle at the end of the left toolbar (outside the tool group).
    pub(super) fn add_grid_toggle(&mut self, tb: &mut ui_kit::Toolbar) {
        let mut b = Button::new_alloc();
        b.set_name(GRID_BUTTON);
        b.set_tooltip_text("Grid Overlay\nShow cell lines on the terrain while editing (show_grid_in_editor).");
        match ui_kit::icon("grid_overlay.svg") {
            Some(i) => b.set_button_icon(&i),
            None => b.set_text("#"),
        }
        b.set_flat(true);
        b.set_toggle_mode(true);
        b.set_expand_icon(true);
        b.set_focus_mode(godot::classes::control::FocusMode::NONE);
        b.set_custom_minimum_size(sz(ui_kit::TOOL_BUTTON_SIZE, ui_kit::TOOL_BUTTON_SIZE));
        b.connect("toggled", &Callable::from_object_method(&self.to_gd(), "on_grid_toggle"));
        let mut c = CenterContainer::new_alloc();
        c.set_custom_minimum_size(sz(ui_kit::ROW_HEIGHT, ui_kit::ROW_HEIGHT));
        c.add_child(&b);
        tb.root.add_child(&c);
    }

    /// Mirrors the edited terrain's grid flag into the toolbar toggle.
    pub(super) fn sync_grid_toggle(&mut self) {
        let on = self.terrain.as_ref().is_some_and(|t| t.bind().show_grid_in_editor);
        if let Some(tb) = &self.toolbar
            && let Some(b) = tb.find_child_ex(GRID_BUTTON).owned(false).done()
            && let Ok(mut b) = b.try_cast::<Button>()
        {
            b.set_pressed_no_signal(on);
        }
    }

    fn auto_rules(&self, create: bool) -> Option<Gd<MarchingSquaresAutoRules>> {
        let mut ts = self.texture_set()?;
        if let Some(r) = ts.bind().auto_rules.clone() {
            return Some(r);
        }
        if !create {
            return None;
        }
        let r = MarchingSquaresAutoRules::new_gd();
        ts.bind_mut().auto_rules = Some(r.clone());
        Some(r)
    }

    /// "Auto Texture" section of the texture panel.
    pub(super) fn build_auto_rules_section(&mut self, vbox: &mut Gd<VBoxContainer>) {
        vbox.add_child(&ui_kit::panel_title(
            "Auto Texture Rules",
            ui_kit::icon("vertex_paint_tool.svg"),
            "Biome rules of the texture preset: rock on slopes, cliff lips, sandy shores, snow peaks, fillers.\nThey replace the Unpainted slot only, so painting any other texture wins.",
        ));
        let rules = self.auto_rules(false);
        let names = self.slot_names();
        for (i, (label, prop, kind, tip)) in RULES.iter().enumerate() {
            let v = rules.as_ref().map(|r| r.get(*prop)).unwrap_or_default();
            let on = Callable::from_object_method(&self.to_gd(), "on_auto_rule").bind(&[(i as i64).to_variant()]);
            let mut c: Gd<godot::classes::Control> = match kind {
                RKind::Bool => {
                    let mut cb = CheckBox::new_alloc();
                    cb.set_flat(true);
                    cb.set_pressed_no_signal(v.try_to::<bool>().unwrap_or(false));
                    cb.connect("toggled", &on);
                    cb.upcast()
                }
                RKind::Slot | RKind::BaseSlot => {
                    let base = matches!(kind, RKind::BaseSlot);
                    let mut items: Vec<String> = if base { Vec::new() } else { vec!["None".into()] };
                    items.extend(names.iter().take(15).cloned());
                    let refs: Vec<&str> = items.iter().map(|x| x.as_str()).collect();
                    let raw = v.try_to::<i64>().unwrap_or(if base { 1 } else { -1 });
                    let sel = if base { (raw.max(1) as u32).trailing_zeros() as i32 } else { raw as i32 + 1 };
                    let mut ob = ui_kit::compact_option(&refs, sel);
                    ob.connect("item_selected", &on);
                    ob.upcast()
                }
                RKind::Float(min, max, step) => {
                    let mut sb = SpinBox::new_alloc();
                    sb.set_min(*min);
                    sb.set_max(*max);
                    sb.set_step(*step);
                    sb.set_allow_greater(true);
                    sb.set_allow_lesser(true);
                    sb.set_value_no_signal(v.try_to::<f64>().unwrap_or(0.0));
                    sb.set_custom_minimum_size(sz(60.0, 25.0));
                    sb.set_select_all_on_focus(true);
                    sb.connect("value_changed", &on);
                    sb.upcast()
                }
            };
            c.set_tooltip_text(*tip);
            c.set_name(*label);
            let mut row = ui_kit::form_row_w(label, &c, 96.0);
            row.set_tooltip_text(*tip);
            vbox.add_child(&row);
        }
        vbox.add_child(&HSeparator::new_alloc());
    }

    fn auto_rule_changed(&mut self, value: Variant, index: usize) {
        let Some((_, prop, kind, _)) = RULES.get(index).copied() else { return };
        let Some(rules) = self.auto_rules(true) else { return };
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        let obj = rules.upcast::<Object>();
        let old = obj.get(prop);
        let n = value.try_to::<f64>().unwrap_or(0.0);
        let new = match kind {
            RKind::Bool => value.try_to::<bool>().unwrap_or(false).to_variant(),
            RKind::Slot => (n.round() as i64 - 1).to_variant(),
            RKind::BaseSlot => (1i64 << (n.round() as i64).clamp(0, 15)).to_variant(),
            RKind::Float(..) => (n as f32).to_variant(),
        };
        if new == old {
            return;
        }
        ur.create_action(&format!("auto texture {prop}"));
        ur.add_do_property(&obj, prop, &new);
        ur.add_undo_property(&obj, prop, &old);
        ur.commit_action();
    }
}

#[godot_api(secondary)]
impl MarchingSquaresTerrainPlugin {
    #[func]
    fn on_grid_toggle(&mut self, pressed: bool) {
        let Some(t) = self.terrain.clone() else { return };
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        let obj = t.upcast::<Object>();
        ur.create_action("terrain grid overlay");
        ur.add_do_property(&obj, "show_grid_in_editor", &pressed.to_variant());
        ur.add_undo_property(&obj, "show_grid_in_editor", &(!pressed).to_variant());
        ur.commit_action();
    }

    #[func]
    fn on_auto_rule(&mut self, value: Variant, index: i64) {
        self.auto_rule_changed(value, index as usize);
    }
}
