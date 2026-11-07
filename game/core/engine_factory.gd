# engine_factory.gd — crea el motor y las fuentes (B2/B9, agente D).
# Si la GDExtension está cargada (ClassDB.class_exists("JDEngine")) devuelve las clases nativas
# JDEngine/JDSource (contracts/gdextension_api.md); si no, las simulaciones MockJDEngine/MockJDSource.
# Todas las llamadas a la extensión son dinámicas (ClassDB), así que este script compila sin ella.
#
# Uso: todo son funciones estáticas → EngineFactory.create_engine(), EngineFactory.now_ns(), ...
class_name EngineFactory
extends RefCounted

const ENGINE_CLASS := "JDEngine"
const SOURCE_CLASS := "JDSource"
const DEFAULT_PORT := 47800

## Forzar simulaciones aunque la extensión esté cargada (depuración). No afecta al reloj.
static var force_mock := false

static var _clock_ready := false
static var _clock_mode := 0            # 0 = Time.get_ticks_usec, 1 = instancia de JDEngine, 2 = ClassDB.class_call_static
static var _clock_holder: Object = null
static var _clock_callable := Callable()


## ¿Está cargada la GDExtension (y no se fuerzan simulaciones)?
static func has_extension() -> bool:
	return not force_mock and ClassDB.class_exists(ENGINE_CLASS) and ClassDB.class_exists(SOURCE_CLASS)


## ¿Es una simulación en GDScript (no un objeto nativo)?
static func is_mock(obj: Object) -> bool:
	return obj is MockJDEngine or obj is MockJDSource


## Texto para mostrar en pantalla o en el registro.
static func describe() -> String:
	var what := "motor nativo (GDExtension)" if has_extension() else "motor SIMULADO (sin GDExtension)"
	return "%s · reloj: %s" % [what, clock_description()]


# ------------------------------------------------------------------ creación

static func create_engine() -> Object:
	if has_extension() and ClassDB.can_instantiate(ENGINE_CLASS):
		var e: Object = ClassDB.instantiate(ENGINE_CLASS)
		if e != null:
			return e
	return MockJDEngine.new()


## Servidor de móviles-mando. pairing_code 0 = aleatorio. Hay que llamar a start().
static func create_phone_server(port: int = DEFAULT_PORT, pairing_code: int = 0) -> Object:
	if has_extension():
		var v: Variant = call_static(SOURCE_CLASS, "create_phone_server", [port, pairing_code])
		if v is Object and v != null:
			return v
		push_warning("EngineFactory: JDSource.create_phone_server no devolvió una fuente; se usa la simulación")
	return MockJDSource.create_phone_server(port, pairing_code)


## Reproducción de una sesión .jsonl. null si el archivo no existe.
static func create_replay(path: String, speed: float = 1.0) -> Object:
	if has_extension():
		var v: Variant = call_static(SOURCE_CLASS, "create_replay", [path, speed])
		return v if v is Object else null
	return MockJDSource.create_replay(path, speed)


## Grabador que envuelve `inner`. Nativo solo si `inner` es nativo.
static func create_recorder(inner: Object, out_path: String) -> Object:
	if inner == null:
		return null
	if has_extension() and not is_mock(inner) and inner.get_script() == null:
		var v: Variant = call_static(SOURCE_CLASS, "create_recorder", [inner, out_path])
		return v if v is Object else null
	return MockJDSource.create_recorder(inner, out_path)


## Fuente simulada (un móvil falso "Pixel (simulado)"), aunque la extensión esté cargada. Nunca null.
static func create_mock_source() -> Object:
	return MockJDSource.create_phone_server(DEFAULT_PORT, 0)


# ------------------------------------------------------------------ reloj

## Reloj monotónico oficial (ns): JDEngine.now_ns() si hay extensión, si no Time.get_ticks_usec()·1000.
## SongClock, MockJDSource y MockJDEngine usan este, para que muestras y canción compartan base.
static func now_ns() -> int:
	if not _clock_ready:
		_setup_clock()
	match _clock_mode:
		1:
			return int(_clock_callable.call())
		2:
			return int(ClassDB.callv("class_call_static", [ENGINE_CLASS, "now_ns"]))
	return Time.get_ticks_usec() * 1000


static func now_s() -> float:
	return float(now_ns()) * 1e-9


static func clock_description() -> String:
	if not _clock_ready:
		_setup_clock()
	match _clock_mode:
		1:
			return "JDEngine.now_ns (instancia)"
		2:
			return "JDEngine.now_ns (ClassDB.class_call_static)"
	return "Time.get_ticks_usec (sin extensión)"


## El reloj no depende de force_mock: si la extensión existe, sus fuentes nativas usan jd_now_ns.
## Preferimos llamar al método estático a través de una instancia guardada (en Godot un método
## estático registrado en ClassDB se puede invocar sobre una instancia; es lo más barato por llamada).
## Si la clase no se puede instanciar, usamos ClassDB.class_call_static (existe desde Godot 4.4).
static func _setup_clock() -> void:
	_clock_ready = true
	_clock_mode = 0
	if not ClassDB.class_exists(ENGINE_CLASS) or not ClassDB.class_has_method(ENGINE_CLASS, "now_ns"):
		return
	if ClassDB.can_instantiate(ENGINE_CLASS):
		var inst: Object = ClassDB.instantiate(ENGINE_CLASS)
		if inst != null and inst.has_method("now_ns"):
			_clock_holder = inst
			_clock_callable = Callable(inst, "now_ns")
			var v: Variant = _clock_callable.call()
			if v is int:
				_clock_mode = 1
				return
	if ClassDB.has_method("class_call_static"):
		var v2: Variant = ClassDB.callv("class_call_static", [ENGINE_CLASS, "now_ns"])
		if v2 is int:
			_clock_mode = 2
			return
	push_warning("EngineFactory: no se pudo llamar a JDEngine.now_ns(); se usa Time.get_ticks_usec()")


# ------------------------------------------------------------------ auxiliares

## Llama a un método estático de una clase nativa. null si la clase o el método no existen.
static func call_static(cls: String, method: String, args: Array) -> Variant:
	if not ClassDB.class_exists(cls) or not ClassDB.class_has_method(cls, method):
		return null
	if ClassDB.has_method("class_call_static"):
		return ClassDB.callv("class_call_static", [cls, method] + args)
	# Plan B (Godot < 4.4): un método estático se puede invocar sobre una instancia cualquiera.
	if ClassDB.can_instantiate(cls):
		var inst: Object = ClassDB.instantiate(cls)
		if inst != null:
			return inst.callv(method, args)
	return null
