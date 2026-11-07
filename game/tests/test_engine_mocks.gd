# requires: EngineFactory, MockJDEngine, MockJDSource
# Humo de MockJDEngine / MockJDSource / EngineFactory (agente D) frente a contracts/gdextension_api.md:
# misma interfaz que las clases nativas y funcionan sin la extensión. Los tests marcados "(motor de
# la fábrica)" se ejecutan con lo que devuelva EngineFactory: el nativo si la GDExtension está cargada
# (CI) o la simulación si no (docs/06: "cada simulación pasa los mismos tests que la real").
extends "res://tests/test_base.gd"

const SOURCE_METHODS := ["endpoint", "pairing_code", "start", "stop", "devices", "assign_limb",
		"set_recorder_mapping", "poll_samples", "last_error"]
const SOURCE_STATICS := ["create_phone_server", "create_replay", "create_recorder"]
const ENGINE_METHODS := ["now_ns", "load_song", "set_config", "set_time_mapping", "pump", "update", "reset"]
const DEVICE_KEYS := ["id", "name", "kind", "limb", "connected", "battery", "sync_rtt_ms"]
const SAMPLE_KEYS := ["t_ns", "id", "limb", "a", "g"]
const RESULT_KEYS := ["step", "limb", "grade", "similarity", "timing_error_ms"]

var _dir := ""
var _saved_force_mock := false


func before_each() -> void:
	_saved_force_mock = EngineFactory.force_mock
	_dir = tmp_dir("engine")


func after_each() -> void:
	EngineFactory.force_mock = _saved_force_mock
	remove_tree(_dir)


## Paquete mínimo válido (contracts/song_package.md) en user://; devuelve la ruta absoluta.
## 3 pasos: 1 extremidad, 2 extremidades, 1 pie → 4 resultados esperados.
func _make_song() -> String:
	var dir := _dir.path_join("song_a")
	DirAccess.make_dir_recursive_absolute(dir)
	var choreo := {
		"format": "jd-choreo", "version": 1,
		"song": {"title": "Test T2", "artist": "T2", "audio": "audio.ogg", "video": null,
				"bpm": 120.0, "offset_s": 0.0, "duration_s": 10.0},
		"steps": [
			{"t": 1.0, "dur": 0.25, "limbs": ["right_hand"], "icon": "punch_right", "kind": "accent"},
			{"t": 2.0, "dur": 1.0, "limbs": ["left_hand", "right_hand"], "icon": "wave", "kind": "move"},
			{"t": 4.0, "dur": 0.25, "limbs": ["left_foot"], "icon": "stomp_left", "kind": "accent"},
		],
		"references": {},
	}
	write_text(dir.path_join("choreo.json"), JSON.stringify(choreo, "  "))
	write_text(dir.path_join("audio.ogg"), "no es un ogg de verdad; el motor no lo decodifica")
	return ProjectSettings.globalize_path(dir)


func _check_methods(obj: Object, names: Array, what: String) -> void:
	for n: String in names:
		assert_has_method(obj, n, what)


func _check_keys(d: Variant, keys: Array, what: String) -> void:
	if not assert_type(d, TYPE_DICTIONARY, what):
		return
	var dd: Dictionary = d
	for k: String in keys:
		assert_has(dd, k, what)


# ------------------------------------------------------------------ interfaz

func test_mock_engine_interface() -> void:
	var e := MockJDEngine.new()
	_check_methods(e, ENGINE_METHODS, "MockJDEngine")
	done()


func test_mock_source_interface() -> void:
	var s := MockJDSource.create_phone_server(47800, 0)
	assert_not_null(s, "create_phone_server")
	_check_methods(s, SOURCE_METHODS, "MockJDSource")
	_check_methods(s, SOURCE_STATICS, "MockJDSource (estáticos)")
	done()


