# calibration_input.gd — calibración de audio + entrada (B9, agente D). Escena construida por código.
# Suena un metrónomo (ClickTrack) con SongClock; el jugador sacude el móvil en cada clic (o pulsa
# Espacio). Las sacudidas se detectan con CalibrationLogic.detect_shakes sobre JDSource.poll_samples,
# se pasan a tiempo "audible" de canción y CalibrationLogic.estimate_offset da input_offset
# (+ = el jugador/la entrada va tarde). "Guardar" lo escribe en el Profile (user://profile.cfg).
#
# Instanciar: res://core/calibration/calibration_input.tscn (o este script sobre un Control).
# Antes de add_child se puede asignar `source` (una fuente ya arrancada; no se para al salir) y
# `profile`. Si no, crea un servidor de móviles con EngineFactory y lo para al salir.
# Con una fuente simulada (MockJDSource) programa sacudidas falsas al ritmo para probar sin móvil.
# No usar a la vez que JDEngine.pump sobre la misma fuente (poll_samples consume las muestras).
extends Control

## Al salir con "Volver". saved = se guardó un resultado.
signal closed(saved: bool)

const SongClockScript := preload("res://core/SongClock.gd")

const CLICK_PERIOD_S := 0.6      # 100 BPM; el emparejamiento admite ±0.3 s
const CLICK_COUNT := 16
const LEAD_IN_S := 2.0
const TAIL_S := 1.0
const MAX_ABS_S := 0.3
const POLL_BATCH := 512
const FLASH_S := 0.1
const UNRELIABLE_STDDEV_S := 0.04

enum State { IDLE, RUNNING, DONE }

## Fuente de movimiento (JDSource o MockJDSource). null → se crea un servidor de móviles.
var source: Object = null
## Perfil a modificar. null → se carga de profile_path.
var profile: Profile = null
var profile_path := Profile.DEFAULT_PATH
## Con fuente simulada, programar sacudidas falsas (latencia 30 ms) para probar sin móvil.
var simulate_with_mock := true

var _clock: SongClockScript = null
var _own_source := false
var _state: State = State.IDLE
var _click_times := PackedFloat64Array()
var _samples: Array = []
var _key_events_mono := PackedFloat64Array()
var _mapping_raw := 0.0          # audible − mono (sin input_offset), del último fotograma sonando
var _result := {}
var _saved := false
var _simulated := false
var _frame := 0
var _last_mag := 0.0

var _info: Label
var _status: Label
var _live: Label
var _result_label: Label
var _beat: ColorRect
var _start_btn: Button
var _save_btn: Button
var _back_btn: Button


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_build_ui()
	if profile == null:
		profile = Profile.load_or_default(profile_path)
	_result_label.text = "input_offset actual: %+.0f ms." % [profile.input_offset * 1000.0]
	_clock = _find_clock()
	_clock.song_finished.connect(_on_song_finished)
	if source == null:
		source = EngineFactory.create_phone_server(profile.phone_port, 0)
		_own_source = true
		if source != null and not bool(source.call(&"start")):
			_result_label.text = "No se pudo abrir el servidor de móviles: %s" % str(source.call(&"last_error"))
	_update_status()


func _exit_tree() -> void:
	if is_instance_valid(_clock):
		if _state == State.RUNNING:
			_clock.stop()
		if _clock.song_finished.is_connected(_on_song_finished):
			_clock.song_finished.disconnect(_on_song_finished)
	if _own_source and source != null:
		source.call(&"stop")


func _input(event: InputEvent) -> void:
	if _state != State.RUNNING:
		return
	var key := event as InputEventKey
	if key != null and key.pressed and not key.echo and key.keycode == KEY_SPACE:
		_key_events_mono.append(_clock.now_mono_s())
		get_viewport().set_input_as_handled()


