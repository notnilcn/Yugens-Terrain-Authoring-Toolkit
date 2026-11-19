# Generates the demo terrain data (headless):
#   godot --headless --path . --script res://tests/build_demo_data.gd [-- cell=2 ring=2 prism=1 map=1]
# Writes demo/hex_island_data.res: a hex-rings island (HexPrism look) laid out for the demo's
# start view. From the coast inwards: wide sand beaches, a lowland ring (level 1 = 2.5 m) with the
# player's meadow in the south-east, a broad central plateau (5 m), a western shelf (7.5 m), the
# highland (10 m) with a pool whose waterfall spills 5 m down its south-east cliff into a pond,
# and a peak (12.5 / 15 m) behind it. Smooth ("Sloped") ramps of path cells connect the levels.
# The committed data was also scattered by tests/scatter_demo.gd afterwards; re-run that too, or
# the island loses its props. `map=1` also writes tests/captures/island_levels.png (top view of
# the levels, for layout work).
extends SceneTree

const SEA := 0.0
# Hex island grid: ring-2 chunks of 2-unit cells (= kg_hex patches of small-hex-sized cells).
const HEX_CELL := 2.0
const HEX_RING := 2
const TERRACE := 2.5
# HexPrism look (the hex island's hex_edge_style): the beach and sea floor below the first
# terrace are quantised to BEACH_STEP prism steps down from it (2.5, 1.9, 1.3, 0.7 dry sand,
# 0.1 shallows, ...: no column top within 0.25 of the 0.45 water line), instead of continuous
# heights that only read as a slope with smooth (FlatWalls) cells. `-- prism=0` keeps them
# continuous. Cells stay flagged smooth either way, so FlatWalls still renders a smooth beach.
var prism := true
const BEACH_STEP := 0.6

# Island layout (world x / z, north = -z). Coast: signed distance from a noisy circle.
const COAST_RADIUS := 50.0
# Width of the beach band between the coast line and the lowland (level 1).
const BEACH_WIDTH := 9.0
# Plateaus: [centre, radius, level]; a cell takes the highest level whose (noise-warped) disc
# contains it.
const PLATEAUS := [
	[Vector2(-7, -7), 31.0, 2], # central plateau, 5 m
	[Vector2(-22, -15), 19.0, 3], # western shelf, 7.5 m
	[Vector2(-7, -14), 15.0, 4], # highland with the pool, 10 m
	[Vector2(-15, -24), 8.5, 5], # peak, 12.5 m
	[Vector2(-17, -27), 4.0, 6], # summit, 15 m
]
# Smooth (Sloped / Bridge-style) ramps: [from, to, from height, to height, half width]. Cells
# within the half width of the segment become slanted path planes from one height to the other.
const RAMPS := [
	[Vector2(19, 17), Vector2(10.5, 8.5), 2.5, 5.0, 2.6], # spawn meadow -> central plateau
	[Vector2(-30, 14), Vector2(-33, 4), 2.5, 5.0, 2.2], # west lowland -> central plateau
	[Vector2(-24, 6), Vector2(-27, -2.5), 5.0, 7.5, 2.2], # central plateau -> western shelf
	[Vector2(-21, -8), Vector2(-15, -11), 7.5, 10.0, 2.2], # western shelf -> highland
]
# Raised pool on the highland's south-east rim: its rim (10 m) closes it on every side except
# the one facing the pond (5 m) right below it, so the pool spills over the 5 m cliff as a wide
# waterfall into the pond, facing the start view.
const POOL_CENTRE := Vector2(2.0, -3.0)
const POOL_RADIUS := 3.3
const POOL_LEVEL := 10.0
const POOL_WATER := 9.85
# Direction from the pool to the pond, and the pond's size.
const POND_DIR := Vector2(0.55, 0.835)
const POND_RADIUS := 3.8
const POND_LEVEL := 5.0
const POND_WATER := 4.85

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
		prism = args.get("prism", "1") == "1"
		_build_hex_island(float(args.get("cell", str(HEX_CELL))), int(args.get("ring", str(HEX_RING))), args.get("map", "0") == "1")
		quit()
	return false


func _noise(seed: int, freq: float) -> FastNoiseLite:
	var n := FastNoiseLite.new()
	n.seed = seed
	n.frequency = freq
	n.fractal_octaves = 3
	return n


