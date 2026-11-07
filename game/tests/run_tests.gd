# run_tests.gd — runner de tests de GDScript sin dependencias (agente T2).
#
#   godot --headless --path game -s res://tests/run_tests.gd [-- --filter=texto] [-- --verbose]
#
# - Descubre res://tests/test_*.gd (salvo test_base.gd), instancia cada uno y ejecuta sus métodos
#   test_* (en orden alfabético) y después, uno a uno, los step_test_* (uno por fotograma, con límite).
# - Una línea "# requires: ClaseA, ClaseB" en el archivo: si falta alguna clase global (class_name o
#   clase nativa) el archivo entero sale como SKIP en vez de romper.
# - Un archivo que no compila, o un test que no llega a done() (error de script), cuenta como FALLO.
# - Sale con quit(0) si todo pasa y quit(1) si algo falla. Sin await: todo se mueve con _process y
#   hay un límite global de fotogramas, así que no se cuelga.
# - No referencia ninguna clase del juego: compila aunque falten todas.
extends SceneTree

const TESTS_DIR := "res://tests"
const BASE_FILE := "test_base.gd"
const MAX_TOTAL_FRAMES := 100000
## Límite global de tiempo real (s): el runner nunca se queda colgado.
const MAX_TOTAL_S := 300.0
## fps máximos mientras corren los tests (los tests por fotogramas miden tiempo real).
const TEST_MAX_FPS := 120

var _phase := 0
var _frame := 0
var _filter := ""
var _verbose := false
var _globals := {}

var _passed := 0
var _failed := 0
var _skipped := 0
var _failures := PackedStringArray()

var _stepped: Array = []        # [{inst, method, label}]
var _cur: Dictionary = {}       # test por fotogramas en curso: {inst, method, label, frame, max}
var _finished := false
var _t0_ms := 0


func _initialize() -> void:
	_t0_ms = Time.get_ticks_msec()
	for a: String in OS.get_cmdline_user_args():
		if a.begins_with("--filter="):
			_filter = a.substr("--filter=".length())
		elif a == "--verbose":
			_verbose = true
	if Engine.max_fps == 0:
		Engine.max_fps = TEST_MAX_FPS
	print("== Tests de GDScript (T2) · Godot %s ==" % Engine.get_version_info().get("string", "?"))
	if _filter != "":
		print("   filtro: \"%s\"" % _filter)


func _process(_delta: float) -> bool:
	if _finished:
		return false
	_frame += 1
	if _frame > MAX_TOTAL_FRAMES or float(Time.get_ticks_msec() - _t0_ms) / 1000.0 > MAX_TOTAL_S:
		_record_fail("runner", "límite global (%d fotogramas o %.0f s) superado: ¿test colgado?" % [MAX_TOTAL_FRAMES, MAX_TOTAL_S])
		_cur = {}
		_stepped.clear()
		_finish()
		return false
	match _phase:
		0:
			# Un fotograma de espera: los autoloads (si los hay) ya están en el árbol y listos.
			_phase = 1
		1:
			_run_files()
			_phase = 2
		2:
			_tick_stepped()
			if _cur.is_empty() and _stepped.is_empty():
				_finish()
	return false


# ------------------------------------------------------------------ descubrimiento y tests normales

func _run_files() -> void:
	for c: Dictionary in ProjectSettings.get_global_class_list():
		_globals[str(c.get("class", ""))] = true
	if _globals.is_empty():
		_record_fail("runner", "no hay ninguna clase global registrada: importa el proyecto antes " +
				"(godot --headless --path game --import) para generar .godot/global_script_class_cache.cfg")
		return
	var da := DirAccess.open(TESTS_DIR)
	if da == null:
		_record_fail("runner", "no se puede abrir %s" % TESTS_DIR)
		return
	var names: Array[String] = []
	for f: String in da.get_files():
		if f.begins_with("test_") and f.ends_with(".gd") and f != BASE_FILE:
			names.append(f)
	names.sort()
	if names.is_empty():
		_record_fail("runner", "no se encontró ningún test_*.gd en %s" % TESTS_DIR)
		return
	for f: String in names:
		_run_file(TESTS_DIR.path_join(f))


