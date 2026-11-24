use godot::classes::Engine;
use godot::obj::Singleton;
pub fn is_editor() -> bool {
    Engine::singleton().is_editor_hint()
}
