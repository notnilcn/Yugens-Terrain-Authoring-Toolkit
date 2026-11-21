extends Camera3D
## Friendly orbit camera for the tutorial scenes.
## Drag with the right mouse button to orbit. Mouse wheel zooms. WASD pans,
## Q / E moves down / up. No project input map needed.

@export var target := Vector3.ZERO
@export var distance := 40.0
@export var yaw_deg := 38.0
@export var pitch_deg := 38.0
@export var min_distance := 6.0
@export var max_distance := 400.0
## Lowest / highest orbit pitch (degrees above the target's horizon). Grazing views of an
## orthographic camera look kilometres across the ground, past a finite sea: keep the minimum
## high enough that the view stays on the scene.
@export var min_pitch_deg := 5.0
@export var max_pitch_deg := 88.0
## Orthographic zoom limits (the wheel changes Size, not distance).
@export var min_size := 4.0
@export var max_size := 400.0

var _yaw := 0.0
var _pitch := 0.0
var _dist := 0.0
var _target := Vector3.ZERO
var _dragging := false

func _ready() -> void:
	_yaw = deg_to_rad(yaw_deg)
	_pitch = deg_to_rad(clampf(pitch_deg, min_pitch_deg, max_pitch_deg))
	_dist = distance
	_target = target
	_apply()

## Orthographic: the lowest edge of the view must stay this far (m) above the target, so the
## camera's near plane never dips under the sea or the terrain.
@export var ortho_clearance := 30.0

func _zoom(factor: float) -> void:
	if projection == PROJECTION_ORTHOGONAL:
		size = clampf(size * factor, min_size, max_size)
	else:
		_dist = clampf(_dist * factor, min_distance, max_distance)
	_apply()

func _apply() -> void:
	var pitch := _pitch
	var dist := _dist
	if projection == PROJECTION_ORTHOGONAL:
		# An orthographic view does not change with the distance, but its near plane is a
		# size x size rectangle through the camera: at a low pitch and a large size its bottom
		# edge went under the water, and the bottom third of the screen showed the void below
		# a straight line (#38). Back off (up to max_distance), then raise the pitch, until
		# that edge clears the target by ortho_clearance.
		var h := size * 0.5
		var need := (h * cos(pitch) + ortho_clearance) / maxf(sin(pitch), 0.02)
		dist = clampf(need, _dist, maxf(max_distance, _dist))
		var r := sqrt(dist * dist + h * h)
		var floor_pitch := atan2(h, dist) + asin(minf(ortho_clearance / r, 1.0))
		pitch = clampf(maxf(pitch, floor_pitch), deg_to_rad(min_pitch_deg), deg_to_rad(max_pitch_deg))
	var offset := Vector3(
		sin(_yaw) * cos(pitch),
		sin(pitch),
		cos(_yaw) * cos(pitch)
	) * dist
	global_position = _target + offset
	look_at(_target, Vector3.UP)

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_RIGHT:
			_dragging = event.pressed
		elif event.button_index == MOUSE_BUTTON_WHEEL_UP and event.pressed:
			_zoom(0.9)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN and event.pressed:
			_zoom(1.1)
	elif event is InputEventMouseMotion and _dragging:
		_yaw -= event.relative.x * 0.006
		_pitch = clampf(_pitch - event.relative.y * 0.006, deg_to_rad(min_pitch_deg), deg_to_rad(max_pitch_deg))
		_apply()

func _process(delta: float) -> void:
	var move := Vector3.ZERO
	if Input.is_key_pressed(KEY_W): move.z -= 1.0
	if Input.is_key_pressed(KEY_S): move.z += 1.0
	if Input.is_key_pressed(KEY_A): move.x -= 1.0
	if Input.is_key_pressed(KEY_D): move.x += 1.0
	if Input.is_key_pressed(KEY_Q): move.y -= 1.0
	if Input.is_key_pressed(KEY_E): move.y += 1.0
	if move != Vector3.ZERO:
		var flat := Vector3(move.x, 0.0, move.z).rotated(Vector3.UP, _yaw)
		_target += (flat + Vector3.UP * move.y) * (12.0 + _dist * 0.25) * delta
		_apply()
