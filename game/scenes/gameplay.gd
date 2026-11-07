# gameplay.gd — partida (agente E). La UI se construye en _ready.
#
# - Audio: AudioStreamOggVorbis.load_from_file(audio.ogg) reproducido por el autoload SongClock (reloj
#   maestro, docs/02).
# - Vídeo del bailarín (video.ogv, Theora sin audio): VideoStreamPlayer mudo, ESCLAVO de
#   SongClock.visual() con VideoSyncLogic. Primero se corrige con seek (stream_position = t); si el
#   seek no surte efecto (Theora en Godot 4.4 no implementa seek: ver docs/agents/E-report.md) se pasa
#   al plan B: pausar el vídeo si va adelantado y acelerarlo con speed_scale si va atrasado.
# - Cada fotograma: engine.set_time_mapping(SongClock.engine_mapping(now)), engine.pump(source),
#   engine.update(SongClock.audible()) → popups Perfect/Good/Miss, ScoreLogic y combo.
# - Fuente (UiSession.pick_source): móviles conectados > sesión grabada (replay) > simulación.
# - Log opcional "t_s,fps" en user://fps_log.csv (UiSession.fps_log, o `-- --fps-log`).
extends Control

enum State { LOADING, COUNTDOWN, PLAYING, FINISHED, FAILED }

const COUNTDOWN_S := 2.0
const LANE_ANCHOR_LEFT := 0.62
## Umbral de deriva del vídeo. El contrato usa 0,04 s por defecto, pero stream_position de Theora va a
## saltos de 1/30 s: con 0,04 se recolocaría casi cada fotograma. 0,07 s ≈ 2 fotogramas de vídeo.
const VIDEO_THRESHOLD_S := 0.07
const VIDEO_SEEK_COOLDOWN_S := 0.5
## Fotogramas tras un seek grande para comprobar que ha funcionado (si no → plan B).
const VIDEO_SEEK_CHECK_FRAMES := 8
const VIDEO_SEEK_VERIFY_S := 0.25
const VIDEO_MAX_CATCH_S := 2.0
const END_GRACE_S := 1.5
const FPS_LOG_PATH := "user://fps_log.csv"
const GRADE_TEXT := {0: "MISS", 1: "GOOD", 2: "PERFECT"}

var _session: UiSession
var _pkg: Dictionary = {}
var _profile: Profile
var _engine: Object = null
var _stream: AudioStream = null
var _source: Object = null
var _src_info: Dictionary = {}
var _replay_pending := false
var _replay_start_at := 0.0
var _score := ScoreLogic.new()
var _state: State = State.LOADING
var _countdown := 0.0
var _duration := 0.0
var _play_time := 0.0
var _idle_time := 0.0
var _expected_judgments := 0

# vídeo
var _video: VideoStreamPlayer = null
var _video_seek_ok := true
var _video_seek_cooldown := 0.0
var _video_check_target := -1.0
var _video_check_frames := 0
var _video_seeks := 0
var _video_drift := 0.0

# log de fps
var _fps_file: FileAccess = null
var _t0_mono := 0.0

# nodos
var _video_area: Control
var _lane: LaneView
var _popups: Control
var _title_l: Label
var _score_l: Label
var _combo_l: Label
var _source_l: Label
var _center_l: Label
var _debug_l: Label
var _progress: ProgressBar
var _back_btn: Button


func _ready() -> void:
	_session = UiSession.get_state()
	_pkg = _session.song
	_profile = _session.profile()
	_build_ui()
	SongClock.stop()
	SongClock.reset(0.0)
	if _pkg.is_empty() or not bool(_pkg.get("ok", false)):
		_fail("No hay ninguna canción elegida.")
		return
	_title_l.text = str(_pkg["title"]) + ("  ·  " + str(_pkg["artist"]) if str(_pkg["artist"]) != "" else "")

	_engine = EngineFactory.create_engine()
	var err := str(_engine.call(&"load_song", str(_pkg["dir"])))
	if err != "":
		_fail("El motor no pudo cargar la canción:\n" + err)
		return
	_engine.call(&"set_config", _profile.perfect_ms, _profile.good_ms, _profile.foot_scale)

	var ogg := AudioStreamOggVorbis.load_from_file(str(_pkg["audio_path"]))
	if ogg == null:
		_fail("No se pudo cargar el audio:\n" + str(_pkg["audio_path"]))
		return
	_stream = ogg
	_duration = float(_pkg["duration_s"])
	var len_s := ogg.get_length()
	if len_s > 0.0:
		_duration = len_s

	_setup_video()
	var steps: Array = _pkg["steps"]
	var times: PackedFloat64Array = _pkg["step_times"]
	_lane.setup(steps, times)
	for st: Variant in steps:
		var sd: Dictionary = st
		var limbs: PackedInt32Array = sd["limbs"]
		_expected_judgments += limbs.size()

	_src_info = _session.pick_source(_pkg)
	_source = _src_info["source"]
	_replay_start_at = float(_src_info["start_at"])
	_replay_pending = _replay_start_at >= 0.0
	_source_l.text = "Entrada: " + str(_src_info["label"])

	# Descarta muestras viejas acumuladas en la fuente compartida.
	_engine.call(&"set_time_mapping", SongClock.engine_mapping(SongClock.now_mono_s()))
	_engine.call(&"pump", _source)
	_engine.call(&"reset")

	SongClock.song_finished.connect(_on_song_finished)
	_open_fps_log()
	_countdown = COUNTDOWN_S
	_state = State.COUNTDOWN


