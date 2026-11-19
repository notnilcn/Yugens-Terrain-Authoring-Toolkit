# Unbounded pixel sizes (headless): every pixel-size property keeps values far above the old
# 4 / 16 caps, the renderer sizes its passes for them (low-res image, source border, block
# sizes, id pass), PixelObjects pack the full size into pt_flags, and the PixelDiagnostics tuner
# spin boxes accept larger values and write them through. Dragging one object's size through
# 5..40 keeps the block sizes other objects still use, and freed objects drop theirs.
# Exit code 0 = pass.
#   godot --headless --path . --script res://tests/pixel_size_check.gd
extends SceneTree

var f := 0
var fail := false
var renderer: Node
var obj: Node
var mesh: MeshInstance3D
var diag: Node
var cam: Camera3D
var other: Node
var drag := 5

func _check(cond: bool, what: String) -> void:
	print(("PASS " if cond else "FAIL ") + what)
	if not cond:
		fail = true

func _resolve_mat() -> ShaderMaterial:
	var rect: Node = renderer.get_node_or_null("PixelPipeline/ResolveViewport/Resolve")
	return rect.material if rect else null

func _flags_size(flags: int) -> int:
	var hi := (flags >> 19) & 4095
	return hi + 1 if hi != 0 else (flags & 3) + 1

# The first tuner SpinBox that follows a label `text` (the renderer section comes first).
func _tuner_spin(text: String) -> SpinBox:
	for l in diag.find_children("*", "Label", true, false):
		if (l as Label).text == text:
			var p: Node = l.get_parent()
			var i: int = l.get_index() + 1
			return p.get_child(i) as SpinBox if i < p.get_child_count() else null
	return null

func _process(_d: float) -> bool:
	f += 1
	if f == 1:
		cam = Camera3D.new()
		cam.position = Vector3(0, 2, 6)
		root.add_child(cam)
		cam.make_current()
		renderer = ClassDB.instantiate("PixelRenderer")
		root.add_child(renderer)
		obj = ClassDB.instantiate("PixelObject")
		root.add_child(obj)
		mesh = MeshInstance3D.new()
		mesh.mesh = BoxMesh.new()
		obj.add_child(mesh)
		diag = ClassDB.instantiate("PixelDiagnostics")
		root.add_child(diag)
	elif f == 3:
		renderer.pixel_size = 24
		renderer.object_pixel_size = 12
	elif f == 6:
		var screen: Vector2 = root.get_visible_rect().size
		_check(renderer.pixel_size == 24, "renderer pixel_size keeps 24")
		_check(renderer.object_pixel_size == 12, "renderer object_pixel_size keeps 12")
		var want_h := int(ceil(screen.y / 24.0))
		want_h += want_h & 1
		var lr: Vector2i = renderer.get_low_res_size()
		_check(lr.y == want_h + 2, "low-res height follows pixel size 24 (%d, want %d)" % [lr.y, want_h + 2])
		var flags := int(mesh.get_instance_shader_parameter("pt_flags"))
		_check(_flags_size(flags) == 12, "inherited object pixel size 12 packed in pt_flags (%d)" % _flags_size(flags))
		_check(flags & 3 == 3, "meta pass carries min(size, 4)")
		var m := _resolve_mat()
		_check(m != null, "resolve material")
		if m:
			_check(bool(m.get_shader_parameter("use_id_tex")), "id pass on for sizes above 4")
			var sizes: PackedInt32Array = m.get_shader_parameter("block_sizes")
			_check(12 in sizes, "resolve tests 12-texel blocks (%s)" % [sizes])
			var margin: Vector2i = m.get_shader_parameter("margin")
			_check(margin.x >= 12 + 2, "source border covers 12-texel anchors (%s)" % [margin])
			var vp: SubViewport = renderer.get_node("PixelPipeline/ResolveViewport/MetaViewport")
			_check(vp.size == lr + 2 * margin, "meta pass = low-res + border (%s)" % [vp.size])
		# Explicit sizes far above the old caps, on every pixel-size property.
		obj.pixel_size = 33
		var profile: Resource = ClassDB.instantiate("PixelObjectProfile")
		profile.pixel_size = 40
		_check(profile.pixel_size == 40, "PixelObjectProfile pixel_size keeps 40")
		var terrain: Node = ClassDB.instantiate("MarchingSquaresTerrain")
		terrain.pixel_size = 20
		terrain.grass_pixel_size = 18
		_check(terrain.pixel_size == 20, "terrain pixel_size keeps 20")
		_check(terrain.grass_pixel_size == 18, "terrain grass_pixel_size keeps 18")
		terrain.free()
		var item: Resource = ClassDB.instantiate("PixelScatterItem")
		item.pixel_size = 50
		_check(item.pixel_size == 50, "PixelScatterItem pixel_size keeps 50")
		renderer.pixel_size = 100000
	elif f == 9:
		_check(obj.pixel_size == 33, "PixelObject pixel_size keeps 33")
		_check(_flags_size(int(mesh.get_instance_shader_parameter("pt_flags"))) == 33, "explicit size 33 packed in pt_flags")
		var lr: Vector2i = renderer.get_low_res_size()
		_check(renderer.pixel_size == 100000 and lr.y == 4, "pixel size above the screen height still renders (low-res %s)" % [lr])
		var m := _resolve_mat()
		if m:
			_check(33 in (m.get_shader_parameter("block_sizes") as PackedInt32Array), "resolve tests 33-texel blocks")
		renderer.pixel_size = 4
		other = ClassDB.instantiate("PixelObject")
		other.pixel_size = 8
		var om := MeshInstance3D.new()
		om.mesh = BoxMesh.new()
		other.add_child(om)
		root.add_child(other)
	elif f >= 11 and drag <= 40:
		# One size per frame, like an inspector drag.
		obj.pixel_size = drag
		drag += 1
	elif f == 50:
		var sizes: PackedInt32Array = _resolve_mat().get_shader_parameter("block_sizes")
		_check(8 in sizes and 40 in sizes, "after dragging 5..40 the other object's 8 is still tested (%s)" % [sizes])
		_check(not (39 in sizes) and not (5 in sizes), "intermediate drag sizes are not kept (%s)" % [sizes])
		other.free()
	elif f == 52:
		var sizes: PackedInt32Array = _resolve_mat().get_shader_parameter("block_sizes")
		_check(not (8 in sizes), "a freed object's size is dropped (%s)" % [sizes])
		diag.toggle()
	elif f == 55:
		for t in ["Pixel Size", "Object Pixel Size"]:
			var s := _tuner_spin(t)
			_check(s != null, "tuner has %s" % t)
			if s:
				_check(s.allow_greater, "tuner %s allows values above %d" % [t, s.max_value])
		var s := _tuner_spin("Pixel Size")
		if s:
			s.value = 37
		var o := _tuner_spin("Object Pixel Size")
		if o:
			o.value = 29
	elif f == 57:
		_check(renderer.pixel_size == 37, "tuner wrote renderer pixel size 37 (%d)" % renderer.pixel_size)
		_check(renderer.object_pixel_size == 29, "tuner wrote object pixel size 29 (%d)" % renderer.object_pixel_size)
		quit(1 if fail else 0)
	return false
