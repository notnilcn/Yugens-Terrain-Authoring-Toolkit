# Headless check of the courtyard fox (demo/courtyard.tscn):
#   godot --headless --path . --script res://tests/courtyard_fox_check.gd
# * PixelAnimationStepper puts the fox's AnimationPlayer in manual mode and advances it in
#   1/12 s steps (the clip position only takes multiples of 1/12 s);
# * MstDemoPlayer's ClipPlayer plays Idle standing, Run while moving, Jump on a jump and Fall
#   in the air.
# Prints COURTYARD_FOX OK (exit 0) or COURTYARD_FOX FAIL (exit 1).
extends SceneTree

var frames := 0
var scene: Node
var anim: AnimationPlayer
var fails: Array[String] = []
var positions: Array[float] = []
var seen := {}

func _initialize() -> void:
	scene = (load("res://demo/courtyard.tscn") as PackedScene).instantiate()
	root.add_child(scene)

func _press(action: String, pressed: bool) -> void:
	var ev := InputEventAction.new()
	ev.action = action
	ev.pressed = pressed
	Input.parse_input_event(ev)

func _process(_d: float) -> bool:
	frames += 1
	if frames == 5:
		var players := scene.get_node("PlayerRig").find_children("*", "AnimationPlayer", true, false)
		if players.is_empty():
			fails.append("no AnimationPlayer under the fox")
			return _finish()
		anim = players[0]
		if anim.callback_mode_process != AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_MANUAL:
			fails.append("AnimationPlayer not in manual mode (%d)" % anim.callback_mode_process)
	if anim == null:
		return false
	seen[String(anim.current_animation)] = true
	if frames >= 60 and frames < 120:
		positions.append(anim.current_animation_position)
	if frames == 120:
		if not seen.has("Idle"):
			fails.append("Idle never played while standing (seen %s)" % [seen.keys()])
		var distinct := {}
		for p in positions:
			var steps := p * 12.0
			if absf(steps - roundf(steps)) > 0.02:
				fails.append("clip position %.4f is not a 1/12 s step" % p)
				break
			distinct[snappedf(p, 0.001)] = true
		# ~1 s of frames: the pose must move, but only about 12 times.
		if distinct.size() < 4:
			fails.append("animation did not advance (%d distinct poses)" % distinct.size())
		seen.clear()
		_press("player_forward", true)
	if frames == 200:
		_press("player_forward", false)
		if not seen.has("Run"):
			fails.append("Run never played while moving (seen %s)" % [seen.keys()])
		seen.clear()
	if frames == 260:
		_press("player_jump", true)
	if frames == 264:
		_press("player_jump", false)
	if frames == 340:
		if not seen.has("Jump") and not seen.has("Fall"):
			fails.append("neither Jump nor Fall played after a jump (seen %s)" % [seen.keys()])
		return _finish()
	return false

func _finish() -> bool:
	if fails.is_empty():
		print("COURTYARD_FOX OK")
	else:
		for f in fails:
			print("  ", f)
		print("COURTYARD_FOX FAIL")
	quit(0 if fails.is_empty() else 1)
	return true