func _run_file(path: String) -> void:
	var file := path.get_file()
	var missing := _missing_requirements(path)
	if not missing.is_empty():
		_skipped += 1
		print("SKIP %s  (faltan clases: %s)" % [file, ", ".join(missing)])
		return
	var res: Resource = load(path)
	var scr := res as GDScript
	if scr == null or not scr.can_instantiate():
		_record_fail(file, "no compila o no se puede instanciar (mira los errores de arriba)")
		return
	var obj: Variant = scr.new()
	if not (obj is Object) or not (obj as Object).has_method("_t2_begin"):
		_record_fail(file, "no hereda de res://tests/test_base.gd")
		return
	var inst: Object = obj
	inst.set("tree", self)

	var tests: Array[String] = []
	var steps: Array[String] = []
	var seen := {}
	for m: Dictionary in inst.get_method_list():
		var n := str(m.get("name", ""))
		if seen.has(n):
			continue
		seen[n] = true
		if n.begins_with("test_"):
			tests.append(n)
		elif n.begins_with("step_test_"):
			steps.append(n)
	tests.sort()
	steps.sort()
	if tests.is_empty() and steps.is_empty():
		print("AVISO %s: no tiene métodos test_* ni step_test_*" % file)

	for n: String in tests:
		var label := "%s::%s" % [file, n]
		if not _matches(label):
			continue
		inst.call("_t2_begin", label)
		inst.call("before_each")
		inst.call(n)
		var r: Variant = inst.call("_t2_end")
		inst.call("after_each")
		_report(label, r)
	for n: String in steps:
		var label := "%s::%s" % [file, n]
		if _matches(label):
			_stepped.append({"inst": inst, "method": n, "label": label})


func _missing_requirements(path: String) -> PackedStringArray:
	var missing := PackedStringArray()
	var text := FileAccess.get_file_as_string(path)
	for line: String in text.split("\n"):
		var l := line.strip_edges()
		if not l.begins_with("# requires:"):
			continue
		for part: String in l.substr("# requires:".length()).split(","):
			var cls := part.strip_edges()
			if cls.is_empty():
				continue
			if not _globals.has(cls) and not ClassDB.class_exists(cls):
				missing.append(cls)
	return missing


func _matches(label: String) -> bool:
	return _filter.is_empty() or label.contains(_filter)


# ------------------------------------------------------------------ tests por fotogramas

func _tick_stepped() -> void:
	if _cur.is_empty():
		if _stepped.is_empty():
			return
		_cur = _stepped.pop_front()
		var inst0: Object = _cur["inst"]
		_cur["frame"] = 0
		var mf: Variant = inst0.call("max_frames")
		_cur["max"] = int(mf) if (typeof(mf) == TYPE_INT or typeof(mf) == TYPE_FLOAT) else 600
		inst0.call("_t2_begin", str(_cur["label"]))
		inst0.call("before_each")
	var inst: Object = _cur["inst"]
	var label := str(_cur["label"])
	var frame: int = int(_cur["frame"])
	var ret: Variant = inst.call(str(_cur["method"]), frame)
	_cur["frame"] = frame + 1
	var finished := false
	if typeof(ret) != TYPE_BOOL:
		# null = el método se cortó por un error de script (o no devuelve bool).
		finished = true
	elif bool(ret):
		finished = true
	elif frame + 1 >= int(_cur["max"]):
		inst.call("fail", "no terminó en %d fotogramas" % int(_cur["max"]))
		inst.call("done")
		finished = true
	if finished:
		var r: Variant = inst.call("_t2_end")
		inst.call("after_each")
		_report(label, r)
		_cur = {}


# ------------------------------------------------------------------ resultados

func _report(label: String, r: Variant) -> void:
	if typeof(r) != TYPE_DICTIONARY:
		_record_fail(label, "el runner no pudo leer el resultado")
		return
	var d: Dictionary = r
	var fails: PackedStringArray = d.get("fails", PackedStringArray())
	if bool(d.get("skipped", false)) and fails.is_empty():
		_skipped += 1
		print("SKIP %s  (%s)" % [label, str(d.get("skip_reason", ""))])
		return
	if not bool(d.get("done", false)):
		fails.append("no llegó a done(): probablemente un error de script en tiempo de ejecución (mira arriba)")
	if fails.is_empty():
		_passed += 1
		if _verbose:
			print("ok   %s  (%d asserts)" % [label, int(d.get("asserts", 0))])
		return
	_failed += 1
	print("FAIL %s" % label)
	for f: String in fails:
		print("       - " + f)
		_failures.append("%s: %s" % [label, f])


func _record_fail(label: String, msg: String) -> void:
	_failed += 1
	print("FAIL %s: %s" % [label, msg])
	_failures.append("%s: %s" % [label, msg])


func _finish() -> void:
	_finished = true
	# Limpieza de los archivos temporales de los tests.
	_remove_tree("user://t2_tests")
	var secs := float(Time.get_ticks_msec() - _t0_ms) / 1000.0
	print("")
	print("== Resultado: %d ok, %d fallidos, %d omitidos (%.1f s) ==" % [_passed, _failed, _skipped, secs])
	if _failed > 0:
		print("Fallos:")
		for f: String in _failures:
			print("  - " + f)
		quit(1)
	else:
		quit(0)


func _remove_tree(path: String) -> void:
	if not DirAccess.dir_exists_absolute(path):
		return
	var da := DirAccess.open(path)
	if da == null:
		return
	for f: String in da.get_files():
		DirAccess.remove_absolute(path.path_join(f))
	for d: String in da.get_directories():
		_remove_tree(path.path_join(d))
	DirAccess.remove_absolute(path)