func _exit_tree() -> void:
	if SongClock.song_finished.is_connected(_on_song_finished):
		SongClock.song_finished.disconnect(_on_song_finished)
	SongClock.stop()
	if _video != null:
		_video.stop()
	if _source != null and bool(_src_info.get("owned", false)):
		_source.call(&"stop")
	if _fps_file != null:
		_fps_file.close()
		_fps_file = null


func _process(delta: float) -> void:
	_log_fps(delta)
	match _state:
		State.COUNTDOWN:
			_countdown -= delta
			_center_l.text = "Prepárate… %d" % maxi(ceili(_countdown), 1)
			_tick_engine()
			if _countdown <= 0.0:
				_start_song()
		State.PLAYING:
			_play_time += delta
			_tick_replay()
			_tick_engine()
			var vt := SongClock.visual()
			_lane.set_time(vt)
			_sync_video(delta, vt)
			_update_hud(delta)
			# Fin: señal song_finished, o red de seguridad si el audio dejó de sonar o pasó la duración.
			if SongClock.is_playing() or SongClock.is_paused():
				_idle_time = 0.0
			else:
				_idle_time += delta
			if SongClock.audible() > _duration + END_GRACE_S or (_play_time > 1.0 and _idle_time > 0.5):
				_finish()


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed(&"ui_cancel"):
		_to_menu()


# ------------------------------------------------------------------ partida

func _start_song() -> void:
	_center_l.text = ""
	SongClock.play(_stream, 0.0)
	if _video != null:
		_video.play()
	_play_time = 0.0
	_state = State.PLAYING


func _tick_replay() -> void:
	if _replay_pending and SongClock.audible() >= _replay_start_at:
		_replay_pending = false
		if not bool(_source.call(&"start")):
			_source_l.text = "Entrada: no arrancó la sesión grabada (%s)" % str(_source.call(&"last_error"))


func _tick_engine() -> void:
	var now := SongClock.now_mono_s()
	_engine.call(&"set_time_mapping", SongClock.engine_mapping(now))
	_engine.call(&"pump", _source)
	_handle_results(_engine.call(&"update", SongClock.audible()))


func _handle_results(res: Variant) -> void:
	if not (res is Array):
		return
	for r: Variant in res:
		if not (r is Dictionary):
			continue
		var d: Dictionary = r
		var grade := int(d.get("grade", 0))
		var pts := _score.add(grade)
		_lane.mark(int(d.get("step", -1)), grade)
		_popup(grade, int(d.get("limb", 0)), pts)


func _on_song_finished() -> void:
	if _state == State.PLAYING:
		_finish()


func _finish() -> void:
	if _state != State.PLAYING:
		return
	_state = State.FINISHED
	# Recoge los pasos que el motor aún no había emitido (el último puede acabar con la música).
	_handle_results(_engine.call(&"update", _duration + 60.0))
	_session.last_result = {
		"song_id": str(_pkg.get("id", "")),
		"title": str(_pkg.get("title", "")),
		"score": _score.score,
		"max_combo": _score.max_combo,
		"counts": _score.counts.duplicate(),
		"judged": _score.total(),
		"expected": _expected_judgments,
		"source": str(_src_info.get("label", "")),
		"video_seeks": _video_seeks,
		"video_mode": _video_mode_text(),
	}
	get_tree().change_scene_to_file(UiSession.RESULTS_SCENE)


