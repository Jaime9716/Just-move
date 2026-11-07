# ui_session.gd — estado compartido entre escenas (agente E).
# No hay autoload propio de la UI (project.godot es del agente G): el estado se guarda como
# metadato del singleton Engine, que sobrevive a change_scene_to_file.
class_name UiSession
extends RefCounted

const META_KEY := "jd_ui_session"

## Paquete de canción elegido (diccionario de SongLibrary.load_package).
var song: Dictionary = {}
## Sesión grabada (.jsonl) a reproducir si no hay móviles conectados; "" = ninguna.
var replay_path: String = ""
## Servidor de móviles (JDSource o simulación de D); se crea en phone_pair y se reutiliza.
var phone_server: Object = null
## Escribir user://fps_log.csv durante la partida (prueba P9).
var fps_log: bool = false
## Resultado de la última partida (lo muestra results).
var last_result: Dictionary = {}


static func get_state() -> UiSession:
	if Engine.has_meta(META_KEY):
		var s: Variant = Engine.get_meta(META_KEY)
		if s is UiSession:
			return s
	var n := UiSession.new()
	Engine.set_meta(META_KEY, n)
	return n


## Número de móviles conectados de una fuente (0 si no es un servidor de móviles).
static func connected_devices(src: Object) -> int:
	if src == null or not is_instance_valid(src) or not src.has_method("devices"):
		return 0
	var devs: Variant = src.call("devices")
	if not (devs is Array):
		return 0
	var n := 0
	for d: Variant in devs:
		if d is Dictionary and bool(d.get("connected", false)):
			n += 1
	return n


## ¿Se pidió el log de fps por línea de órdenes? (`godot --path game -- --fps-log`)
static func fps_log_from_cmdline() -> bool:
	return OS.get_cmdline_user_args().has("--fps-log") or OS.get_environment("JD_FPS_LOG") == "1"
