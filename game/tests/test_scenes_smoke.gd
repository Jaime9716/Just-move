# requires: UiSession, SongLibrary, EngineFactory, MockJDEngine, ScoreLogic
# Prueba de HUMO de escenas (agente E): instancia main_menu y gameplay con el motor simulado y la
# canción de prueba data/songs/demo_metronomo, avanza fotogramas y comprueba que siguen vivas y que la
# partida no falla. MARCADA: depende del autoload SongClock (si no está en el árbol → SKIP) y de que
# exista data/songs/demo_metronomo (si no → SKIP). No puede capturar los "ERROR:" del registro de
# Godot 4.4: revisa también la salida.
extends "res://tests/test_base.gd"

const MAIN_MENU := "res://scenes/main_menu.tscn"
const GAMEPLAY := "res://scenes/gameplay.tscn"
const DEMO_ID := "demo_metronomo"
# gameplay.gd: enum State { LOADING, COUNTDOWN, PLAYING, FINISHED, FAILED }
const STATE_COUNTDOWN := 1
const STATE_PLAYING := 2
const STATE_FINISHED := 3
const STATE_FAILED := 4
## La cuenta atrás de gameplay dura 2 s; se espera hasta 3,5 s de tiempo real.
const GAMEPLAY_WAIT_MS := 3500
const MENU_FRAMES := 30

var _node: Node = null
var _t0 := 0
var _active := false
var _saved_force := false
var _saw_playing := false


func max_frames() -> int:
	return 2000


func before_each() -> void:
	_node = null
	_active = false
	_saw_playing = false


func after_each() -> void:
	_cleanup()


func _start_common() -> bool:
	if tree == null:
		fail("el runner no pasó el SceneTree")
		done()
		return false
	if tree.root.get_node_or_null(^"SongClock") == null:
		skip("el autoload SongClock no está en el árbol (las escenas lo usan directamente)")
		return false
	_saved_force = EngineFactory.force_mock
	EngineFactory.force_mock = true
	_t0 = Time.get_ticks_msec()
	_active = true
	return true


func _cleanup() -> void:
	if not _active:
		return
	_active = false
	if is_instance_valid(_node):
		_node.queue_free()
	_node = null
	# Si gameplay terminó, habrá cambiado a la escena de resultados: también se quita.
	if tree != null and tree.current_scene != null:
		tree.current_scene.queue_free()
	if tree != null:
		var clock: Node = tree.root.get_node_or_null(^"SongClock")
		if clock != null and clock.has_method("stop"):
			clock.call("stop")
		var session: Node = tree.root.get_node_or_null(^"UiSession")
		if session != null:
			session.call("shutdown")
			session.queue_free()
	EngineFactory.force_mock = _saved_force


func _instance(path: String) -> bool:
	var packed := load(path) as PackedScene
	if not assert_not_null(packed, "cargar " + path):
		return false
	_node = packed.instantiate()
	if not assert_not_null(_node, "instanciar " + path):
		return false
	tree.root.add_child(_node)
	return true


func _demo_dir() -> String:
	return SongLibrary.default_extra_dir().path_join(DEMO_ID)


# ------------------------------------------------------------------ main_menu

func step_test_main_menu(frame: int) -> bool:
	if frame == 0:
		if not _start_common():
			return true
		if not _instance(MAIN_MENU):
			done()
			return true
		return false
	if not is_instance_valid(_node) or not _node.is_inside_tree():
		fail("main_menu salió del árbol sin que nadie lo pidiera")
		done()
		return true
	if frame < MENU_FRAMES:
		return false
	var songs: Variant = _node.get("_songs")
	if assert_type(songs, TYPE_ARRAY, "main_menu._songs"):
		var arr: Array = songs
		if FileAccess.file_exists(_demo_dir().path_join("choreo.json")):
			var found := false
			for p: Variant in arr:
				if typeof(p) == TYPE_DICTIONARY and str((p as Dictionary).get("id", "")) == DEMO_ID:
					found = true
					assert_true(bool((p as Dictionary).get("ok", false)), "la demo es válida")
			assert_true(found, "main_menu lista la canción de prueba " + DEMO_ID)
	var status: Variant = _node.get("_status")
	if status is Label:
		assert_true((status as Label).text.contains("SIMULADO"), "la barra de estado dice motor SIMULADO (%s)" % (status as Label).text)
	done()
	return true


# ------------------------------------------------------------------ gameplay

func step_test_gameplay(frame: int) -> bool:
	if frame == 0:
		if not _start_common():
			return true
		var dir := _demo_dir()
		if not FileAccess.file_exists(dir.path_join("choreo.json")):
			skip("no existe %s" % dir)
			return true
		var pkg := SongLibrary.load_package(dir)
		if not assert_true(bool(pkg.get("ok", false)), "demo válida (%s)" % str(pkg.get("errors", []))):
			done()
			return true
		var session := UiSession.get_state()
		session.replay_choice = UiSession.REPLAY_NONE
		session.song = pkg
		if not _instance(GAMEPLAY):
			done()
			return true
		return false

	if not is_instance_valid(_node):
		# gameplay solo se va sola al terminar (cambia a results); eso vale si llegó a jugar.
		assert_true(_saw_playing, "gameplay se cerró sin haber llegado a PLAYING")
		done()
		return true

	var st := int(_node.get("_state"))
	if st == STATE_FAILED:
		var msg := ""
		var center: Variant = _node.get("_center_l")
		if center is Label:
			msg = (center as Label).text
		fail("la partida pasó a FAILED: " + msg)
		done()
		return true
	if st == STATE_PLAYING or st == STATE_FINISHED:
		_saw_playing = true
	if frame == 1:
		var engine: Variant = _node.get("_engine")
		assert_true(engine is MockJDEngine, "gameplay usa el motor simulado (force_mock)")
		var source: Variant = _node.get("_source")
		assert_true(source is Object and source != null, "gameplay tiene fuente de entrada")
		assert_true(st == STATE_COUNTDOWN or st == STATE_PLAYING, "tras _ready: cuenta atrás (estado %d)" % st)
	if Time.get_ticks_msec() - _t0 < GAMEPLAY_WAIT_MS:
		return false
	assert_true(_saw_playing, "tras la cuenta atrás de 2 s la partida está en PLAYING (estado %d)" % st)
	var score: Variant = _node.get("_score")
	assert_true(score is ScoreLogic, "gameplay tiene ScoreLogic")
	done()
	return true
