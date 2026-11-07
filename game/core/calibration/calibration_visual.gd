# calibration_visual.gd — calibración de pantalla (B9, agente D). Escena construida por código.
# Un metrónomo en bucle (ClickTrack) suena con SongClock y un cuadro destella cuando
# SongClock.visual() pasa por cada clic. El jugador mueve el deslizador hasta que destello y clic
# coinciden: el valor es visual_offset (s; + = se dibuja adelantado porque la pantalla va con retraso).
# Se aplica en vivo a SongClock; "Guardar" lo escribe en el Profile. "Volver" sin guardar lo deshace.
#
# Instanciar: res://core/calibration/calibration_visual.tscn (o este script sobre un Control).
# Opcional antes de add_child: `profile`, `profile_path`.
extends Control

signal closed(saved: bool)

const SongClockScript := preload("res://core/SongClock.gd")

const CLICK_PERIOD_S := 0.75
const CLICK_COUNT := 8           # bucle de 6 s
const FLASH_S := 0.1
const SLIDER_MIN_MS := -150.0
const SLIDER_MAX_MS := 300.0

var profile: Profile = null
var profile_path := Profile.DEFAULT_PATH

var _clock: SongClockScript = null
var _original_offset := 0.0
var _saved := false

var _flash: ColorRect
var _bar: ColorRect
var _bar_track: Control
var _slider: HSlider
var _value_label: Label
var _msg: Label


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	if profile == null:
		profile = Profile.load_or_default(profile_path)
	_clock = _find_clock()
	_original_offset = _clock.visual_offset
	_build_ui()
	_slider.value = clampf(_clock.visual_offset * 1000.0, SLIDER_MIN_MS, SLIDER_MAX_MS)
	_update_value_label()
	_clock.play(ClickTrack.make(CLICK_PERIOD_S, CLICK_COUNT, 0.0, 0.0, true))


func _exit_tree() -> void:
	if is_instance_valid(_clock):
		_clock.stop()
		if not _saved:
			_clock.visual_offset = _original_offset


func _process(_delta: float) -> void:
	var phase := fposmod(_clock.visual(), CLICK_PERIOD_S)
	_flash.color = Color(1.0, 1.0, 1.0) if phase < FLASH_S else Color(0.12, 0.12, 0.18)
	# Barra que cruza el cuadro: llega al centro en el instante del clic (ayuda a anticipar).
	var x := fposmod(phase / CLICK_PERIOD_S + 0.5, 1.0)
	var w := _bar_track.size.x
	_bar.position = Vector2(x * w - _bar.size.x * 0.5, 0.0)


func _on_slider(value_ms: float) -> void:
	_clock.visual_offset = value_ms / 1000.0
	_update_value_label()


func _nudge(delta_ms: float) -> void:
	_slider.value = _slider.value + delta_ms   # dispara value_changed


func _update_value_label() -> void:
	_value_label.text = "visual_offset: %+.0f ms" % [_slider.value]


func _on_save() -> void:
	profile.visual_offset = _slider.value / 1000.0
	var err := profile.save(profile_path)
	_clock.apply_profile(profile)
	_saved = err == OK
	_original_offset = profile.visual_offset
	_msg.text = "Guardado (visual_offset = %+.0f ms)." % [_slider.value] if err == OK else "Error al guardar (%s)." % error_string(err)


func _on_back() -> void:
	_clock.stop()
	if not _saved:
		_clock.visual_offset = _original_offset
	closed.emit(_saved)
	if get_tree().current_scene == self:
		var main := str(ProjectSettings.get_setting("application/run/main_scene", ""))
		if main != "" and main != scene_file_path and ResourceLoader.exists(main):
			get_tree().change_scene_to_file(main)
	elif closed.get_connections().is_empty():
		queue_free()


func _find_clock() -> SongClockScript:
	var n := get_node_or_null(^"/root/SongClock")
	if n is SongClockScript:
		return n as SongClockScript
	var c: SongClockScript = SongClockScript.new()
	c.name = "LocalSongClock"
	add_child(c)
	return c


func _build_ui() -> void:
	var bg := ColorRect.new()
	bg.color = Color(0.07, 0.07, 0.11)
	add_child(bg)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var margin := MarginContainer.new()
	add_child(margin)
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side: String in ["left", "right", "top", "bottom"]:
		margin.add_theme_constant_override("margin_" + side, 40)
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 14)
	margin.add_child(box)

	_label(box, "Calibración · pantalla", 30)
	_label(box, "Mueve el deslizador hasta que el destello coincida con el clic. Si el destello llega tarde, súbelo; si llega antes, bájalo. Haz antes la calibración de audio y entrada.", 18)

	_flash = ColorRect.new()
	_flash.custom_minimum_size = Vector2(220, 220)
	_flash.size_flags_horizontal = Control.SIZE_SHRINK_CENTER
	box.add_child(_flash)

	_bar_track = Control.new()
	_bar_track.custom_minimum_size = Vector2(0, 16)
	_bar_track.clip_contents = true
	box.add_child(_bar_track)
	var mid := ColorRect.new()
	mid.color = Color(1, 1, 1, 0.35)
	mid.anchor_left = 0.5
	mid.anchor_right = 0.5
	mid.anchor_bottom = 1.0
	mid.offset_left = -1.0
	mid.offset_right = 1.0
	_bar_track.add_child(mid)
	_bar = ColorRect.new()
	_bar.color = Color(1.0, 0.85, 0.2)
	_bar.size = Vector2(10, 16)
	_bar_track.add_child(_bar)

	_value_label = _label(box, "", 20)
	_slider = HSlider.new()
	_slider.min_value = SLIDER_MIN_MS
	_slider.max_value = SLIDER_MAX_MS
	_slider.step = 1.0
	_slider.custom_minimum_size = Vector2(0, 32)
	_slider.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	box.add_child(_slider)
	_slider.value_changed.connect(_on_slider)

	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 12)
	box.add_child(row)
	_button(row, "−5 ms", _nudge.bind(-5.0))
	_button(row, "+5 ms", _nudge.bind(5.0))
	_button(row, "Guardar", _on_save)
	_button(row, "Volver", _on_back)
	_msg = _label(box, "", 16)


func _label(parent: Control, text: String, font_size: int) -> Label:
	var l := Label.new()
	l.text = text
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	l.add_theme_font_size_override("font_size", font_size)
	parent.add_child(l)
	return l


func _button(parent: Control, text: String, cb: Callable) -> Button:
	var b := Button.new()
	b.text = text
	b.custom_minimum_size = Vector2(120, 44)
	b.focus_mode = Control.FOCUS_NONE
	b.pressed.connect(cb)
	parent.add_child(b)
	return b
