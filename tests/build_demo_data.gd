# Generates the demo terrain data resources (headless):
#   godot --headless --path . --script res://tests/build_demo_data.gd
# Writes demo/island_data.res (square grid, terraced island with cliffs and beaches) and
# demo/hex_world_data.res (hex-rings grid, stepped hex highlands) and demo/hex_island_data.res
# (the square island resampled on a hex-rings grid). `-- only=hex_island cell=2 ring=6` limits /
# tunes the run.
extends SceneTree

const SEA := 0.0
# Hex island grid: ring-2 chunks of 2-unit cells (= kg_hex patches of small-hex-sized cells).
const HEX_CELL := 2.0
const HEX_RING := 2
const TERRACE := 2.5
# Half-terrace shoulder cells (two 1.25 ramps) where the square terraces slope; off by default:
# grassed ramps read as overgrown cliffs, while full steps match the square demo's clean walls.
var shoulders := false
# HexPrism look (the hex island's hex_edge_style): the beach and sea floor below the first
# terrace are quantised to BEACH_STEP prism steps down from it (2.5, 1.9, 1.3, 0.7 dry sand,
# 0.1 shallows, ...: no column top within 0.25 of the 0.45 water line), instead of continuous
# heights that only read as a slope with smooth (FlatWalls) cells. `-- prism=0` keeps them
# continuous. Cells stay flagged smooth either way, so FlatWalls still renders a smooth beach.
var prism := true
const BEACH_STEP := 0.6

var frames := 0


# Saves `res` at `path` with a resource UID (keeping the file's existing one). A UID-less .res
# is rewritten by the editor's filesystem scan the first time a scene using it is opened, which
# would show up as an unexpected change to the demo data.
static func save_with_uid(res: Resource, path: String) -> Error:
	var uid := ResourceLoader.get_resource_uid(path) if ResourceLoader.exists(path) else ResourceUID.INVALID_ID
	if uid == ResourceUID.INVALID_ID:
		uid = ResourceUID.create_id()
	res.take_over_path(path)
	var err := ResourceSaver.save(res, path)
	if err == OK:
		err = ResourceSaver.set_uid(path, uid)
	return err


func _process(_d: float) -> bool:
	frames += 1
	if frames == 2:
		var args := {}
		for a in OS.get_cmdline_user_args():
			var kv := a.split("=")
			args[kv[0]] = kv[1] if kv.size() > 1 else "1"
		var only: String = args.get("only", "")
		if only == "" or only == "island":
			_build_island()
		if only == "" or only == "hex_world":
			_build_hex_world()
		shoulders = args.get("shoulders", "0") == "1"
		prism = args.get("prism", "1") == "1"
		if only == "" or only == "hex_island":
			_build_hex_island(float(args.get("cell", str(HEX_CELL))), int(args.get("ring", str(HEX_RING))))
		quit()
	return false


func _noise(seed: int, freq: float) -> FastNoiseLite:
	var n := FastNoiseLite.new()
	n.seed = seed
	n.frequency = freq
	n.fractal_octaves = 4
	return n


func _island_height(p: Vector2, radius: float, n: FastNoiseLite, ridge: FastNoiseLite) -> float:
	var d := p.length() / radius
	var shape := 1.0 - smoothstep(0.35, 1.0, d + n.get_noise_2dv(p) * 0.25)
	var h := shape * 14.0 - 3.0
	h += ridge.get_noise_2dv(p) * 5.0 * shape
	if h < 1.0:
		# Gentle sea floor/beach: no tiny cliffs under the water.
		h = lerp(-2.0, 1.0, smoothstep(-3.0, 1.0, h))
		# Shelf falling away towards the data edge, so the water reaches its deep colour before
		# the (square) terrain ends instead of showing the chunk outline as a shallow band.
		h -= 4.0 * smoothstep(0.95, 1.45, d)
	if h > 1.0:
		# Terraces: flat plateaus separated by cliffs, the Minor Deity look.
		var step := 2.5
		var t := h / step
		h = (floor(t) + smoothstep(0.75, 1.0, t - floor(t))) * step
	return h


