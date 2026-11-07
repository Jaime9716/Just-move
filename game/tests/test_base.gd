# test_base.gd — base de los tests de GDScript (agente T2). Sin dependencias (ni GUT ni clases del juego).
#
# Uso en un archivo game/tests/test_<algo>.gd:
#
#   # requires: SongClockLogic, Profile      ← clases globales necesarias; si falta alguna → SKIP
#   extends "res://tests/test_base.gd"
#
#   func test_algo() -> void:
#       assert_eq(1 + 1, 2)
#       done()                                  ← OBLIGATORIO al final: así el runner sabe que el test
#                                                  no se cortó por un error de script en tiempo de ejecución
#
# - `skip("motivo")` + `return` para omitir un test en tiempo de ejecución (p. ej. sin autoload).
# - Tests por fotogramas: `func step_test_<algo>(frame: int) -> bool` se llama una vez por fotograma
#   (frame = 0, 1, 2…) hasta que devuelve true (y antes llama a done()). Límite: max_frames().
# - `before_each()` / `after_each()` opcionales (after_each también tras los tests por fotogramas).
# - `tree` = el SceneTree del runner.
extends RefCounted

const TMP_ROOT := "user://t2_tests"

var tree: SceneTree = null

var _label := ""
var _fails := PackedStringArray()
var _asserts := 0
var _done := false
var _skipped := false
var _skip_reason := ""


# ------------------------------------------------------------------ ganchos (se pueden redefinir)

func before_each() -> void:
	pass


func after_each() -> void:
	pass


## Fotogramas máximos de cada step_test_* antes de darlo por colgado.
func max_frames() -> int:
	return 600


# ------------------------------------------------------------------ protocolo con el runner

func _t2_begin(label: String) -> void:
	_label = label
	_fails = PackedStringArray()
	_asserts = 0
	_done = false
	_skipped = false
	_skip_reason = ""


func _t2_end() -> Dictionary:
	return {
		"done": _done,
		"skipped": _skipped,
		"skip_reason": _skip_reason,
		"fails": _fails,
		"asserts": _asserts,
	}


# ------------------------------------------------------------------ control

## Marca el final normal del test. Sin esto el runner lo da por fallido.
func done() -> void:
	_done = true


func skip(reason: String) -> void:
	_skipped = true
	_skip_reason = reason
	_done = true


func fail(msg: String) -> void:
	_asserts += 1
	_fails.append(msg)


# ------------------------------------------------------------------ asserts (devuelven si se cumplió)

func assert_true(cond: bool, msg: String = "") -> bool:
	_asserts += 1
	if not cond:
		_fails.append(_m("se esperaba true", msg))
	return cond


func assert_false(cond: bool, msg: String = "") -> bool:
	_asserts += 1
	if cond:
		_fails.append(_m("se esperaba false", msg))
	return not cond


## Igualdad. int y float se comparan como números; tipos distintos (no numéricos) = distintos.
func assert_eq(got: Variant, expected: Variant, msg: String = "") -> bool:
	_asserts += 1
	var ok := values_equal(got, expected)
	if not ok:
		_fails.append(_m("obtenido %s, esperado %s" % [_s(got), _s(expected)], msg))
	return ok


func assert_ne(got: Variant, not_expected: Variant, msg: String = "") -> bool:
	_asserts += 1
	var ok := not values_equal(got, not_expected)
	if not ok:
		_fails.append(_m("obtenido %s, se esperaba otro valor" % _s(got), msg))
	return ok


## |got − expected| ≤ tol.
func assert_near(got: float, expected: float, tol: float, msg: String = "") -> bool:
	_asserts += 1
	var ok := absf(got - expected) <= tol
	if not ok:
		_fails.append(_m("obtenido %.9f, esperado %.9f ± %.9f (dif %.9f)" % [got, expected, tol, got - expected], msg))
	return ok


func assert_lt(got: float, limit: float, msg: String = "") -> bool:
	_asserts += 1
	var ok := got < limit
	if not ok:
		_fails.append(_m("obtenido %.9f, se esperaba < %.9f" % [got, limit], msg))
	return ok


