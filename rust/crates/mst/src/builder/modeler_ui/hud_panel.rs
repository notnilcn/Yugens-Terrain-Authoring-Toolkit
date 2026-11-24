//! The heads-up card that floats over the 3D viewport while a modelling tool is active.
//!
//! Intent: while drawing a shape, cutting or painting, the person modelling should always
//! see which tool they are in, what to do next and how to finish or back out, without
//! looking away from the viewport. The card also opens the tool's own settings in a
//! small popup above it, and doubles as the place a command reports its result. A result
//! describes the scene as it was when the command ran, so the card retires it once the
//! edit history moves on or the person presses Escape.
//!
//! Responsibility: it owns the arrangement of the card and its settings popup: the
//! heading, the guidance text, the finish, cancel, snap and settings buttons, the shape
//! choice and the tool's commands. It decides nothing about tools; the builder fills in
//! the text, decides which buttons show and answers every press.
//!
//! Why a node: the card is drawn in the editor's viewport and must sit in the tree there,
//! receiving clicks and laying itself out against the viewport's edges.
//!
//! Why a new type: a built-in panel declared in a scene can hold the parts, but the
//! builder needs to reach each of them by role. Typed references assigned in the scene
//! let the card be rearranged freely while a missing part is reported to whoever edits
//! the scene, instead of the code hunting for parts by name.
use super::assigned::unassigned_warnings;
use godot::classes::editor_undo_redo_manager::SpecialHistory;
use godot::classes::object::ConnectFlags;
use godot::classes::{
    Button, EditorInterface, HBoxContainer, IPanelContainer, InputEvent, InputEventKey, Label,
    OptionButton, PanelContainer, PopupPanel, VBoxContainer,
};
use godot::global::Key;
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = PanelContainer)]
pub struct PixelModelerHud {
    #[export]
    pub column: Option<Gd<VBoxContainer>>,
    #[export]
    pub heading: Option<Gd<Label>>,
    #[export]
    pub detail: Option<Gd<Label>>,
    #[export]
    pub snap: Option<Gd<Button>>,
    #[export]
    pub options: Option<Gd<Button>>,
    #[export]
    pub cancel: Option<Gd<Button>>,
    #[export]
    pub done: Option<Gd<Button>>,
    #[export]
    pub tool_options: Option<Gd<PopupPanel>>,
    #[export]
    pub shape_row: Option<Gd<HBoxContainer>>,
    #[export]
    pub shape_caption: Option<Gd<Label>>,
    #[export]
    pub shape: Option<Gd<OptionButton>>,
    #[export]
    pub tool_form_holder: Option<Gd<VBoxContainer>>,
    #[export]
    pub remove_point: Option<Gd<Button>>,
    result: Option<(String, HistoryStamp)>,
    retired: Option<(String, HistoryStamp)>,
    showing_result: bool,
    base: Base<PanelContainer>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HistoryStamp {
    global: u64,
    scene: u64,
}
impl HistoryStamp {
    fn now() -> HistoryStamp {
        let ei = EditorInterface::singleton();
        let Some(ur) = ei.get_editor_undo_redo() else {
            return HistoryStamp { global: 0, scene: 0 };
        };
        let version = |id: i32| {
            ur.get_history_undo_redo(id)
                .map_or(0, |h| h.get_version())
        };
        let scene = ei
            .get_edited_scene_root()
            .map_or(0, |root| version(ur.get_object_history_id(&root)));
        HistoryStamp {
            global: version(SpecialHistory::GLOBAL_HISTORY.ord()),
            scene,
        }
    }
}
impl HistoryStamp {
    fn ahead_of(&self, earlier: &HistoryStamp) -> bool {
        self.global > earlier.global || self.scene > earlier.scene
    }
}
type Shown = Option<(String, HistoryStamp)>;
pub fn admit(result: &Shown, retired: &Shown, detail: &str, now: HistoryStamp) -> (bool, Shown, Shown) {
    let fresh = (true, Some((detail.to_string(), now)), None);
    match retired {
        Some((text, at)) if text == detail && !now.ahead_of(at) => (false, None, retired.clone()),
        Some((text, _)) if text == detail => fresh,
        _ => match result {
            Some((text, at)) if text == detail && *at != now && !now.ahead_of(at) => {
                (false, None, Some((detail.to_string(), now)))
            }
            Some((text, _)) if text == detail => (true, result.clone(), None),
            _ => fresh,
        },
    }
}
pub fn retire_after_move(result: &Shown, now: HistoryStamp) -> Option<Shown> {
    match result {
        Some((text, at)) if *at != now && !now.ahead_of(at) => Some(Some((text.clone(), now))),
        _ => None,
    }
}
impl PixelModelerHud {
    pub fn admit_result(&mut self, detail: &str) -> bool {
        let (shown, result, retired) = admit(&self.result, &self.retired, detail, HistoryStamp::now());
        self.result = result;
        self.retired = retired;
        self.showing_result = shown;
        shown
    }
    pub fn showing_tool(&mut self) {
        self.showing_result = false;
    }
    fn dismiss_result(&mut self) {
        self.result = None;
        self.showing_result = false;
        if let Some(mut done) = self.done.clone() {
            done.call_deferred("emit_signal", &["pressed".to_variant()]);
        }
    }
}
#[godot_api]
impl PixelModelerHud {
    #[func]
    fn on_history_moved(&mut self) {
        let Some(retired) = retire_after_move(&self.result, HistoryStamp::now()) else {
            return;
        };
        if self.showing_result && self.base().is_visible_in_tree() {
            self.dismiss_result();
        } else {
            self.result = None;
            self.retired = retired;
        }
    }
}
#[godot_api]
impl IPanelContainer for PixelModelerHud {
    fn ready(&mut self) {
        if !godot::classes::Engine::singleton().is_editor_hint() {
            return;
        }
        let cb = self.to_gd().callable("on_history_moved");
        if let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo()
            && !ur.is_connected("version_changed", &cb)
        {
            ur.connect_flags("version_changed", &cb, ConnectFlags::DEFERRED);
        }
    }
    fn exit_tree(&mut self) {
        let cb = self.to_gd().callable("on_history_moved");
        if let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo()
            && ur.is_connected("version_changed", &cb)
        {
            ur.disconnect("version_changed", &cb);
        }
    }
    fn unhandled_key_input(&mut self, event: Gd<InputEvent>) {
        let Ok(key) = event.try_cast::<InputEventKey>() else {
            return;
        };
        if self.showing_result
            && self.base().is_visible_in_tree()
            && key.is_pressed()
            && !key.is_echo()
            && key.get_keycode() == Key::ESCAPE
        {
            self.dismiss_result();
            if let Some(mut vp) = self.base().get_viewport() {
                vp.set_input_as_handled();
            }
        }
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        unassigned_warnings(&[
            (
                self.column.is_none(),
                "Assign the column that holds the card's rows.",
            ),
            (self.heading.is_none(), "Assign the heading label."),
            (self.detail.is_none(), "Assign the guidance label."),
            (self.snap.is_none(), "Assign the snap toggle."),
            (self.options.is_none(), "Assign the tool settings button."),
            (self.cancel.is_none(), "Assign the cancel button."),
            (self.done.is_none(), "Assign the finish button."),
            (
                self.tool_options.is_none(),
                "Assign the tool settings popup.",
            ),
            (
                self.shape_row.is_none(),
                "Assign the row that holds the shape choice.",
            ),
            (
                self.shape_caption.is_none(),
                "Assign the shape choice's caption.",
            ),
            (self.shape.is_none(), "Assign the shape choice."),
            (
                self.tool_form_holder.is_none(),
                "Assign the container for the tool's settings.",
            ),
            (
                self.remove_point.is_none(),
                "Assign the remove point button.",
            ),
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const V5: HistoryStamp = HistoryStamp { global: 0, scene: 5 };
    const V6: HistoryStamp = HistoryStamp { global: 0, scene: 6 };
    const TEXT: &str = "Converted Knight_Body";
    #[test]
    fn a_new_result_is_shown_and_stays_while_the_history_stands_still() {
        let (shown, result, retired) = admit(&None, &None, TEXT, V6);
        assert!(shown);
        assert!(admit(&result, &retired, TEXT, V6).0);
    }
    #[test]
    fn undoing_the_command_retires_its_result_until_the_history_moves_forward() {
        let (_, result, _) = admit(&None, &None, TEXT, V6);
        let retired = retire_after_move(&result, V5).expect("an undo retires the result");
        assert!(!admit(&None, &retired, TEXT, V5).0, "selecting another mesh must not revive it");
        assert!(admit(&None, &retired, TEXT, V6).0, "redo or running the command again shows it");
    }
    #[test]
    fn moving_forward_does_not_retire_a_result() {
        let (_, result, _) = admit(&None, &None, TEXT, V5);
        assert!(retire_after_move(&result, V6).is_none());
    }
    #[test]
    fn a_stale_result_seen_after_an_undo_is_retired_on_sight() {
        let (_, result, _) = admit(&None, &None, TEXT, V6);
        let (shown, _, retired) = admit(&result, &None, TEXT, V5);
        assert!(!shown);
        assert!(retired.is_some());
    }
    #[test]
    fn a_different_result_replaces_a_retired_one() {
        let retired = Some((TEXT.to_string(), V5));
        let (shown, result, retired) = admit(&None, &retired, "Painted Ruin", V5);
        assert!(shown);
        assert_eq!(result, Some(("Painted Ruin".to_string(), V5)));
        assert_eq!(retired, None);
    }
}