func _build_island() -> void:
	var t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 0
	t.chunk_size = 16
	t.cell_size = Vector2(2, 2)
	t.default_merge_mode = 1
	root.add_child(t)
	for z in range(-2, 2):
		for x in range(-2, 2):
			t.add_chunk(Vector2i(x, z))
	var n := _noise(7, 0.02)
	var ridge := _noise(11, 0.035)
	var paths := _noise(23, 0.05)
	var forest := _noise(31, 0.04)
	for z in range(-32, 33):
		for x in range(-32, 33):
			var s := Vector2i(x, z)
			var p := Vector2(x, z) * 2.0
			var h := _island_height(p, 58.0, n, ridge)
			t.set_sample_height(s, h)
			var floor_slot := 0
			if h < SEA + 1.2:
				floor_slot = 3 # sand
			elif h > 9.0:
				floor_slot = 1 # meadow on the highlands
			elif forest.get_noise_2dv(p) > 0.25:
				floor_slot = 2 # forest floor
			if abs(paths.get_noise_2dv(p)) < 0.035 and h > SEA + 1.5:
				floor_slot = 11 # path
			t.set_sample_slot(s, floor_slot, false)
			t.set_sample_slot(s, 6 if h < 3.0 else 5, true)
	t.update_configuration_warnings()
	t.commit_to_data()
	var err := save_with_uid(t.data, "res://demo/island_data.res")
	print("island data: ", err, " chunks=", t.get_chunks().size())
	t.queue_free()


func _build_hex_world() -> void:
	var t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 3
	t.ring_radius = 6
	t.cell_size = Vector2(2, 2)
	t.default_merge_mode = 0
	root.add_child(t)
	var coords := [Vector2i(0, 0), Vector2i(1, 0), Vector2i(0, 1), Vector2i(-1, 1), Vector2i(-1, 0), Vector2i(0, -1), Vector2i(1, -1)]
	for c in coords:
		t.add_chunk(c)
	var n := _noise(5, 0.03)
	# Walk every sample of every chunk through world positions on a fine lattice.
	for z in range(-40, 41):
		for x in range(-40, 41):
			var world := Vector3(x * 1.0, 0, z * 1.0)
			if not t.has_ground_at(world):
				continue
			var s: Vector2i = t.world_to_sample(world)
			var p := Vector2(world.x, world.z)
			var d := p.length() / 34.0
			var h := (1.0 - smoothstep(0.2, 1.0, d)) * 7.0 + n.get_noise_2dv(p) * 3.0
			h = round(h * 2.0) / 2.0 # half-unit steps: hex terraces
			t.set_sample_height(s, h)
			var slot := 0
			if h <= 0.5:
				slot = 3
			elif h >= 5.5:
				slot = 7 # snow caps
			elif h >= 4.0:
				slot = 8 # gravel
			t.set_sample_slot(s, slot, false)
			t.set_sample_slot(s, 5, true)
	t.commit_to_data()
	var err := save_with_uid(t.data, "res://demo/hex_world_data.res")
	print("hex world data: ", err, " chunks=", t.get_chunks().size())
	t.queue_free()


# The square island's height function evaluated at hex cell centres, then quantised so the
# cell mesher reproduces its look: terrace plateaus stay flat cells, terrace shoulders become
# half-terrace cells (1.25 steps < Polyhedron 1.3 -> ramps), and everything below the first
# terrace (sea floor, beach, low shoulders) becomes smooth cells (corner-averaged planes).
func _hex_quantise(h: float) -> Array:
	if h < TERRACE - 0.05:
		if prism:
			var q := TERRACE - BEACH_STEP * roundf((TERRACE - h) / BEACH_STEP)
			return [q, true] if q < TERRACE - 0.05 else [TERRACE, false]
		return [h, true]
	var t := h / TERRACE
	var k := floorf(t + 0.001)
	var f := t - k
	if f < 0.05:
		return [k * TERRACE, false]
	return [(k + (roundf(f * 2.0) * 0.5 if shoulders else roundf(f))) * TERRACE, false]


func _floor_slot(h: float, p: Vector2, paths: FastNoiseLite, forest: FastNoiseLite, path_width: float) -> int:
	var floor_slot := 0
	if h < SEA + 1.2:
		floor_slot = 3 # sand
	elif h > 9.0:
		floor_slot = 1 # meadow on the highlands
	elif forest.get_noise_2dv(p) > 0.25:
		floor_slot = 2 # forest floor
	if abs(paths.get_noise_2dv(p)) < path_width and h > SEA + 1.5:
		floor_slot = 11 # path
	return floor_slot