func assert_le(got: float, limit: float, msg: String = "") -> bool:
	_asserts += 1
	var ok := got <= limit
	if not ok:
		_fails.append(_m("obtenido %.9f, se esperaba <= %.9f" % [got, limit], msg))
	return ok


func assert_gt(got: float, limit: float, msg: String = "") -> bool:
	_asserts += 1
	var ok := got > limit
	if not ok:
		_fails.append(_m("obtenido %.9f, se esperaba > %.9f" % [got, limit], msg))
	return ok


func assert_ge(got: float, limit: float, msg: String = "") -> bool:
	_asserts += 1
	var ok := got >= limit
	if not ok:
		_fails.append(_m("obtenido %.9f, se esperaba >= %.9f" % [got, limit], msg))
	return ok


func assert_null(v: Variant, msg: String = "") -> bool:
	_asserts += 1
	var ok := typeof(v) == TYPE_NIL
	if not ok:
		_fails.append(_m("se esperaba null, obtenido %s" % _s(v), msg))
	return ok


func assert_not_null(v: Variant, msg: String = "") -> bool:
	_asserts += 1
	var ok := typeof(v) != TYPE_NIL
	if not ok:
		_fails.append(_m("se esperaba un valor no null", msg))
	return ok


## typeof(v) == type (TYPE_INT, TYPE_DICTIONARY…).
func assert_type(v: Variant, expected_type: int, msg: String = "") -> bool:
	_asserts += 1
	var ok := typeof(v) == expected_type
	if not ok:
		_fails.append(_m("tipo %s, esperado %s" % [type_string(typeof(v)), type_string(expected_type)], msg))
	return ok


func assert_has(d: Dictionary, key: Variant, msg: String = "") -> bool:
	_asserts += 1
	var ok := d.has(key)
	if not ok:
		_fails.append(_m("falta la clave %s en %s" % [_s(key), _s(d)], msg))
	return ok


func assert_has_method(obj: Object, method: String, msg: String = "") -> bool:
	_asserts += 1
	var ok := obj != null and obj.has_method(method)
	if not ok:
		_fails.append(_m("el objeto no tiene el método %s" % method, msg))
	return ok


# ------------------------------------------------------------------ utilidades

static func values_equal(a: Variant, b: Variant) -> bool:
	var ta := typeof(a)
	var tb := typeof(b)
	var na := ta == TYPE_INT or ta == TYPE_FLOAT
	var nb := tb == TYPE_INT or tb == TYPE_FLOAT
	if na and nb:
		return float(a) == float(b)
	if ta != tb:
		return false
	return a == b


## Carpeta temporal de los tests (user://t2_tests/<sub>), vacía y creada.
func tmp_dir(sub: String) -> String:
	var path := TMP_ROOT.path_join(sub)
	remove_tree(path)
	DirAccess.make_dir_recursive_absolute(path)
	return path


## Borra una carpeta (user://…) con todo su contenido. Solo dentro de TMP_ROOT, por seguridad.
static func remove_tree(path: String) -> void:
	if not path.begins_with(TMP_ROOT):
		return
	if not DirAccess.dir_exists_absolute(path):
		return
	var da := DirAccess.open(path)
	if da == null:
		return
	for f: String in da.get_files():
		DirAccess.remove_absolute(path.path_join(f))
	for d: String in da.get_directories():
		remove_tree(path.path_join(d))
	DirAccess.remove_absolute(path)


static func write_text(path: String, text: String) -> bool:
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		return false
	f.store_string(text)
	f.close()
	return true


func _m(what: String, msg: String) -> String:
	if msg.is_empty():
		return what
	return "%s: %s" % [msg, what]


static func _s(v: Variant) -> String:
	if typeof(v) == TYPE_STRING:
		return "\"%s\"" % v
	if typeof(v) == TYPE_FLOAT:
		return "%.9f" % float(v)
	return "%s(%s)" % [str(v), type_string(typeof(v))]