func test_factory_forced_mock() -> void:
	EngineFactory.force_mock = true
	assert_false(EngineFactory.has_extension(), "force_mock → has_extension false")
	var e: Object = EngineFactory.create_engine()
	assert_true(e is MockJDEngine, "create_engine → MockJDEngine")
	assert_true(EngineFactory.is_mock(e), "is_mock(engine)")
	var s: Object = EngineFactory.create_phone_server(47800, 4321)
	assert_true(s is MockJDSource, "create_phone_server → MockJDSource")
	assert_true(EngineFactory.is_mock(s), "is_mock(source)")
	assert_eq(int(s.call(&"pairing_code")), 4321, "código pedido")
	done()


func test_factory_matches_classdb() -> void:
	EngineFactory.force_mock = false
	var native := ClassDB.class_exists("JDEngine") and ClassDB.class_exists("JDSource")
	assert_eq(EngineFactory.has_extension(), native, "has_extension = ClassDB.class_exists")
	var e: Object = EngineFactory.create_engine()
	assert_not_null(e, "create_engine nunca null")
	assert_eq(EngineFactory.is_mock(e), not native, "motor simulado solo sin extensión")
	_check_methods(e, ENGINE_METHODS, "motor de la fábrica")
	var m: Object = EngineFactory.create_mock_source()
	assert_true(m is MockJDSource, "create_mock_source siempre es simulación")
	done()


func test_factory_clock() -> void:
	var a := EngineFactory.now_ns()
	var b := EngineFactory.now_ns()
	assert_type(a, TYPE_INT)
	assert_ge(float(b - a), 0.0, "now_ns no retrocede")
	assert_gt(float(a), 0.0, "now_ns > 0")
	OS.delay_msec(20)
	var c := EngineFactory.now_ns()
	assert_ge(float(c - b) * 1e-9, 0.015, "20 ms de espera: el reloj avanza ≥ 15 ms")
	assert_lt(float(c - b) * 1e-9, 1.0, "20 ms de espera: el reloj no salta")
	assert_near(EngineFactory.now_s(), float(EngineFactory.now_ns()) * 1e-9, 0.01, "now_s = now_ns·1e-9")
	assert_eq(MockJDEngine.now_ns() > 0, true, "MockJDEngine.now_ns estático")
	assert_true(EngineFactory.describe().length() > 0, "describe")
	done()


# ------------------------------------------------------------------ MockJDSource

func test_mock_phone_server() -> void:
	var s := MockJDSource.create_phone_server(47811, 1234)
	assert_eq(s.pairing_code(), 1234, "pairing_code")
	assert_true(s.endpoint().ends_with(":47811"), "endpoint \"IP:puerto\" (%s)" % s.endpoint())
	assert_eq(s.endpoint().split(":").size(), 2, "endpoint con un solo ':'")
	var r := MockJDSource.create_phone_server(47800, 0)
	assert_true(r.pairing_code() >= 1000 and r.pairing_code() <= 9999, "código aleatorio de 4 cifras")
	s.connect_delay_s = 0.0
	assert_true(s.start(), "start")
	assert_eq(s.last_error(), "")
	OS.delay_msec(60)
	var devs := s.devices()
	if assert_eq(devs.size(), 1, "un móvil simulado"):
		_check_keys(devs[0], DEVICE_KEYS, "devices()[0]")
		assert_true(bool(devs[0]["connected"]), "conectado")
		var b := float(devs[0]["battery"])
		assert_true(b >= 0.0 and b <= 1.0, "battery 0–1")
	var smp := s.poll_samples(512)
	assert_gt(float(smp.size()), 0.0, "tras 60 ms hay muestras (200 Hz)")
	assert_le(float(smp.size()), 512.0, "respeta max")
	var prev := -1
	var mono := true
	for x: Dictionary in smp:
		var t: int = int(x["t_ns"])
		if t <= prev:
			mono = false
		prev = t
	assert_true(mono, "t_ns estrictamente creciente")
	if smp.size() > 0:
		_check_keys(smp[0], SAMPLE_KEYS, "muestra")
		assert_type(smp[0]["a"], TYPE_VECTOR3, "a es Vector3")
		assert_type(smp[0]["g"], TYPE_VECTOR3, "g es Vector3")
		assert_type(smp[0]["t_ns"], TYPE_INT, "t_ns es int")
		var now := EngineFactory.now_ns()
		assert_le(absf(float(now - int(smp[smp.size() - 1]["t_ns"]))) * 1e-9, 0.1,
				"t_ns en el mismo reloj que EngineFactory.now_ns")
	s.assign_limb(1, 3)
	assert_eq(int(s.devices()[0]["limb"]), 3, "assign_limb cambia el limb del dispositivo")
	OS.delay_msec(20)
	var smp2 := s.poll_samples(512)
	if smp2.size() > 0:
		assert_eq(int(smp2[0]["limb"]), 3, "las muestras llevan el limb nuevo")
	assert_eq(s.poll_samples(3).size() <= 3, true, "poll_samples(3) ≤ 3")
	s.stop()
	OS.delay_msec(10)
	assert_eq(s.poll_samples(512).size(), 0, "parado → sin muestras")
	done()


