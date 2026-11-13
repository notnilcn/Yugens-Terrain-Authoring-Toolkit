# Regression check (windowed or headless): changing a PixelObject at runtime and spawning
# children keeps every mesh pixelized. Exit code 0 = pass.
#   godot --headless --path . --script res://tests/pixel_object_check.gd
extends SceneTree

var f := 0
var obj
var spawned: MeshInstance3D
var deep: MeshInstance3D
var label: Label3D
var fail := false

func _check(cond: bool, what: String) -> void:
	print(("PASS " if cond else "FAIL ") + what)
	if not cond:
		fail = true

func _process(_d: float) -> bool:
	f += 1
	if f == 1:
		obj = ClassDB.instantiate("PixelObject")
		root.add_child(obj)
		var m := MeshInstance3D.new()
		m.mesh = BoxMesh.new()
		m.mesh.material = StandardMaterial3D.new()
		m.name = "First"
		obj.add_child(m)
	elif f == 3:
		var m: MeshInstance3D = obj.get_node("First")
		_check(m.layers & (1 << 19) != 0, "initial mesh on meta layer")
		obj.pixel_size = 3
		obj.outline_id = 2
		_check(m.layers & (1 << 19) != 0, "still on meta layer after runtime change")
		_check(int(m.get_instance_shader_parameter("pt_flags")) & 3 == 2, "pixel size flag updated")
		spawned = MeshInstance3D.new()
		spawned.mesh = SphereMesh.new()
		obj.add_child(spawned)
		var holder := Node3D.new()
		obj.add_child(holder)
		deep = MeshInstance3D.new()
		deep.mesh = BoxMesh.new()
		holder.add_child(deep)
		label = Label3D.new()
		label.text = "hi"
		obj.add_child(label)
	elif f == 6:
		var m: MeshInstance3D = obj.get_node("First")
		_check(m.layers & (1 << 19) != 0, "existing mesh kept after spawn")
		_check(spawned.layers & (1 << 19) != 0, "spawned child pixelized")
		_check(deep.layers & (1 << 19) != 0, "deep descendant pixelized")
		_check(label.layers & (1 << 19) == 0, "Label3D kept off the meta layer")
		quit(1 if fail else 0)
	return false
