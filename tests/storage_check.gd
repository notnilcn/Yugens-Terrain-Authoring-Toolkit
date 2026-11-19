# Storage regression check (headless): Baked / Runtime directory storage, change tracking,
# baked-mesh loading, chunk removal, the data resource's dirty flag, noise_hmap and reading
# the original's MSTChunkData folder in place (never written).
#   godot --headless --path . --script res://tests/storage_check.gd   (exit 0 = pass)
extends SceneTree

const DIR := "res://tests/tmp/storage_check"
const ORIGINAL := "/home/c/g/code_examples/godot-pixelizer-3d/demo/demo_TerrainData/MarchingSquaresTerrain_126b8b50"

var ok := true
var step := 0
var t1
var t2
var t3

func check(cond: bool, what: String) -> void:
	print("  ", "ok  " if cond else "FAIL", " ", what)
	if not cond:
		ok = false

func rm_tree(path: String) -> void:
	if not DirAccess.dir_exists_absolute(path):
		return
	for d in DirAccess.get_directories_at(path):
		rm_tree(path.path_join(d))
	for f in DirAccess.get_files_at(path):
		DirAccess.remove_absolute(path.path_join(f))
	DirAccess.remove_absolute(path)

func make_terrain(mode: int, dir: String):
	var t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 2
	t.chunk_size = 8
	t.storage_mode = mode
	t.data_directory = dir
	root.add_child(t)
	return t

func chunk_file(c: Vector2i) -> String:
	return DIR.path_join("hex/chunk_%d_%d/metadata.res" % [c.x, c.y])

func file_size(p: String) -> int:
	var f := FileAccess.open(p, FileAccess.READ)
	return f.get_length() if f else -1

func _process(_d: float) -> bool:
	step += 1
	match step:
		1:
			rm_tree(DIR)
			print("storage_check: Baked save")
			t1 = make_terrain(1, DIR)
			var n := FastNoiseLite.new()
			n.frequency = 0.05
			t1.noise_hmap = n
			t1.noise_hmap_height = 4.0
			for c in [Vector2i(0, 0), Vector2i(1, 0), Vector2i(0, 1), Vector2i(1, 1)]:
				t1.add_chunk(c)
		2:
			var n: FastNoiseLite = t1.noise_hmap
			check(absf(t1.get_cell_height(Vector2i(3, 3)) - n.get_noise_2d(t1.cell_to_world(Vector2i(3, 3)).x, t1.cell_to_world(Vector2i(3, 3)).z) * 4.0) < 1e-3, "noise_hmap heights (cell centre x height)")
			check(t1.save_terrain_data() == OK, "save_terrain_data OK")
			check(t1.get_last_save_count() == 4, "4 chunk files written (got %d)" % t1.get_last_save_count())
			for c in [Vector2i(0, 0), Vector2i(1, 1)]:
				check(FileAccess.file_exists(chunk_file(c)), "file " + chunk_file(c))
			var res = ResourceLoader.load(chunk_file(Vector2i(0, 0)), "", ResourceLoader.CACHE_MODE_IGNORE)
			check(res != null and res.mesh != null and res.grass_multimesh != null and res.collision_faces.size() > 0, "baked file holds mesh, grass and collision")
			check(t1.save_terrain_data() == OK and t1.get_last_save_count() == 0, "unchanged terrain: nothing rewritten")
			# A cell in the middle of chunk (0,0) only: the chunk and its (baked) neighbours.
			t1.set_cell_height(Vector2i(4, 4), 9.0)
			t1.save_terrain_data()
			check(t1.get_last_save_count() == 1, "interior edit rewrote only its chunk (got %d)" % t1.get_last_save_count())
			t1.remove_chunk(Vector2i(1, 1))
			t1.save_terrain_data()
			check(not DirAccess.dir_exists_absolute(DIR.path_join("hex/chunk_1_1")), "removed chunk folder deleted")
		3:
			print("storage_check: Baked load")
			t2 = make_terrain(1, DIR)
		5:
			check(t2.get_chunks().size() == 3, "3 chunks loaded (got %d)" % t2.get_chunks().size())
			check(absf(t2.get_cell_height(Vector2i(4, 4)) - 9.0) < 1e-4, "edited height persisted")
			check(t2.get_baked_load_count() == 3, "meshes taken from the baked files (got %d)" % t2.get_baked_load_count())
			check(t2.save_terrain_data() == OK and t2.get_last_save_count() == 0, "freshly loaded: nothing rewritten")
			var baked_size := file_size(chunk_file(Vector2i(0, 0)))
			print("storage_check: Runtime")
			t2.storage_mode = 2
			t2.save_terrain_data()
			check(t2.get_last_save_count() == 3, "switching to Runtime rewrites every file (got %d)" % t2.get_last_save_count())
			var rt_size := file_size(chunk_file(Vector2i(0, 0)))
			check(rt_size > 0 and rt_size < baked_size, "runtime file smaller (%d < %d bytes)" % [rt_size, baked_size])
			var res = ResourceLoader.load(chunk_file(Vector2i(0, 0)), "", ResourceLoader.CACHE_MODE_IGNORE)
			check(res.mesh == null, "runtime file has no mesh")
			t2.set_cell_height(Vector2i(2, 2), 1.0)
			t2.save_terrain_data()
			check(t2.get_last_save_count() == 1, "runtime edit rewrites only its chunk (got %d)" % t2.get_last_save_count())
			print("storage_check: original MSTChunkData folder (read only)")
			if DirAccess.dir_exists_absolute(ORIGINAL):
				t3 = make_terrain(2, ORIGINAL)
				t3.chunk_size = 33
		7:
			if t3:
				check(t3.get_chunks().size() == 4, "original demo folder loads 4 chunks (got %d)" % t3.get_chunks().size())
				t3.queue_free()
			print("storage_check: data resource dirty flag")
			var d = ClassDB.instantiate("MarchingSquaresTerrainData")
			check(not d.is_dirty(), "new data is clean")
			var t4 = ClassDB.instantiate("MarchingSquaresTerrain")
			t4.data = d
			root.add_child(t4)
			t4.add_chunk(Vector2i(0, 0))
			check(d.is_dirty(), "add_chunk marks the data dirty")
			d.clear_dirty()
			check(not d.is_dirty(), "clear_dirty")
			t4.set_sample_height(Vector2i(1, 1), 2.0)
			check(not d.is_dirty(), "runtime edits stay in memory until commit_to_data")
			t4.commit_to_data()
			check(d.is_dirty(), "commit_to_data marks dirty")
			rm_tree(DIR)
			rm_tree("res://tests/tmp")
			print("storage_check: ", "PASS" if ok else "FAIL")
			quit(0 if ok else 1)
	return false
