extends Object
class_name EngineWrapper


static var instance : EngineWrapper


static func _static_init() -> void:
	instance = EngineWrapper.new()


func is_editor() -> bool:
	return Engine.is_editor_hint()


func get_edited_scene_root():
	var editor_interface = Engine.get_singleton('EditorInterface')
	return editor_interface.get_edited_scene_root()


func get_root_for_node(node: Node) -> Node:
	if is_editor():
		return get_edited_scene_root()
	if node and node.is_inside_tree():
		return node.get_tree().root
	return null


func set_owner_recursive(node: Node, _owner: Node = null) -> void:
	if not _owner:
		_owner = get_root_for_node(node)
	node.owner = _owner
	for c in node.get_children():
		set_owner_recursive(c, _owner)
