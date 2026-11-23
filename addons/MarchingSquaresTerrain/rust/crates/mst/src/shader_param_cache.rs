use std::cell::RefCell;
use std::collections::HashMap;
use godot::classes::ShaderMaterial;
use godot::prelude::*;
thread_local! {
    static NAMES: RefCell<HashMap<&'static str, StringName>> = RefCell::new(HashMap::new());
}
pub fn clear_names() {
    NAMES.with_borrow_mut(|m| m.clear());
}
pub fn moniker(n: &'static str) -> StringName {
    NAMES.with_borrow_mut(|m| m.entry(n).or_insert_with(|| StringName::from(n)).clone())
}
pub trait ShaderParams {
    fn param(&mut self, name: &'static str, value: &Variant);
}
impl ShaderParams for Gd<ShaderMaterial> {
    fn param(&mut self, name: &'static str, value: &Variant) {
        let sn = self::moniker(name);
        if self.get_shader_parameter(&sn) == *value {
            return;
        }
        self.set_shader_parameter(&sn, value);
    }
}