func _fail(msg: String) -> void:
	_state = State.FAILED
	_center_l.text = msg
	_back_btn.visible = true
	push_warning("gameplay: " + msg.replace("\n", " "))


func _to_menu() -> void:
	get_tree().change_scene_to_file(UiSession.MAIN_MENU_SCENE)


# ------------------------------------------------------------------ vídeo

func _setup_video() -> void:
	var vp := str(_pkg.get("video_path", ""))
	if vp == "":
		var no_video := UiStyle.label(str(_pkg["title"]), 44, Color(1, 1, 1, 0.18))
		no_video.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		no_video.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
		no_video.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		_video_area.add_child(no_video)
		no_video.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
		return
	var vs := VideoStreamTheora.new()
	vs.file = vp
	var arc := AspectRatioContainer.new()
	arc.ratio = 16.0 / 9.0
	arc.stretch_mode = AspectRatioContainer.STRETCH_FIT
	arc.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_video_area.add_child(arc)
	arc.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_video = VideoStreamPlayer.new()
	_video.stream = vs
	_video.autoplay = false
	_video.expand = true
	_video.volume = 0.0          # mudo: el audio es audio.ogg (y el .ogv no debería traer pista)
	_video.mouse_filter = Control.MOUSE_FILTER_IGNORE
	arc.add_child(_video)


## Mantiene el vídeo pegado a SongClock.visual().
func _sync_video(delta: float, vt: float) -> void:
	if _video == null or not _video.is_playing():
		return
	var vpos := _video.stream_position
	_video_drift = vpos - vt
	# ¿Funcionó el último seek grande?
	if _video_check_target >= 0.0:
		_video_check_frames += 1
		if absf(vpos - _video_check_target) < VIDEO_SEEK_VERIFY_S:
			_video_check_target = -1.0
		elif _video_check_frames >= VIDEO_SEEK_CHECK_FRAMES:
			_video_check_target = -1.0
			_video_seek_ok = false
			push_warning("gameplay: el seek del vídeo no tiene efecto; se usa el plan B (pausa / speed_scale)")
		else:
			return
	if _video_seek_ok:
		_video_seek_cooldown -= delta
		var c := VideoSyncLogic.correction(vpos, vt, VIDEO_THRESHOLD_S)
		if bool(c["seek"]) and _video_seek_cooldown <= 0.0:
			var target := float(c["target"])
			_video.stream_position = target
			_video_seeks += 1
			_video_seek_cooldown = VIDEO_SEEK_COOLDOWN_S
			if absf(target - vpos) > VIDEO_SEEK_VERIFY_S:
				_video_check_target = target
				_video_check_frames = 0
		return
	# Plan B: sin seek.
	var pb := VideoSyncLogic.plan_b(vpos, vt, VIDEO_THRESHOLD_S, VIDEO_MAX_CATCH_S)
	match str(pb["action"]):
		"hold":
			_video.paused = true
		"catch":
			_video.paused = false
			_set_video_speed(VideoSyncLogic.catch_up_speed(float(pb["amount"])))
		"lost":
			_video.paused = false
			_set_video_speed(VideoSyncLogic.catch_up_speed(float(pb["amount"]), 1.0, 2.0))
		_:
			_video.paused = false
			_set_video_speed(1.0)


func _set_video_speed(s: float) -> void:
	if "speed_scale" in _video:
		_video.set("speed_scale", s)


func _video_mode_text() -> String:
	if _video == null:
		return "sin vídeo"
	return "seek" if _video_seek_ok else "plan B (pausa/speed_scale)"


# ------------------------------------------------------------------ interfaz

