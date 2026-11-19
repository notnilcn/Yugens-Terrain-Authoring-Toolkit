# Feature toggles and the shared clock (features.rs, clock.rs) on the hex island:
#   godot --headless --path . --script res://tests/features_check.gd
# Switches features off and on through the PixelFeatures script API and checks that the gated
# nodes hide / restore their internal meshes and stop / resume processing, and that the shared
# clock advances at runtime (global shader parameters cannot be read back outside the editor).
# Prints FEATURES_CHECK OK and exits 0 on success.
extends SceneTree

var scene: Node
var frame := 0
var failures: Array[String] = []
var t0 := 0.0


func _initialize() -> void:
	for key in PixelFeatures.get_feature_keys():
		PixelFeatures.set_enabled(key, true)
	scene = (load("res://demo/hex_island.tscn") as PackedScene).instantiate()
	root.add_child(scene)


func check(ok: bool, what: String) -> void:
	if not ok:
		failures.append(what)
		printerr("FEATURES_CHECK FAIL: ", what)


## Visible internal (unsaved) children of a node.
func internals_visible(n: Node) -> bool:
	var internal := n.get_child_count(true) - n.get_child_count()
	for i in internal:
		var c := n.get_child(i, true)
		if c is Node3D and (c as Node3D).visible:
			return true
	return false


func _process(_d: float) -> bool:
	frame += 1
	var birds := scene.get_node("Birds")
	var water := scene.get_node("PixelWater")
	var flies := scene.get_node("Fireflies")
	match frame:
		10:
			t0 = PixelFeatures.get_time()
			check(PixelFeatures.is_enabled("birds"), "birds on after set_enabled")
			check(birds.get_flock() != null and birds.get_flock().visible, "flock visible with birds on")
			check(internals_visible(water), "water surface visible with water on")
			check(internals_visible(flies), "fireflies visible with fireflies on")
		20:
			check(PixelFeatures.get_time() > t0, "pt_time advances at runtime")
			check("rays on" in scene.get_node("PixelRenderer").get_status_line(), "god rays on in the renderer status")
			for key in ["birds", "water", "fireflies", "clouds", "god_rays", "weather"]:
				PixelFeatures.set_enabled(key, false)
		25:
			check(not birds.get_flock().visible, "flock hidden with birds off")
			check(not birds.is_processing(), "birds stop processing when off")
			check(not internals_visible(water), "water hidden with water off")
			check("rays off" in scene.get_node("PixelRenderer").get_status_line(), "god rays off in the renderer status")
			check(not scene.get_node("Weather").is_processing(), "weather stops processing when off")
			check(not internals_visible(flies), "fireflies hidden with fireflies off")
			for key in ["birds", "water", "fireflies", "clouds", "god_rays", "weather"]:
				PixelFeatures.set_enabled(key, true)
		30:
			check(birds.get_flock().visible and birds.is_processing(), "birds back on")
			check(internals_visible(water), "water back on")
			check(internals_visible(flies), "fireflies back on")
			check(scene.get_node("Weather").is_processing(), "weather back on")
			if failures.is_empty():
				print("FEATURES_CHECK OK")
			quit(0 if failures.is_empty() else 1)
	return false
