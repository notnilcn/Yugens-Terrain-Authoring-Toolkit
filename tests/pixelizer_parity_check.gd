# Pixelizer parity regression check (headless): automatic outline ids, terrain outline ids
# above 7, outline multiply mode, renderer Anchor / Low Res mode and the cloud field scope.
#   godot --headless --path . --script res://tests/pixelizer_parity_check.gd
# Exit code 0 = pass.
extends SceneTree

var f := 0
var fail := false
var a
var b
var nested
var nested_auto
var renderer
var terrain

func _check(cond: bool, what: String) -> void:
	print(("PASS " if cond else "FAIL ") + what)
	if not cond:
		fail = true

func _obj(parent: Node, name: String) -> Node:
	var o = ClassDB.instantiate("PixelObject")
	o.name = name
	parent.add_child(o)
	var m := MeshInstance3D.new()
	m.mesh = BoxMesh.new()
	o.add_child(m)
	return o

func _process(_d: float) -> bool:
	f += 1
	if f == 1:
		renderer = ClassDB.instantiate("PixelRenderer")
		root.add_child(renderer)
		a = _obj(root, "A")
		b = _obj(root, "B")
		nested = _obj(a, "Nested")
		nested_auto = _obj(a, "NestedAuto")
		nested_auto.outline_id = -2
		terrain = ClassDB.instantiate("MarchingSquaresTerrain")
		root.add_child(terrain)
	elif f == 4:
		var ia: int = a.get_effective()["outline_id"]
		var ib: int = b.get_effective()["outline_id"]
		var inn: int = nested.get_effective()["outline_id"]
		var iauto: int = nested_auto.get_effective()["outline_id"]
		_check(ia >= 1 and ia <= 253, "top-level inherit gets an automatic id (%d)" % ia)
		_check(ib >= 1 and ib <= 253 and ib != ia, "second top-level object gets its own id (%d)" % ib)
		_check(inn == ia, "nested inherit shares the parent's automatic id (%d)" % inn)
		_check(iauto != ia and iauto >= 1, "outline_id -2 allocates its own id (%d)" % iauto)
		var again: int = a.get_effective()["outline_id"]
		_check(again == ia, "automatic id is stable")
		terrain.outline_id = 200
		_check(terrain.outline_id == 200, "terrain outline_id accepts 200")
		terrain.outline_id = 300
		_check(terrain.outline_id == 253, "terrain outline_id clamps to 253")
		var pal = ClassDB.instantiate("PixelOutlinePalette")
		pal.multiply_colors = true
		_check(pal.multiply_colors, "outline palette multiply mode")
		renderer.mode = 1
		_check(renderer.mode == 1, "renderer Low Res mode")
		var sky = ClassDB.instantiate("PixelSky")
		_check("cloud_camera_scope" in sky, "PixelSky has cloud_camera_scope")
		sky.free()
	elif f == 6:
		quit(1 if fail else 0)
	return false
