# main_menu.gd — escena principal (agente E): lista de canciones y accesos a emparejar el móvil,
# calibrar y salir. La UI se construye en _ready.
extends Control

const STATUS_PERIOD_S := 0.5

var _session: UiSession
var _library := SongLibrary.new()
var _songs: Array[Dictionary] = []
var _replays: Array = []

var _list: ItemList
var _info: Label
var _play_btn: Button
var _replay_opt: OptionButton
var _fps_check: CheckBox
var _status: Label
var _status_timer := 0.0


func _ready() -> void:
	_session = UiSession.get_state()
	_build_ui()
	_reload()
	_update_status()


func _process(delta: float) -> void:
	_status_timer -= delta
	if _status_timer <= 0.0:
		_status_timer = STATUS_PERIOD_S
		_update_status()


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed(&"ui_accept") and not _play_btn.disabled:
		_on_play()


# ------------------------------------------------------------------ interfaz

func _build_ui() -> void:
	var box := UiStyle.page(self)
	box.add_child(UiStyle.label("Juego de Baile", 48, UiStyle.ACCENT))

	var body := UiStyle.hbox(24)
	body.size_flags_vertical = Control.SIZE_EXPAND_FILL
	box.add_child(body)

	var left := VBoxContainer.new()
	left.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	left.size_flags_stretch_ratio = 1.4
	body.add_child(left)
	left.add_child(UiStyle.label("Canciones", 26))
	_list = ItemList.new()
	_list.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_list.add_theme_font_size_override("font_size", 22)
	_list.item_selected.connect(_on_song_selected)
	_list.item_activated.connect(_on_song_activated)
	left.add_child(_list)

	var right := VBoxContainer.new()
	right.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	right.add_theme_constant_override("separation", 10)
	body.add_child(right)
	_info = UiStyle.label("", 20, UiStyle.TEXT_DIM)
	_info.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_info.size_flags_vertical = Control.SIZE_EXPAND_FILL
	right.add_child(_info)

	right.add_child(UiStyle.label("Entrada si no hay móvil conectado:", 18, UiStyle.TEXT_DIM))
	_replay_opt = OptionButton.new()
	_replay_opt.add_theme_font_size_override("font_size", 18)
	_replay_opt.item_selected.connect(_on_replay_selected)
	right.add_child(_replay_opt)
	_fps_check = CheckBox.new()
	_fps_check.text = "Registrar fps (user://fps_log.csv)"
	_fps_check.button_pressed = _session.fps_log
	_fps_check.toggled.connect(func(on: bool) -> void: _session.fps_log = on)
	right.add_child(_fps_check)

	_play_btn = UiStyle.button("Jugar", 30)
	_play_btn.pressed.connect(_on_play)
	right.add_child(_play_btn)

	var row := UiStyle.hbox()
	box.add_child(row)
	var b_phone := UiStyle.button("Conectar móvil")
	b_phone.pressed.connect(func() -> void: get_tree().change_scene_to_file(UiSession.PHONE_PAIR_SCENE))
	row.add_child(b_phone)
	var b_cal_in := UiStyle.button("Calibrar entrada")
	b_cal_in.pressed.connect(func() -> void: _session.open_calibration(get_tree(), UiSession.CAL_INPUT_SCENE))
	row.add_child(b_cal_in)
	var b_cal_vis := UiStyle.button("Calibrar pantalla")
	b_cal_vis.pressed.connect(func() -> void: _session.open_calibration(get_tree(), UiSession.CAL_VISUAL_SCENE))
	row.add_child(b_cal_vis)
	var b_reload := UiStyle.button("Recargar")
	b_reload.pressed.connect(_reload)
	row.add_child(b_reload)
	var spacer := Control.new()
	spacer.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(spacer)
	var b_quit := UiStyle.button("Salir")
	b_quit.pressed.connect(_on_quit)
	row.add_child(b_quit)

	_status = UiStyle.label("", 16, UiStyle.TEXT_DIM)
	box.add_child(_status)


