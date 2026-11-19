# PixelDiagnostics overlay check (windowed): opens the overlay on a scene and drives its
# Effects tab through the real buttons: weather Rain -> Snow (Instant), lightning, a feature
# toggle, the debug view list, then captures the Effects tab.
#   godot --path . --script res://tests/diagnostics_check.gd -- [scene=res://demo/hex_island.tscn] [name=diagnostics]
# Prints DIAG OK / DIAG PROBLEM ...; writes tests/captures/<name>_{rain,snow}.png.
extends SceneTree

var frames := 0
var args := {}
var inst: Node
var diag: Node
var weather: Node
var problems: Array[String] = []

func _initialize() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		args[kv[0]] = kv[1] if kv.size() > 1 else "1"
	var ps: PackedScene = load(args.get("scene", "res://demo/hex_island.tscn"))
	inst = ps.instantiate()
	root.add_child.call_deferred(inst)

func _check(ok: bool, what: String) -> void:
	if not ok:
		problems.append(what)

func _button(text: String) -> Button:
	for b in diag.find_children("*", "Button", true, false):
		if b.text == text:
			return b
	return null

func _press(text: String) -> bool:
	var b := _button(text)
	if b == null:
		problems.append("no visible button \"%s\"" % text)
		return false
	if b.toggle_mode:
		b.button_pressed = not b.button_pressed
	else:
		b.pressed.emit()
	return true

func _titles() -> Array:
	var t := []
	for f in diag.find_children("*", "FoldableContainer", true, false):
		t.append(f.title)
	return t

func _open_section(prefix: String) -> void:
	for f in diag.find_children("*", "FoldableContainer", true, false):
		f.folded = not (f.title as String).begins_with(prefix)

func _process(_d: float) -> bool:
	frames += 1
	var name: String = args.get("name", "diagnostics")
	if frames == 2:
		diag = _of(inst, "PixelDiagnostics")
		weather = _of(inst, "PixelWeather")
		_check(diag != null, "scene has a PixelDiagnostics")
		_check(weather != null, "scene has a PixelWeather")
		if diag == null or weather == null:
			return _finish()
		diag.toggle()
	if frames == 6:
		var titles := _titles()
		print("DIAG sections: ", ", ".join(titles))
		for want in ["Features", "PixelWeather", "PixelDayNight", "PixelRenderer"]:
			_check(titles.any(func(t): return (t as String).begins_with(want)), "section " + want)
		var tabs: TabContainer = diag.find_children("*", "TabContainer", true, false)[0]
		_check(tabs.get_tab_count() == 3, "three tuner tabs")
		var dv: OptionButton = null
		for o in diag.find_children("*", "OptionButton", true, false):
			if o.get_item_count() > 0 and o.get_item_text(0) == "Final":
				dv = o
				break
		_check(dv != null and dv.get_item_count() >= 8, "debug view list has every view (incl. Outline Cause / God Rays)")
		_open_section("PixelWeather")
		var inst_box: CheckBox = null
		for c in diag.find_children("*", "CheckBox", true, false):
			if c.text == "Instant":
				inst_box = c
		_check(inst_box != null, "Instant switch")
		if inst_box:
			inst_box.button_pressed = true
		_press("Rain")
	if frames == 40:
		_check(weather.get_weather_name() == "Rain", "Rain button -> weather Rain (got %s)" % weather.get_weather_name())
		root.get_texture().get_image().save_png("res://tests/captures/%s_rain.png" % name)
		_press("Lightning")
		_press("Snow")
	if frames == 80:
		_check(weather.get_weather_name() == "Snow", "Snow button -> weather Snow (got %s)" % weather.get_weather_name())
		var status := ""
		for l in diag.find_children("*", "Label", true, false):
			if (l.text as String).contains("fps"):
				status = l.text
		print("DIAG status: ", status.replace("\n", " / "))
		_check(status.contains("weather Snow"), "status line shows the weather")
		root.get_texture().get_image().save_png("res://tests/captures/%s_snow.png" % name)
		# Feature toggle through the overlay (lasts for the session only).
		var birds_on := PixelFeatures.is_enabled("birds")
		for c in diag.find_children("*", "CheckBox", true, false):
			if c.text == "Birds":
				c.button_pressed = not birds_on
		_check(PixelFeatures.is_enabled("birds") != birds_on, "Birds feature switch")
		PixelFeatures.set_enabled("birds", birds_on)
		var dn := _of(inst, "PixelDayNight")
		if dn:
			_open_section("PixelDayNight")
			_press("Noon")
			_check(absf(dn.time_of_day - 0.5) < 0.01, "Noon button")
	if frames == 90:
		return _finish()
	return false

func _finish() -> bool:
	if problems.is_empty():
		print("DIAG OK")
	else:
		for p in problems:
			print("DIAG PROBLEM ", p)
	quit(0 if problems.is_empty() else 1)
	return true

func _of(n: Node, cls: String) -> Node:
	var found := n.find_children("*", cls, true, false)
	return found[0] if found.size() > 0 else null
