@tool
extends RefCounted
## Shared by the windowed editor tests (terrain_editor_capture.gd, foliage_editor_check.gd,
## sculpt_editor_check.gd, brush_perf.gd): waits until the editor can take synthetic input.
##
## On a cold start (no .godot/ yet) the editor scans and imports the whole project behind a
## modal "Scanning..." / "(Re)Importing Assets" progress dialog that swallows clicks, and
## `is_scanning()` may turn true only some frames after the first check. So this waits for
## `calm_frames` consecutive frames with no scan, no visible progress dialog and no exclusive
## popup, giving up (with a message) after `timeout_s`. Leftover modal message boxes (e.g. a
## first-start warning) are logged and closed so they don't eat the test's input.
##   const EditorReady = preload("res://tests/editor_ready.gd")
##   await EditorReady.wait(get_tree())

static func wait(tree: SceneTree, timeout_s := 300.0, calm_frames := 30) -> bool:
	var t0 := Time.get_ticks_msec()
	var calm := 0
	var why := ""
	var last := ""
	while calm < calm_frames:
		why = busy_reason(tree)
		if why != last and why != "":
			print("editor_ready: waiting (%s)" % why)
		last = why
		calm = 0 if why != "" else calm + 1
		if Time.get_ticks_msec() - t0 > timeout_s * 1000.0:
			printerr("editor_ready: editor still busy after %ds (%s); continuing" % [int(timeout_s), why])
			return false
		await tree.process_frame
	return true


## Why the editor can't take input right now ("" = ready).
static func busy_reason(tree: SceneTree) -> String:
	if EditorInterface.get_resource_filesystem().is_scanning():
		return "filesystem scan"
	for n in tree.root.find_children("*", "ProgressDialog", true, false):
		if (n is CanvasItem and n.is_visible_in_tree()) or (n is Window and n.visible):
			return "progress dialog"
	for w in _windows(tree.root):
		if not (w.visible and w.exclusive):
			continue
		if w is AcceptDialog and not w.is_class("ProgressDialog"):
			var d := w as AcceptDialog
			print("editor_ready: closing modal dialog '%s': %s" % [d.title, d.dialog_text.left(160)])
			d.hide()
			return "modal dialog"
		return "popup " + w.get_class()
	return ""


static func _windows(root: Node) -> Array[Window]:
	var out: Array[Window] = []
	for n in root.find_children("*", "Window", true, false):
		out.append(n as Window)
	return out