func _process(_delta: float) -> void:
	_frame += 1
	if _frame % 30 == 0:
		_update_status()
	if _state != State.RUNNING:
		_drain_source()   # que no se acumulen muestras viejas
		return
	_poll_source()
	if _clock.is_playing():
		# audible = t_mono + mapping sin input_offset (lo que se quiere medir).
		_mapping_raw = _clock.engine_mapping() + _clock.input_offset
	_maybe_simulate()
	var a := _clock.audible()
	var lit := false
	for c: float in _click_times:
		if a >= c and a < c + FLASH_S:
			lit = true
			break
	_beat.color = Color(1.0, 0.85, 0.2) if lit else Color(0.2, 0.2, 0.28)
	if _frame % 10 == 0:
		var n_shakes := CalibrationLogic.detect_shakes(_samples).size()
		_live.text = "Sacudidas detectadas: %d · Espacio: %d · |a| = %.1f m/s²" % [n_shakes, _key_events_mono.size(), _last_mag]
	# Por si no llega song_finished.
	if a > _click_times[_click_times.size() - 1] + TAIL_S + 0.5:
		_finish()


# ------------------------------------------------------------------ flujo

func _on_start() -> void:
	_samples.clear()
	_key_events_mono.clear()
	_result = {}
	_simulated = false
	_drain_source()
	_click_times = ClickTrack.click_times(CLICK_PERIOD_S, CLICK_COUNT, LEAD_IN_S)
	_clock.play(ClickTrack.make(CLICK_PERIOD_S, CLICK_COUNT, LEAD_IN_S, TAIL_S, false))
	_mapping_raw = _clock.engine_mapping() + _clock.input_offset
	_state = State.RUNNING
	_start_btn.disabled = true
	_save_btn.disabled = true
	_result_label.text = "Escuchando… sacude el móvil (o pulsa Espacio) justo en cada clic."


func _on_song_finished() -> void:
	_finish()


func _finish() -> void:
	if _state != State.RUNNING:
		return
	_poll_source()
	_state = State.DONE
	_clock.stop()
	_beat.color = Color(0.2, 0.2, 0.28)
	_start_btn.disabled = false
	_start_btn.text = "Repetir"

	var phone_events := PackedFloat64Array()
	for t: float in CalibrationLogic.detect_shakes(_samples):
		phone_events.append(t + _mapping_raw)
	var key_events := PackedFloat64Array()
	for t: float in _key_events_mono:
		key_events.append(t + _mapping_raw)
	var r_phone := CalibrationLogic.estimate_offset(_click_times, phone_events, MAX_ABS_S)
	var r_key := CalibrationLogic.estimate_offset(_click_times, key_events, MAX_ABS_S)

	var used := ""
	if bool(r_phone["ok"]):
		_result = r_phone
		used = "sacudidas del móvil"
	elif bool(r_key["ok"]):
		_result = r_key
		used = "tecla Espacio (mide el teclado, no el móvil)"
	else:
		_result = {}
		_result_label.text = "No hay suficientes golpes a ritmo (móvil: %d, Espacio: %d; hacen falta 4). Repite." % [int(r_phone["n_used"]), int(r_key["n_used"])]
		return
	var off: float = _result["offset"]
	var sd: float = _result["stddev"]
	var txt := "Desfase medido con %s: %+.0f ms (± %.0f ms, %d golpes). + = llegas tarde al clic." % [used, off * 1000.0, sd * 1000.0, int(_result["n_used"])]
	if sd > UNRELIABLE_STDDEV_S:
		txt += "\nPoco fiable (mucha dispersión): mejor repetir."
	if _simulated:
		txt += "\n(Sacudidas SIMULADAS: el móvil es falso.)"
	txt += "\nActual: %+.0f ms. Pulsa Guardar para usarlo." % [profile.input_offset * 1000.0]
	_result_label.text = txt
	_save_btn.disabled = false


func _on_save() -> void:
	if _result.is_empty():
		return
	profile.input_offset = float(_result["offset"])
	var err := profile.save(profile_path)
	_clock.apply_profile(profile)
	_saved = err == OK
	_save_btn.disabled = true
	_result_label.text += "\nGuardado (input_offset = %+.0f ms)." % [profile.input_offset * 1000.0] if err == OK else "\nError al guardar (%s)." % error_string(err)