func test_mock_replay_missing_is_null() -> void:
	assert_null(MockJDSource.create_replay(_dir.path_join("no_existe.jsonl")), "replay de archivo ausente → null")
	done()


func _write_session(path: String, n: int) -> void:
	var lines := PackedStringArray()
	lines.append(JSON.stringify({"format": "jd-session", "version": 1, "time_base": "song", "song_id": null,
			"recorded_at": "2026-11-07T00:00:00Z", "note": "T2",
			"devices": [{"source_id": 1, "name": "Pixel T2", "kind": "phone", "limb": "left_hand"}]}))
	lines.append("# comentario: se ignora")
	for i in range(n):
		lines.append(JSON.stringify({"t": 5.0 + float(i) * 0.005, "src": 1, "limb": "left_hand",
				"a": [0.1, 9.8, 0.2], "g": [0.0, 0.0, 0.01]}))
	write_text(path, "\n".join(lines) + "\n")


func test_mock_replay_emits_all() -> void:
	var path := _dir.path_join("s.jsonl")
	_write_session(path, 5)
	var s := MockJDSource.create_replay(path, 1.0)
	if not assert_not_null(s, "create_replay"):
		done()
		return
	assert_true(s.start(), "start (%s)" % s.last_error())
	OS.delay_msec(60)
	var smp := s.poll_samples(512)
	assert_eq(smp.size(), 5, "las 5 muestras (0–20 ms) ya han salido tras 60 ms")
	if smp.size() > 0:
		_check_keys(smp[0], SAMPLE_KEYS, "muestra de replay")
		assert_eq(int(smp[0]["limb"]), 0, "left_hand = 0")
		assert_eq(int(smp[0]["id"]), 1)
	var devs := s.devices()
	if assert_eq(devs.size(), 1, "dispositivos de la cabecera"):
		_check_keys(devs[0], DEVICE_KEYS, "dispositivo de replay")
		assert_eq(str(devs[0]["name"]), "Pixel T2")
	assert_eq(s.poll_samples(512).size(), 0, "no repite muestras")
	s.stop()
	done()


func test_mock_recorder_writes_session() -> void:
	var inner := MockJDSource.create_phone_server(47812, 1111)
	inner.connect_delay_s = 0.0
	var out := _dir.path_join("rec.jsonl")
	var rec := MockJDSource.create_recorder(inner, out)
	if not assert_not_null(rec, "create_recorder"):
		done()
		return
	rec.set_recorder_mapping(-100.0)
	assert_true(rec.start(), "start del grabador")
	OS.delay_msec(50)
	var got := rec.poll_samples(512)
	rec.stop()
	assert_gt(float(got.size()), 0.0, "el grabador deja pasar las muestras")
	var text := FileAccess.get_file_as_string(out)
	var lines := text.strip_edges().split("\n")
	assert_eq(lines.size(), got.size() + 1, "cabecera + una línea por muestra")
	var head: Variant = JSON.parse_string(lines[0])
	if assert_type(head, TYPE_DICTIONARY, "cabecera JSON"):
		var hd: Dictionary = head
		assert_eq(str(hd.get("format", "")), "jd-session", "format")
	if lines.size() > 1:
		var first: Variant = JSON.parse_string(lines[1])
		if assert_type(first, TYPE_DICTIONARY, "muestra JSON"):
			var fd: Dictionary = first
			for k: String in ["t", "src", "limb", "a", "g"]:
				assert_has(fd, k, "línea de muestra")
	assert_null(MockJDSource.create_recorder(null, out), "sin fuente interior → null")
	done()


