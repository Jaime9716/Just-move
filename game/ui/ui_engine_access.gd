# ui_engine_access.gd — acceso al motor desde la UI (agente E).
# Orden de preferencia para crear motor y fuentes:
#   1. game/core/engine_factory.gd (agente D: da JDEngine/JDSource reales o simulaciones),
#      como script con funciones estáticas, como clase instanciable o como autoload EngineFactory;
#   2. la GDExtension directamente (ClassDB), si está cargada;
#   3. simulaciones mínimas propias (UiFallbackEngine / UiFallbackSource), para que la UI no se rompa.
# Todas las llamadas son dinámicas: el script compila aunque falten la extensión o la fábrica.
class_name UiEngineAccess
extends RefCounted

const FACTORY_PATH := "res://core/engine_factory.gd"
const FACTORY_AUTOLOADS := ["EngineFactory", "JDEngineFactory", "JdEngineFactory"]

static var _checked := false
static var _factory_script: Script = null
static var _factory_node: Object = null
static var _factory_instance: Object = null
static var _now_callable: Callable = Callable()
static var _now_checked := false


static func has_extension() -> bool:
	return ClassDB.class_exists("JDEngine") and ClassDB.class_exists("JDSource")


## Texto para la interfaz.
static func describe() -> String:
	var parts := PackedStringArray()
	parts.append("extensión del motor cargada" if has_extension() else "SIN extensión del motor (simulación)")
	_probe()
	if _factory_node != null or _factory_script != null:
		parts.append("fábrica de D encontrada")
	else:
		parts.append("sin engine_factory.gd (simulación propia de la UI)")
	return ", ".join(parts)


static func create_engine() -> Object:
	var r := _factory_call(["create_engine", "make_engine", "new_engine", "engine"], [])
	if r["found"] and r["value"] is Object:
		return r["value"]
	if ClassDB.class_exists("JDEngine") and ClassDB.can_instantiate("JDEngine"):
		var e: Object = ClassDB.instantiate("JDEngine")
		if e != null:
			return e
	return UiFallbackEngine.new()


static func create_phone_server(port: int, pairing_code: int = 0) -> Object:
	var r := _factory_call(["create_phone_server", "make_phone_server", "phone_server"], [port, pairing_code])
	if r["found"] and r["value"] is Object:
		return r["value"]
	var v: Variant = _class_static("JDSource", "create_phone_server", [port, pairing_code])
	return v if v is Object else null


static func create_replay(path: String, speed: float = 1.0) -> Object:
	var r := _factory_call(["create_replay", "make_replay", "replay"], [path, speed])
	if r["found"] and r["value"] is Object:
		return r["value"]
	var v: Variant = _class_static("JDSource", "create_replay", [path, speed])
	return v if v is Object else null


## Fuente simulada (sin móviles ni sesión). Nunca devuelve null.
static func create_mock_source() -> Object:
	var r := _factory_call(["create_mock_source", "create_mock", "make_mock_source", "mock_source", "create_mock_phone"], [])
	if r["found"] and r["value"] is Object:
		return r["value"]
	return UiFallbackSource.new()


## Reloj monotónico oficial en segundos (JDEngine.now_ns si existe).
static func now_s() -> float:
	return float(now_ns()) * 1e-9


static func now_ns() -> int:
	if not _now_checked:
		_now_checked = true
		_probe()
		if _factory_node != null and _factory_node.has_method("now_ns"):
			_now_callable = Callable(_factory_node, "now_ns")
		elif _factory_script != null and _script_has(_factory_script, "now_ns"):
			_now_callable = Callable(_factory_script, "now_ns")
	if _now_callable.is_valid():
		var v: Variant = _now_callable.call()
		if v is int or v is float:
			return int(v)
	if ClassDB.class_exists("JDEngine"):
		var v2: Variant = _class_static("JDEngine", "now_ns", [])
		if v2 is int:
			return v2
	return Time.get_ticks_usec() * 1000


## ¿Es un objeto nativo de la extensión (no una simulación en GDScript)?
static func is_native(obj: Object, cls: String) -> bool:
	return obj != null and obj.get_script() == null and obj.get_class() == cls


# ------------------------------------------------------------------ internos

static func _probe() -> void:
	if _checked:
		return
	_checked = true
	var loop := Engine.get_main_loop()
	if loop is SceneTree:
		var root: Window = (loop as SceneTree).root
		for n: String in FACTORY_AUTOLOADS:
			var node := root.get_node_or_null(n)
			if node != null:
				_factory_node = node
				break
	if ResourceLoader.exists(FACTORY_PATH):
		var s: Variant = load(FACTORY_PATH)
		if s is Script:
			_factory_script = s


static func _script_has(s: Script, method: String) -> bool:
	for m: Dictionary in s.get_script_method_list():
		if String(m.get("name", "")) == method:
			return true
	return false


## Llama al primer método de `names` que tenga la fábrica. Devuelve {found: bool, value: Variant}.
static func _factory_call(names: Array, args: Array) -> Dictionary:
	_probe()
	if _factory_node != null:
		for wanted: String in names:
			if _factory_node.has_method(wanted):
				var argc := _factory_node.get_method_argument_count(wanted)
				return {"found": true, "value": _factory_node.callv(wanted, args.slice(0, mini(argc, args.size())))}
	if _factory_script == null:
		return {"found": false, "value": null}
	var methods: Array[Dictionary] = _factory_script.get_script_method_list()
	for wanted: String in names:
		for md: Dictionary in methods:
			if String(md.get("name", "")) != wanted:
				continue
			var nargs: int = (md.get("args", []) as Array).size()
			var ndef: int = (md.get("default_args", []) as Array).size()
			if args.size() < nargs - ndef:
				continue
			var call_args := args.slice(0, mini(nargs, args.size()))
			var flags: int = int(md.get("flags", 0))
			if flags & METHOD_FLAG_STATIC:
				return {"found": true, "value": _factory_script.callv(wanted, call_args)}
			var inst := _get_factory_instance()
			if inst != null and inst.has_method(wanted):
				return {"found": true, "value": inst.callv(wanted, call_args)}
			# Sin bandera de estático y sin instancia posible: se intenta como estático.
			return {"found": true, "value": _factory_script.callv(wanted, call_args)}
	return {"found": false, "value": null}


static func _get_factory_instance() -> Object:
	if _factory_instance == null and _factory_script != null and _factory_script.can_instantiate():
		var o: Variant = _factory_script.call("new")
		if o is Object:
			_factory_instance = o
	return _factory_instance


## Llama a un método estático de una clase de la extensión.
static func _class_static(cls: String, method: String, args: Array) -> Variant:
	if not ClassDB.class_exists(cls):
		return null
	if not ClassDB.class_has_method(cls, method):
		return null
	if ClassDB.has_method("class_call_static"):
		return ClassDB.callv("class_call_static", [cls, method] + args)
	# Plan B: los métodos estáticos se pueden llamar desde una instancia cualquiera.
	if ClassDB.can_instantiate(cls):
		var inst: Object = ClassDB.instantiate(cls)
		if inst != null:
			return inst.callv(method, args)
	return null