# Distance from p to the segment a-b and the position along it (0 at a, 1 at b).
static func _segment(p: Vector2, a: Vector2, b: Vector2) -> Vector2:
	var ab := b - a
	var t := clampf((p - a).dot(ab) / ab.length_squared(), 0.0, 1.0)
	return Vector2(p.distance_to(a + ab * t), t)


# [height, smooth] of the designed island at p (before the speckle filter).
func _island_height(p: Vector2, coast: FastNoiseLite, warp: FastNoiseLite) -> Array:
	# Signed distance inland from the coast line (> 0 on land).
	var s := COAST_RADIUS + coast.get_noise_2dv(p) * 9.0 - p.length()
	if s < BEACH_WIDTH:
		# Beach and sea floor: a smooth slope through the 0.45 water line, then a shelf falling
		# away towards the data edge so the water reaches its deep colour before the terrain ends.
		var h := maxf(0.45 + (s - 0.5) * 0.2, -2.5)
		h -= 4.0 * smoothstep(-16.0, -30.0, s)
		if prism:
			var q := TERRACE - BEACH_STEP * roundf((TERRACE - h) / BEACH_STEP)
			return [minf(q, TERRACE - BEACH_STEP), true]
		return [minf(h, TERRACE - 0.3), true]
	var level := 1
	for pl in PLATEAUS:
		var c: Vector2 = pl[0]
		var r: float = pl[1] + warp.get_noise_2dv(p + c * 3.0) * 4.5
		if p.distance_to(c) < r:
			level = maxi(level, pl[2])
	return [level * TERRACE, false]


func _floor_slot(h: float, smooth: bool, p: Vector2, paths: FastNoiseLite, forest: FastNoiseLite) -> int:
	if smooth and h < TERRACE - 0.05:
		return 3 # sand beach
	var floor_slot := 0
	if h > 9.0:
		floor_slot = 1 # meadow on the highlands
	elif forest.get_noise_2dv(p) > 0.22:
		floor_slot = 2 # forest floor
	if abs(paths.get_noise_2dv(p)) < 0.03 and h > SEA + 1.5:
		floor_slot = 11 # path
	return floor_slot