func _build_hex_island(cell: float, ring: int) -> void:
	var t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 3
	t.ring_radius = ring
	t.cell_size = Vector2(cell, cell)
	t.default_merge_mode = 1
	root.add_child(t)
	# Chunks whose disk reaches into the square island's 128 x 128 footprint.
	var reach := 64.0 * sqrt(2.0) + (ring + 1) * cell
	for cr in range(-20, 21):
		for cq in range(-20, 21):
			var c := Vector2i(cq, cr)
			var w: Vector3 = t.sample_to_world(t.get_chunk_center_cell(c))
			if Vector2(w.x, w.z).length() < reach and absf(w.x) < 64.0 + (ring + 1) * cell and absf(w.z) < 64.0 + (ring + 1) * cell:
				t.add_chunk(c)
	var n := _noise(7, 0.02)
	var ridge := _noise(11, 0.035)
	var paths := _noise(23, 0.05)
	var forest := _noise(31, 0.04)
	var path_width := 0.035 * maxf(1.0, cell / 2.0)
	# Pass 1: quantised height per cell (axial coords straight from the chunk cell lists).
	var hq := {}
	var pos := {}
	for c in t.get_chunks():
		for id in t.get_chunk_cells(c):
			var w: Vector3 = t.sample_to_world(id)
			var p := Vector2(w.x, w.z)
			pos[id] = p
			hq[id] = _hex_quantise(_island_height(p, 58.0, n, ridge))
	# Pass 2: drop one-cell pillars and fill one-cell pits (noise speckle reads badly as hex
	# columns; with `prism` the quantised beach too), then write heights, smooth flags and slots per chunk in bulk.
	var cells := 0
	for c in t.get_chunks():
		var ids = t.get_chunk_cells(c)
		var heights := PackedFloat32Array()
		var floors := PackedByteArray()
		var walls := PackedByteArray()
		for id in ids:
			var p: Vector2 = pos[id]
			var q: Array = hq[id]
			var h: float = q[0]
			if prism or not q[1]:
				var hs: Array[float] = []
				for nb in t.get_cell_neighbors(id):
					hs.append(hq[nb][0] if hq.has(nb) else h)
				hs.sort()
				h = clampf(h, hs[1], hs[4])
			heights.append(h)
			floors.append(_floor_slot(h, p, paths, forest, path_width))
			walls.append(6 if h < 3.0 else 5)
			if q[1]:
				t.set_sample_smooth(id, true)
		t.set_chunk_heights(c, heights)
		t.set_chunk_slots(c, floors, false)
		t.set_chunk_slots(c, walls, true)
		cells += ids.size()
	var water := _carve_hex_pool(t) if cell == HEX_CELL and ring == HEX_RING else PackedVector3Array()
	t.commit_to_data()
	# Raised water for the scene's PixelWater (build_demo_scenes.gd copies it to water_cells).
	t.data.set_meta("water_cells", water)
	var err := save_with_uid(t.data, "res://demo/hex_island_data.res")
	print("hex island data: ", err, " chunks=", t.get_chunks().size(), " cells=", cells, " water=", water.size())
	t.queue_free()


# A small pool in a hollow on the plateau (h 10) just north of the player spawn (0, 7.5, 8): its
# bed is sunk half a level (the plateau ramps down into it), its water sits a little below the
# rim, and only (0, 2) borders the spawn hollow (h 7.5), so the pool spills over one edge of the
# 2.5 m cliff as a waterfall facing the default third-person view. The 2.35 m drop is >= 4
# levels of 0.5 (the scene's PixelWater waterfall_min_levels / waterfall_level_height).
const HEX_POOL := [Vector2i(-1, 2), Vector2i(0, 2), Vector2i(0, 1)]
const POOL_LEVEL := 10.0
const POOL_WATER := 9.85


func _carve_hex_pool(t) -> PackedVector3Array:
	var water := PackedVector3Array()
	for c in HEX_POOL:
		if not is_equal_approx(t.get_cell_height(c), POOL_LEVEL):
			push_warning("hex pool cell %s is at %f, not %f" % [c, t.get_cell_height(c), POOL_LEVEL])
		t.set_cell_height(c, POOL_LEVEL - 0.5)
		t.set_sample_slot(c, 3, false) # sandy bed
		water.append(Vector3(c.x, c.y, POOL_WATER))
	return water
