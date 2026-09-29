extends SceneTree
## Regression: instantiating a scene whose MarchingSquaresTerrain has a
## non-square grid_type must not run the editor-side grid switch while the
## terrain is still outside the tree. That switch used to create duplicate
## "Chunk (x, y)" nodes next to the scene's own chunk nodes, which produced
## the "incoming node's name clashes" warning and left the scene chunks
## uninitialized (out-of-bounds height_map reads).
## Run with:
##   godot --headless --editor --quit-after 60 --script res://tests/run_scene_instantiate_tests.gd

const DEMO_SCENE := "res://scenes/mst_demo_scene.tscn"

var _failures := 0


func _initialize() -> void:
	if not Engine.is_editor_hint():
		print("SCENE INSTANTIATE TESTS SKIPPED (not running in editor mode)")
		quit(0)
		return
	
	var packed := load(DEMO_SCENE) as PackedScene
	if packed == null:
		printerr("FAIL: could not load ", DEMO_SCENE)
		quit(1)
		return
	
	var root := packed.instantiate()
	var terrain := root.get_node_or_null("SubViewportContainer/SubViewport/MarchingSquaresTerrain") as MarchingSquaresTerrain
	if terrain == null:
		printerr("FAIL: demo scene has no MarchingSquaresTerrain node")
		quit(1)
		return
	
	var chunks : Array[Node] = []
	for child in terrain.get_children():
		if child is MarchingSquaresTerrainChunkBase:
			chunks.append(child)
	
	# The scene file itself stores exactly four chunk nodes; the grid switch
	# must not add runtime duplicates during instantiation.
	_check(chunks.size() == 4, "instantiation creates exactly 4 chunk nodes (got %d)" % chunks.size())
	for chunk in chunks:
		_check(chunk.name == "Chunk " + str((chunk as MarchingSquaresTerrainChunkBase).chunk_coords),
			"chunk name matches its coords: " + chunk.name)
	
	# Entering the tree must initialize the scene's own chunks (this is the
	# step that the duplicate runtime chunks used to abort, leaving empty
	# height maps behind the out-of-bounds errors).
	get_root().add_child(root)
	for i in 3:
		await process_frame
	_check(terrain.chunks.size() == 4, "terrain resolves its 4 scene chunks (got %d)" % terrain.chunks.size())
	for chunk in chunks:
		var base := chunk as MarchingSquaresTerrainChunkBase
		var cell := chunk as MarchingSquaresCellChunk
		_check(cell != null, "chunk %s is a cell chunk" % base.name)
		if cell == null:
			continue
		var cell_count := cell.cells_per_chunk().x * cell.cells_per_chunk().y
		_check(cell.height_map.size() == cell_count,
			"chunk %s height map sized on enter tree (%d/%d)" % [base.name, cell.height_map.size(), cell_count])
		_check(cell.mesh != null, "chunk %s has a mesh on enter tree" % base.name)
	
	root.free()
	
	if _failures == 0:
		print("SCENE INSTANTIATE TESTS PASSED")
		quit(0)
	else:
		printerr("SCENE INSTANTIATE TESTS FAILED: ", _failures)
		quit(1)


func _check(condition: bool, message: String) -> void:
	if not condition:
		_failures += 1
		printerr("FAIL: ", message)
		print("  FAIL: ", message)
