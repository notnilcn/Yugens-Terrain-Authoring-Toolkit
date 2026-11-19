# Test-only orbit framing for a plain Camera3D (the demo scenes' overview `Camera`).
#
#   var cam = preload("res://tests/orbit_view.gd").attach(inst.get_node("Camera"))
#   cam.target = Vector3(0, 10, 4); cam.yaw_deg = 45.0; cam.ortho_size = 26.0
#
# `attach` derives target / yaw / pitch / projection from the camera's current transform
# (target = `distance` metres along its view axis), so the scene's authored view is kept until a
# test changes something. Every property write re-places the camera.
extends Camera3D

var target := Vector3.ZERO:
	set(v):
		target = v
		_place()
var distance := 40.0:
	set(v):
		distance = v
		_place()
var yaw_deg := 45.0:
	set(v):
		yaw_deg = v
		_place()
var pitch_deg := 50.0:
	set(v):
		pitch_deg = v
		_place()
var orthographic := true:
	set(v):
		orthographic = v
		_place()
var ortho_size := 24.0:
	set(v):
		ortho_size = v
		_place()
## Accepted for older test scripts; the view never eases.
var smoothing := 0.0

var _ready_to_place := false


## Gives `cam` this script, keeping its current view. Returns the same camera (null for null).
static func attach(cam: Camera3D, dist := 40.0):
	if cam == null:
		return null
	var xf := cam.global_transform if cam.is_inside_tree() else cam.transform
	var back := xf.basis.z.normalized()
	var ortho := cam.projection == Camera3D.PROJECTION_ORTHOGONAL
	var size := cam.size
	cam.set_script(load("res://tests/orbit_view.gd"))
	var c = cam
	c.distance = dist
	c.target = xf.origin - back * dist
	c.pitch_deg = rad_to_deg(asin(clampf(back.y, -1.0, 1.0)))
	c.yaw_deg = rad_to_deg(atan2(back.x, back.z))
	c.orthographic = ortho
	c.ortho_size = size
	c._ready_to_place = true
	c._place()
	return cam


func focus(point: Vector3) -> void:
	target = point


func _place() -> void:
	if not _ready_to_place:
		return
	var basis := Basis.from_euler(Vector3(-deg_to_rad(pitch_deg), deg_to_rad(yaw_deg), 0.0), EULER_ORDER_YXZ)
	var xf := Transform3D(basis, target + basis.z * maxf(distance, 0.01))
	if is_inside_tree():
		global_transform = xf
	else:
		transform = xf
	projection = PROJECTION_ORTHOGONAL if orthographic else PROJECTION_PERSPECTIVE
	if orthographic:
		size = ortho_size