func _on_back() -> void:
	if _state == State.RUNNING:
		_clock.stop()
		_state = State.IDLE
	closed.emit(_saved)
	if get_tree().current_scene == self:
		var main := str(ProjectSettings.get_setting("application/run/main_scene", ""))
		if main != "" and main != scene_file_path and ResourceLoader.exists(main):
			get_tree().change_scene_to_file(main)
	elif closed.get_connections().is_empty():
		queue_free()


# ------------------------------------------------------------------ fuente

func _poll_source() -> void:
	if source == null:
		return
	for _i in range(16):
		var got: Variant = source.call(&"poll_samples", POLL_BATCH)
		if not (got is Array):
			return
		var arr: Array = got
		for s: Variant in arr:
			if typeof(s) == TYPE_DICTIONARY:
				_samples.append(s)
				var a: Variant = (s as Dictionary).get("a", null)
				if a is Vector3:
					_last_mag = (a as Vector3).length()
		if arr.size() < POLL_BATCH:
			return


func _drain_source() -> void:
	if source == null:
		return
	for _i in range(16):
		var got: Variant = source.call(&"poll_samples", POLL_BATCH)
		if not (got is Array) or (got as Array).size() < POLL_BATCH:
			return


func _maybe_simulate() -> void:
	if _simulated or not simulate_with_mock or not (source is MockJDSource):
		return
	if _clock.song_time() < 0.5:   # que el reloj se haya asentado
		return
	_simulated = true
	# El clic 0 se oye cuando audible == click_times[0] → instante monotónico = click − mapping.
	var first_click_ns := int((_click_times[0] - _mapping_raw) * 1e9)
	(source as MockJDSource).simulate_rhythm(first_click_ns, CLICK_PERIOD_S, CLICK_COUNT)


func _update_status() -> void:
	if source == null:
		_status.text = "Sin fuente de movimiento: usa la tecla Espacio."
		return
	var lines := PackedStringArray()
	var ep := str(source.call(&"endpoint")) if source.has_method("endpoint") else ""
	if ep != "":
		lines.append("Conecta la app del móvil a %s · código %04d" % [ep, int(source.call(&"pairing_code"))])
	var devs: Variant = source.call(&"devices")
	var n := 0
	if devs is Array:
		for d: Variant in devs:
			if typeof(d) != TYPE_DICTIONARY:
				continue
			var dd: Dictionary = d
			n += 1
			lines.append("• %s — %s, rtt %.0f ms" % [str(dd.get("name", "?")), "conectado" if bool(dd.get("connected", false)) else "desconectado", float(dd.get("sync_rtt_ms", 0.0))])
	if n == 0:
		lines.append("Ningún móvil conectado todavía (puedes usar Espacio).")
	if EngineFactory.is_mock(source):
		lines.append("(Simulación: sin GDExtension o fuente falsa.)")
	_status.text = "\n".join(lines)


# ------------------------------------------------------------------ interfaz

func _find_clock() -> SongClockScript:
	var n := get_node_or_null(^"/root/SongClock")
	if n is SongClockScript:
		return n as SongClockScript
	# Sin autoload (p. ej. en pruebas): reloj local.
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

	_label(box, "Calibración · audio y entrada", 30)
	_info = _label(box, "Sonarán %d clics. Sujeta el móvil y dale un golpe seco (sacúdelo) justo cuando oigas cada clic. Sin móvil, pulsa Espacio. Usa los mismos altavoces/auriculares con los que juegas." % CLICK_COUNT, 18)
	_status = _label(box, "", 16)
	_beat = ColorRect.new()
	_beat.custom_minimum_size = Vector2(140, 140)
	_beat.color = Color(0.2, 0.2, 0.28)
	_beat.size_flags_horizontal = Control.SIZE_SHRINK_CENTER
	box.add_child(_beat)
	_live = _label(box, "", 16)
	_result_label = _label(box, "", 18)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 12)
	box.add_child(row)
	_start_btn = _button(row, "Empezar", _on_start)
	_save_btn = _button(row, "Guardar", _on_save)
	_save_btn.disabled = true
	_back_btn = _button(row, "Volver", _on_back)


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
	b.custom_minimum_size = Vector2(140, 44)
	b.focus_mode = Control.FOCUS_NONE   # que Espacio no pulse botones
	b.pressed.connect(cb)
	parent.add_child(b)
	return b
