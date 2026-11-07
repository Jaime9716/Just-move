# requires: SongClockLogic, Profile, EngineFactory
# Humo del autoload SongClock (agente D). MARCADO: depende de que el autoload esté en el árbol
# (en Godot 4.x lo está también con -s; si no, SKIP). Solo con el reloj parado: sin audio real.
# No guarda nada en disco: el perfil del jugador se restaura al terminar.
extends "res://tests/test_base.gd"

var _clock: Node = null
var _saved_profile: Variant = null
var _saved_offsets: Array = []


func before_each() -> void:
	_clock = null
	if tree != null:
		_clock = tree.root.get_node_or_null(^"SongClock")
	if _clock != null:
		_saved_profile = _clock.get("profile")
		_saved_offsets = [_clock.get("audio_offset"), _clock.get("visual_offset"), _clock.get("input_offset")]
		_clock.call("stop")


func after_each() -> void:
	if _clock == null:
		return
	# apply_profile cambia el perfil y los offsets: se devuelven ambos como estaban.
	if _saved_profile is Profile:
		_clock.call("apply_profile", _saved_profile)
	if _saved_offsets.size() == 3:
		_clock.set("audio_offset", _saved_offsets[0])
		_clock.set("visual_offset", _saved_offsets[1])
		_clock.set("input_offset", _saved_offsets[2])
	_clock.call("reset", 0.0)


func _test_profile() -> Profile:
	var p := Profile.new()
	p.audio_offset = 0.06
	p.visual_offset = 0.02
	p.input_offset = 0.035
	return p


func test_autoload_api_present() -> void:
	if _clock == null:
		skip("autoload SongClock no está en el árbol")
		return
	for m: String in ["play", "stop", "seek", "reset", "song_time", "audible", "visual", "engine_mapping",
			"input_to_song", "sample_to_song", "now_ns", "now_mono_s", "apply_profile", "is_playing"]:
		assert_has_method(_clock, m, "SongClock")
	assert_true(_clock.get("logic") is SongClockLogic, "SongClock.logic es SongClockLogic")
	done()


func test_offsets_from_profile() -> void:
	if _clock == null:
		skip("autoload SongClock no está en el árbol")
		return
	_clock.call("apply_profile", _test_profile())
	_clock.call("reset", 20.0)
	assert_near(float(_clock.call("song_time")), 20.0, 1e-12, "song_time")
	assert_near(float(_clock.call("audible")), 20.0 - 0.06, 1e-12, "audible = song_time − audio_offset")
	assert_near(float(_clock.call("visual")), 20.0 - 0.06 + 0.02, 1e-12, "visual = audible + visual_offset")
	done()


func test_input_to_song_stopped() -> void:
	if _clock == null:
		skip("autoload SongClock no está en el árbol")
		return
	_clock.call("apply_profile", _test_profile())
	_clock.call("reset", 30.0)
	var now := float(EngineFactory.now_ns()) * 1e-9
	var m := float(_clock.call("engine_mapping", now))
	assert_near(m, 30.0 - now - 0.035 - 0.06, 1e-9, "engine_mapping(now) con el reloj parado")
	# Parado, la canción está congelada: una muestra de "ahora" cae en audible − input_offset.
	var t_now := float(EngineFactory.now_ns()) * 1e-9
	var s := float(_clock.call("input_to_song", t_now))
	assert_near(s, 30.0 - 0.06 - 0.035, 0.002, "input_to_song(ahora)")
	var t_ns := EngineFactory.now_ns()
	var s2 := float(_clock.call("sample_to_song", t_ns))
	assert_near(s2, 30.0 - 0.06 - 0.035, 0.002, "sample_to_song(t_ns de ahora)")
	# Diferencias exactas: 1 s más tarde en el reloj = 1 s más tarde en la canción.
	var a := float(_clock.call("input_to_song", 1000.0))
	var b := float(_clock.call("input_to_song", 1001.0))
	assert_near(b - a, 1.0, 0.002, "input_to_song es lineal en el reloj")
	done()