# ------------------------------------------------------------------ motor (simulado y de la fábrica)

func _engine_smoke(e: Object, what: String) -> void:
	var bad: String = str(e.call(&"load_song", _dir.path_join("no_existe")))
	assert_ne(bad, "", what + ": load_song de carpeta inexistente da error")
	var dir := _make_song()
	var err: String = str(e.call(&"load_song", dir))
	assert_eq(err, "", what + ": load_song del paquete de prueba")
	e.call(&"set_config", 50.0, 100.0, 1.5)
	e.call(&"set_time_mapping", 0.0)
	var r0: Variant = e.call(&"update", 0.5)
	if assert_type(r0, TYPE_ARRAY, what + ": update devuelve Array"):
		var a0: Array = r0
		assert_eq(a0.size(), 0, what + ": nada que juzgar antes del primer paso")
	# Sin muestras (quieto): todos los pasos acaban en Miss (docs/06, B5: quieto → Miss).
	var res: Array = e.call(&"update", 1000.0)
	assert_eq(res.size(), 4, what + ": un resultado por extremidad de cada paso")
	var steps_seen := {}
	for r: Variant in res:
		_check_keys(r, RESULT_KEYS, what + ": resultado")
		if typeof(r) != TYPE_DICTIONARY:
			continue
		var d: Dictionary = r
		var g := int(d.get("grade", -1))
		assert_true(g >= 0 and g <= 2, what + ": grade 0..2")
		assert_eq(g, 0, what + ": quieto → Miss")
		steps_seen[int(d.get("step", -1))] = true
	assert_eq(steps_seen.size(), 3, what + ": resultados de los 3 pasos")
	var again: Array = e.call(&"update", 2000.0)
	assert_eq(again.size(), 0, what + ": no repite resultados")
	e.call(&"reset")
	var after_reset: Array = e.call(&"update", 1000.0)
	assert_eq(after_reset.size(), 4, what + ": reset vuelve al inicio de la canción")


func test_mock_engine_smoke() -> void:
	_engine_smoke(MockJDEngine.new(), "MockJDEngine")
	done()


func test_factory_engine_smoke() -> void:
	EngineFactory.force_mock = false
	var e: Object = EngineFactory.create_engine()
	_engine_smoke(e, "motor de la fábrica (%s)" % ("simulado" if EngineFactory.is_mock(e) else "nativo"))
	done()


func test_mock_engine_pump_mock_source() -> void:
	var e := MockJDEngine.new()
	var s := MockJDSource.create_phone_server(47813, 0)
	s.connect_delay_s = 0.0
	s.start()
	OS.delay_msec(40)
	var n := e.pump(s)
	assert_gt(float(n), 0.0, "pump devuelve cuántas muestras leyó")
	assert_eq(s.poll_samples(512).size() <= 2, true, "pump vació la fuente")
	assert_eq(e.pump(null), 0, "pump(null) → 0")
	s.stop()
	done()


func test_mock_engine_load_song_errors_have_json_path() -> void:
	var dir := _dir.path_join("bad_song")
	DirAccess.make_dir_recursive_absolute(dir)
	write_text(dir.path_join("choreo.json"), JSON.stringify({"format": "jd-choreo", "version": 1,
			"song": {"title": "x", "audio": "audio.ogg", "duration_s": 5.0},
			"steps": [{"t": 2.0, "dur": 0.2, "limbs": ["right_hand"]}, {"t": 1.0, "dur": 0.2, "limbs": ["right_hand"]}]}))
	var err := MockJDEngine.new().load_song(dir)
	assert_ne(err, "", "pasos desordenados → error")
	assert_true(err.contains("steps[1]"), "el error lleva la ruta JSON (%s)" % err)
	done()
