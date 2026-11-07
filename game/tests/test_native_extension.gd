# Interfaz de la GDExtension (contracts/gdextension_api.md) vista desde ClassDB. Sin dependencias:
# si la extensión no está cargada (aquí o en un PC sin compilar jd_godot) los tests salen como SKIP.
extends "res://tests/test_base.gd"

const SOURCE_METHODS := ["create_phone_server", "endpoint", "pairing_code", "create_replay", "create_recorder",
		"start", "stop", "devices", "assign_limb", "set_recorder_mapping", "poll_samples", "last_error"]
const ENGINE_METHODS := ["now_ns", "load_song", "set_config", "set_time_mapping", "pump", "update", "reset"]


func _loaded() -> bool:
	return ClassDB.class_exists("JDEngine") and ClassDB.class_exists("JDSource")


func test_native_classes_methods() -> void:
	if not _loaded():
		skip("GDExtension no cargada (JDEngine/JDSource no existen en ClassDB)")
		return
	for m: String in SOURCE_METHODS:
		assert_true(ClassDB.class_has_method("JDSource", m), "JDSource." + m)
	for m: String in ENGINE_METHODS:
		assert_true(ClassDB.class_has_method("JDEngine", m), "JDEngine." + m)
	assert_true(ClassDB.is_parent_class("JDEngine", "RefCounted"), "JDEngine es RefCounted")
	assert_true(ClassDB.is_parent_class("JDSource", "RefCounted"), "JDSource es RefCounted")
	done()


func test_native_statics() -> void:
	if not _loaded():
		skip("GDExtension no cargada")
		return
	if not ClassDB.has_method("class_call_static"):
		skip("ClassDB.class_call_static no existe en esta versión de Godot")
		return
	var now: Variant = ClassDB.callv("class_call_static", ["JDEngine", "now_ns"])
	assert_type(now, TYPE_INT, "JDEngine.now_ns() es int")
	var rep: Variant = ClassDB.callv("class_call_static", ["JDSource", "create_replay", "user://t2_tests/no_existe.jsonl", 1.0])
	assert_true(typeof(rep) == TYPE_NIL or (typeof(rep) == TYPE_OBJECT and rep == null),
			"create_replay de archivo ausente → null")
	done()


func test_mock_has_native_methods() -> void:
	# Lo que la simulación ofrece debe ser al menos lo que ofrece el nativo (mismo contrato).
	if not _loaded():
		skip("GDExtension no cargada")
		return
	var mock_engine := load("res://core/mock_engine.gd") as GDScript
	var mock_source := load("res://core/mock_source.gd") as GDScript
	if mock_engine == null or mock_source == null or not mock_engine.can_instantiate() or not mock_source.can_instantiate():
		fail("no se pueden cargar las simulaciones de game/core")
		done()
		return
	var e: Object = mock_engine.new()
	var s: Object = mock_source.new()
	for m: String in ENGINE_METHODS:
		assert_true(e.has_method(m), "MockJDEngine." + m)
	for m: String in SOURCE_METHODS:
		assert_true(s.has_method(m), "MockJDSource." + m)
	done()