func _reload() -> void:
	_songs = _library.scan()
	_list.clear()
	var select := -1
	var prev_id := str(_session.song.get("id", ""))
	for i in range(_songs.size()):
		var pkg: Dictionary = _songs[i]
		var text := str(pkg["title"])
		if str(pkg["artist"]) != "":
			text += " — " + str(pkg["artist"])
		var idx := _list.add_item(text)
		if not bool(pkg["ok"]):
			_list.set_item_text(idx, text + "  (no válida)")
			_list.set_item_custom_fg_color(idx, Color(0.6, 0.45, 0.45))
			var errs: PackedStringArray = pkg["errors"]
			_list.set_item_tooltip(idx, "\n".join(errs))
		elif select < 0 or str(pkg["id"]) == prev_id:
			select = idx
	_reload_replays()
	if select >= 0:
		_list.select(select)
		_on_song_selected(select)
	else:
		_play_btn.disabled = true
		var dirs := PackedStringArray([ProjectSettings.globalize_path(SongLibrary.USER_SONGS_DIR)])
		dirs.append_array(_library.extra_dirs)
		_info.text = "No hay canciones válidas.\nImporta una con tools/youtube_import (uso personal) en:\n" + "\n".join(dirs)


func _reload_replays() -> void:
	_replays = UiSessionLibrary.scan()
	_replay_opt.clear()
	_replay_opt.add_item("Automática: sesión grabada de la canción o simulación")
	_replay_opt.set_item_metadata(0, UiSession.REPLAY_AUTO)
	_replay_opt.add_item("Simulación")
	_replay_opt.set_item_metadata(1, UiSession.REPLAY_NONE)
	var sel := 0 if _session.replay_choice == UiSession.REPLAY_AUTO else (1 if _session.replay_choice == UiSession.REPLAY_NONE else 0)
	for s: Variant in _replays:
		var info: Dictionary = s
		_replay_opt.add_item("Sesión: %s" % str(info["name"]))
		var i := _replay_opt.item_count - 1
		_replay_opt.set_item_metadata(i, str(info["path"]))
		if str(info["path"]) == _session.replay_choice:
			sel = i
	_replay_opt.select(sel)
	_session.replay_choice = str(_replay_opt.get_item_metadata(sel))


func _update_status() -> void:
	var phones := "servidor de móviles cerrado"
	if _session.phone_server != null:
		phones = "móviles conectados: %d" % UiSession.connected_devices(_session.phone_server)
	_status.text = "%s · %s" % [EngineFactory.describe(), phones]


# ------------------------------------------------------------------ acciones

func _on_song_selected(index: int) -> void:
	if index < 0 or index >= _songs.size():
		return
	var pkg: Dictionary = _songs[index]
	var lines := PackedStringArray()
	lines.append(str(pkg["title"]))
	if str(pkg["artist"]) != "":
		lines.append(str(pkg["artist"]))
	lines.append("%.0f BPM · %d:%02d · %d pasos" % [
		float(pkg["bpm"]), int(float(pkg["duration_s"])) / 60, int(float(pkg["duration_s"])) % 60,
		(pkg["steps"] as Array).size()])
	lines.append("Vídeo del bailarín: %s" % ("sí" if str(pkg["video_path"]) != "" else "no"))
	if not bool(pkg["ok"]):
		lines.append("")
		lines.append("Errores:")
		var errs: PackedStringArray = pkg["errors"]
		lines.append_array(errs)
	_info.text = "\n".join(lines)
	_play_btn.disabled = not bool(pkg["ok"])


func _on_song_activated(index: int) -> void:
	_on_song_selected(index)
	if not _play_btn.disabled:
		_on_play()


func _on_replay_selected(index: int) -> void:
	_session.replay_choice = str(_replay_opt.get_item_metadata(index))


func _on_play() -> void:
	var sel := _list.get_selected_items()
	if sel.is_empty():
		return
	var pkg: Dictionary = _songs[sel[0]]
	if not bool(pkg["ok"]):
		return
	_session.song = pkg
	get_tree().change_scene_to_file(UiSession.GAMEPLAY_SCENE)


func _on_quit() -> void:
	_session.shutdown()
	get_tree().quit()
