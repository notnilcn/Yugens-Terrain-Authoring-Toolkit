//! Reports the node references an editor panel scene's root still needs assigned, so every
//! authored editor panel tells its scene author the same thing in the same words.
use godot::classes::ClassDb;
use godot::register::info::PropertyHint;
use godot::prelude::*;

pub fn unassigned_warnings(obj: &Gd<Object>) -> PackedStringArray {
    let node_type = PropertyHint::NODE_TYPE.ord() as i64;
    ClassDb::singleton()
        .class_get_property_list_ex(&obj.get_class().to_string())
        .no_inheritance(true)
        .done()
        .iter_shared()
        .filter(|d| d.get("hint").and_then(|h| h.try_to::<i64>().ok()) == Some(node_type))
        .filter_map(|d| d.get("name").map(|n| n.to_string()))
        .filter(|name| obj.get(name.as_str()).is_nil())
        .map(|name| GString::from(&format!("Assign the {} node.", name.replace('_', " "))))
        .collect()
}

#[cfg(test)]
pub fn unresolved_root_refs(scene: &str) -> Vec<String> {
    let root_start = scene.find("[node name=").expect("scene has a root node");
    let root_end = scene[root_start + 1..]
        .find("\n[")
        .map(|e| root_start + 1 + e)
        .unwrap_or(scene.len());
    let declared: Vec<String> = scene
        .lines()
        .filter(|l| l.starts_with("[node name=") && l.contains("parent=\""))
        .map(|l| {
            let name = l.split('"').nth(1).unwrap_or_default();
            let parent = l.split("parent=\"").nth(1).and_then(|p| p.split('"').next()).unwrap_or_default();
            if parent == "." {
                name.to_string()
            } else {
                format!("{parent}/{name}")
            }
        })
        .collect();
    let refs: Vec<&str> = scene[root_start..root_end]
        .lines()
        .filter_map(|l| l.split_once(" = NodePath(\"").map(|(_, p)| p.trim_end_matches("\")")))
        .collect();
    assert!(!refs.is_empty(), "the scene root assigns no node references");
    refs.into_iter()
        .filter(|p| !declared.iter().any(|d| d == p))
        .map(str::to_string)
        .collect()
}