func _build_ui() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var bg := ColorRect.new()
	bg.color = UiStyle.BG_COLOR
	bg.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(bg)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)

	_video_area = Control.new()
	_video_area.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_video_area)
	_set_rect(_video_area, 0.0, 0.0, LANE_ANCHOR_LEFT, 1.0)

	_lane = LaneView.new()
	_lane.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_lane)
	_set_rect(_lane, LANE_ANCHOR_LEFT, 0.0, 1.0, 1.0)
	_popups = Control.new()
	_popups.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_lane.add_child(_popups)
	_popups.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)

	var hud := VBoxContainer.new()
	hud.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(hud)
	hud.position = Vector2(24, 16)
	_title_l = _outlined(UiStyle.label("", 22, UiStyle.TEXT_DIM))
	hud.add_child(_title_l)
	_score_l = _outlined(UiStyle.label("0", 52))
	hud.add_child(_score_l)
	_combo_l = _outlined(UiStyle.label("", 30, UiStyle.ACCENT))
	hud.add_child(_combo_l)

	_progress = ProgressBar.new()
	_progress.show_percentage = false
	_progress.max_value = 1.0
	_progress.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_progress)
	_set_rect(_progress, 0.0, 1.0, LANE_ANCHOR_LEFT, 1.0)
	_progress.offset_top = -10.0

	_source_l = _outlined(UiStyle.label("", 16, UiStyle.TEXT_DIM))
	add_child(_source_l)
	_set_rect(_source_l, 0.0, 1.0, LANE_ANCHOR_LEFT, 1.0)
	_source_l.offset_left = 24.0
	_source_l.offset_top = -40.0
	_source_l.offset_bottom = -14.0

	_debug_l = _outlined(UiStyle.label("", 14, Color(1, 1, 1, 0.5)))
	_debug_l.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	add_child(_debug_l)
	_set_rect(_debug_l, 0.0, 0.0, LANE_ANCHOR_LEFT, 0.0)
	_debug_l.offset_right = -16.0
	_debug_l.offset_top = 12.0
	_debug_l.offset_bottom = 60.0

	var center := VBoxContainer.new()
	center.alignment = BoxContainer.ALIGNMENT_CENTER
	add_child(center)
	_set_rect(center, 0.0, 0.0, LANE_ANCHOR_LEFT, 1.0)
	_center_l = _outlined(UiStyle.label("Cargando…", 44))
	_center_l.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_center_l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	center.add_child(_center_l)
	_back_btn = UiStyle.button("Volver al menú")
	_back_btn.visible = false
	_back_btn.size_flags_horizontal = Control.SIZE_SHRINK_CENTER
	_back_btn.pressed.connect(_to_menu)
	center.add_child(_back_btn)


func _update_hud(_delta: float) -> void:
	_score_l.text = str(_score.score)
	_combo_l.text = "Combo %d" % _score.combo if _score.combo >= 2 else ""
	if _duration > 0.0:
		_progress.value = clampf(SongClock.audible() / _duration, 0.0, 1.0)
	var vtxt := "vídeo: %s, deriva %+.0f ms" % [_video_mode_text(), _video_drift * 1000.0] if _video != null else "sin vídeo"
	_debug_l.text = "%d fps · %s" % [Engine.get_frames_per_second(), vtxt]


func _popup(grade: int, limb: int, pts: int) -> void:
	var col: Color = StepIcons.GRADE_COLORS.get(grade, Color.WHITE)
	var txt := str(GRADE_TEXT.get(grade, "?"))
	if pts > 0:
		txt += "\n+%d" % pts
	var l := _outlined(UiStyle.label(txt, 30, col))
	l.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	l.size = Vector2(200, 80)
	l.position = Vector2(_lane.column_x(limb) - 100.0, _lane.target_y() - 140.0)
	_popups.add_child(l)
	var tw := l.create_tween()
	tw.set_parallel(true)
	tw.tween_property(l, "position:y", l.position.y - 60.0, 0.6)
	tw.tween_property(l, "modulate:a", 0.0, 0.45).set_delay(0.25)
	tw.chain().tween_callback(l.queue_free)


static func _outlined(l: Label) -> Label:
	l.add_theme_constant_override("outline_size", 6)
	l.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.85))
	return l


static func _set_rect(c: Control, left: float, top: float, right: float, bottom: float) -> void:
	c.set_anchor_and_offset(SIDE_LEFT, left, 0.0)
	c.set_anchor_and_offset(SIDE_TOP, top, 0.0)
	c.set_anchor_and_offset(SIDE_RIGHT, right, 0.0)
	c.set_anchor_and_offset(SIDE_BOTTOM, bottom, 0.0)


# ------------------------------------------------------------------ log de fps

func _open_fps_log() -> void:
	if not _session.fps_log:
		return
	_fps_file = FileAccess.open(FPS_LOG_PATH, FileAccess.WRITE)
	if _fps_file == null:
		push_warning("gameplay: no se pudo abrir %s" % FPS_LOG_PATH)
		return
	_fps_file.store_line("t_s,fps")
	_t0_mono = SongClock.now_mono_s()


func _log_fps(delta: float) -> void:
	if _fps_file == null or delta <= 0.0:
		return
	_fps_file.store_line("%.4f,%.2f" % [SongClock.now_mono_s() - _t0_mono, 1.0 / delta])