func _build_hex_island(cell: float, ring: int, map: bool) -> void:
	var t = ClassDB.instantiate("MarchingSquaresTerrain")
	t.grid_type = 3
	t.ring_radius = ring
	t.cell_size = Vector2(cell, cell)
	t.default_merge_mode = 1
	root.add_child(t)
	# Chunks whose disk reaches into the island's 128 x 128 footprint.
	var reach := 64.0 * sqrt(2.0) + (ring + 1) * cell
	for cr in range(-20, 21):
		for cq in range(-20, 21):
			var c := Vector2i(cq, cr)
			var w: Vector3 = t.sample_to_world(t.get_chunk_center_cell(c))
			if Vector2(w.x, w.z).length() < reach and absf(w.x) < 64.0 + (ring + 1) * cell and absf(w.z) < 64.0 + (ring + 1) * cell:
				t.add_chunk(c)
	var coast := _noise(7, 0.03)
	var warp := _noise(11, 0.045)
	var paths := _noise(23, 0.05)
	var forest := _noise(31, 0.045)
	# Pass 1: designed height per cell.
	var hq := {}
	var pos := {}
	for c in t.get_chunks():
		for id in t.get_chunk_cells(c):
			var w: Vector3 = t.sample_to_world(id)
			var p := Vector2(w.x, w.z)
			pos[id] = p
			hq[id] = _island_height(p, coast, warp)
	# Pass 2: speckle filter, twice. A land cell takes the median of itself and its six
	# neighbours (no one-cell mesas, pits or notches: the hex columns read as broad plateaus);
	# beach cells keep their place between their 2nd-lowest and 2nd-highest neighbour.
	for i in 2:
		var next := {}
		for id in hq:
			var q: Array = hq[id]
			var hs: Array[float] = [q[0]]
			for nb in t.get_cell_neighbors(id):
				hs.append(hq[nb][0] if hq.has(nb) else q[0])
			hs.sort()
			if q[1]:
				var nbs := hs.duplicate()
				nbs.erase(q[0])
				next[id] = [clampf(q[0], nbs[1], nbs[nbs.size() - 2]), true]
			elif hs[3] < TERRACE - 0.05:
				next[id] = q # a land cell on the coast keeps its level (beach stays below)
			else:
				next[id] = [hs[3], false]
		hq = next
	# Pass 3: ramps (smooth slanted path planes) and the pool / pond basins.
	var ramp := {}
	var water := PackedVector3Array()
	for id in hq:
		var p: Vector2 = pos[id]
		for r in RAMPS:
			var st := _segment(p, r[0], r[1])
			if st.x < r[4]:
				hq[id] = [lerpf(r[2], r[3], st.y), true]
				ramp[id] = true
		if cell == HEX_CELL and ring == HEX_RING:
			var pond := POOL_CENTRE + POND_DIR * (POOL_RADIUS + POND_RADIUS - 0.8)
			var dp := p.distance_to(POOL_CENTRE)
			var along := (p - POOL_CENTRE).dot(POND_DIR)
			if dp < POOL_RADIUS:
				hq[id] = [POOL_LEVEL - 0.5, false]
				water.append(Vector3(id.x, id.y, POOL_WATER))
			elif dp < POOL_RADIUS + 3.5 and along < 1.2:
				hq[id] = [POOL_LEVEL, false] # rim
			elif p.distance_to(pond) < POND_RADIUS:
				hq[id] = [POND_LEVEL - 0.5, false]
				water.append(Vector3(id.x, id.y, POND_WATER))
			elif p.distance_to(pond) < POND_RADIUS + 3.0:
				hq[id] = [POND_LEVEL, false] # banks
	var basin := {}
	for w in water:
		basin[Vector2i(int(w.x), int(w.y))] = true
	# Pass 4: write heights, smooth flags and slots per chunk in bulk.
	var cells := 0
	for c in t.get_chunks():
		var ids = t.get_chunk_cells(c)
		var heights := PackedFloat32Array()
		var floors := PackedByteArray()
		var walls := PackedByteArray()
		for id in ids:
			var q: Array = hq[id]
			var h: float = q[0]
			heights.append(h)
			var slot := _floor_slot(h, q[1], pos[id], paths, forest)
			if ramp.has(id):
				slot = 11
			elif basin.has(id):
				slot = 3 # sandy bed
			floors.append(slot)
			walls.append(6 if h < 3.0 else 5)
			if q[1]:
				t.set_sample_smooth(id, true)
		t.set_chunk_heights(c, heights)
		t.set_chunk_slots(c, floors, false)
		t.set_chunk_slots(c, walls, true)
		cells += ids.size()
	t.commit_to_data()
	# Raised water for the scene's PixelWater (build_demo_scenes.gd copies it to water_cells).
	t.data.set_meta("water_cells", water)
	var err := save_with_uid(t.data, "res://demo/hex_island_data.res")
	print("hex island data: ", err, " chunks=", t.get_chunks().size(), " cells=", cells, " water=", water)
	if map:
		_write_map(hq, pos, ramp, basin)
	t.queue_free()


# Top view of the levels (1 px = 0.5 m, north up, origin in the centre) for layout work.
func _write_map(hq: Dictionary, pos: Dictionary, ramp: Dictionary, basin: Dictionary) -> void:
	var img := Image.create(280, 280, false, Image.FORMAT_RGB8)
	img.fill(Color(0.1, 0.2, 0.4))
	for id in hq:
		var p: Vector2 = pos[id]
		var h: float = hq[id][0]
		var col := Color(0.85, 0.78, 0.55) if hq[id][1] and h < 2.4 else Color.from_hsv(0.3 - h * 0.012, 0.6, 0.35 + h * 0.04)
		if h < 0.45:
			col = Color(0.2, 0.45, 0.6)
		if ramp.has(id):
			col = Color(0.8, 0.5, 0.3)
		if basin.has(id):
			col = Color(0.3, 0.7, 1.0)
		var px := Vector2i(int((p.x + 70.0) * 2.0), int((p.y + 70.0) * 2.0))
		for dy in range(-2, 2):
			for dx in range(-2, 2):
				var q := px + Vector2i(dx, dy)
				if q.x >= 0 and q.y >= 0 and q.x < 280 and q.y < 280:
					img.set_pixelv(q, col)
	# 10 m grid ticks.
	for k in range(-60, 70, 10):
		for j in 280:
			img.set_pixel(int((k + 70) * 2), j, img.get_pixel(int((k + 70) * 2), j).darkened(0.25))
			img.set_pixel(j, int((k + 70) * 2), img.get_pixel(j, int((k + 70) * 2)).darkened(0.25))
	img.save_png("res://tests/captures/island_levels.png")
