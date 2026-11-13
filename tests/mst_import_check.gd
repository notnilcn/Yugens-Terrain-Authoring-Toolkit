# Imports the original MarchingSquaresTerrain demo (GDScript addon, its MSTChunkData files are
# read directly from the original project) and renders it with the camera of tests/mst_view.gd,
# so tests/captures/<name>.png can be compared with the original's shot (r5_original.png).
#   godot --path . --resolution 960x540 --script res://tests/mst_import_check.gd -- \
#         [src=/abs/original/demo/demo.tscn] [name=r5_import] [view=iso|top] [pixel=1] [save=res://x.res]
# Exit code 0 = the import produced the expected chunks.
extends SceneTree

const View := preload("res://tests/mst_view.gd")
const DEFAULT_SRC := "/home/c/g/code_examples/godot-pixelizer-3d/demo/demo.tscn"

var frames := 0
var args := {}
var terrain
var cam: Camera3D
var ok := true

func check(cond: bool, what: String) -> void:
	if not cond:
		ok = false
		printerr("MST_IMPORT FAIL: ", what)

func _setup() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	View.add_neutral_lighting(root)
	terrain = ClassDB.instantiate("MarchingSquaresTerrain")
	root.add_child(terrain)
	var src: String = args.get("src", DEFAULT_SRC)
	if not FileAccess.file_exists(src) and not DirAccess.dir_exists_absolute(src):
		print("MST_IMPORT SKIP: ", src, " not found")
		quit(0)
		return
	check(terrain.import_mst(src), "import_mst returned false")
	var chunks: Array = terrain.get_chunks()
	print("MST_IMPORT grid=", terrain.grid_type, " chunk_size=", terrain.chunk_size, " cell_size=", terrain.cell_size, " chunks=", chunks)
	if src == DEFAULT_SRC:
		# The original demo: 4 hex chunks of 33x33 cells, 2 m cells.
		check(terrain.grid_type == 2, "grid type hex")
		check(terrain.chunk_size == 33, "chunk size 33")
		check(chunks.size() == 4, "4 chunks")
		for c in [Vector2i(-1, -1), Vector2i(0, -1), Vector2i(-1, 0), Vector2i(0, 0)]:
			check(terrain.has_chunk(c), "chunk %s" % c)
		var hmin := 1e9
		var hmax := -1e9
		for c in chunks:
			for h in terrain.get_chunk_heights(c):
				hmin = minf(hmin, h)
				hmax = maxf(hmax, h)
		print("MST_IMPORT heights ", hmin, " .. ", hmax)
		check(hmax > hmin, "terrain is not flat")
		# Texture look: the original's noise ground textures, leaf sprites and albedos.
		var ts = terrain.texture_set
		check(ts != null and ts.preset_name == "Imported MST", "imported texture set")
		if ts:
			for i in 6:
				var s = ts.slots[i]
				check(s.texture != null, "slot %d texture" % i)
				check(s.grass_sprite != null, "slot %d grass sprite" % i)
				check(s.has_grass, "slot %d has grass" % i)
			check(ts.slots[0].tint.is_equal_approx(Color("647851")), "slot 0 albedo")
			check(ts.slots[5].tint.is_equal_approx(Color("71725d")), "slot 5 albedo")
			print("MST_IMPORT slots ", ts.slots.slice(0, 6).map(func(s): return [s.display_name, s.texture.resource_path if s.texture else "", s.grass_sprite.resource_path if s.grass_sprite else "", s.tint.to_html(false)]))
	if args.has("glb"):
		# Geometry baker: textured GLB, re-imported and shown instead of the terrain.
		var glb := ProjectSettings.globalize_path("res://tests/captures/%s.glb" % args.get("name", "r5_import"))
		check(terrain.export_glb_baked(glb, int(args.get("res", "16"))) == OK, "export_glb_baked")
		var doc := GLTFDocument.new()
		var state := GLTFState.new()
		check(doc.append_from_file(glb, state) == OK, "re-import " + glb)
		var scene := doc.generate_scene(state)
		check(scene != null and scene.get_child_count() == chunks.size(), "one mesh per chunk in the GLB")
		terrain.visible = false
		root.add_child(scene)
	if args.has("save"):
		check(ResourceSaver.save(terrain.data, args["save"]) == OK, "save " + args["save"])
	cam = View.make_camera(args.get("view", "iso"), float(args.get("size", "150")))
	root.add_child(cam)
	cam.look_at(View.CENTER)
	cam.current = true
	if args.has("pixel"):
		var r = ClassDB.instantiate("PixelRenderer")
		r.pixel_size = int(args.get("px", "2"))
		root.add_child(r)

func _process(_d: float) -> bool:
	frames += 1
	if frames == 1:
		_setup()
	if frames == 30:
		var name: String = args.get("name", "r5_import")
		root.get_texture().get_image().save_png("res://tests/captures/%s.png" % name)
		print("MST_IMPORT ", "OK" if ok else "FAILED", " -> tests/captures/", name, ".png")
		quit(0 if ok else 1)
	return false
