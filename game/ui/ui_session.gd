# ui_session.gd — estado compartido entre escenas (agente E).
# Nodo persistente propio (no es un autoload: project.godot es de G). get_state() lo crea la primera
# vez y lo cuelga de /root con call_deferred; los nodos hijos de /root que no son la escena actual
# sobreviven a change_scene_to_file. Guarda el servidor de móviles (JDSource o MockJDSource de D),
# que así se comparte entre phone_pair, calibración y partida, y lo para al cerrar el juego.
#
# Motor, fuentes y reloj: los de game/core (EngineFactory, MockJDEngine/MockJDSource, SongClock).
class_name UiSession
extends Node

const NODE_NAME := "UiSession"
const LIMB_NONE := 255
const LIMB_LABELS := ["Mano izquierda", "Mano derecha", "Pie izquierdo", "Pie derecho"]

const SOURCE_PHONES := "phones"
const SOURCE_REPLAY := "replay"
const SOURCE_MOCK := "mock"

## Elección de entrada sin móviles: REPLAY_AUTO = sesión grabada con el mismo song_id (si hay);
## REPLAY_NONE = simulación directamente; cualquier otro valor = ruta de un .jsonl.
const REPLAY_AUTO := "auto"
const REPLAY_NONE := ""

const MAIN_MENU_SCENE := "res://scenes/main_menu.tscn"
const PHONE_PAIR_SCENE := "res://scenes/phone_pair.tscn"
const GAMEPLAY_SCENE := "res://scenes/gameplay.tscn"
const RESULTS_SCENE := "res://scenes/results.tscn"
const CAL_INPUT_SCENE := "res://core/calibration/calibration_input.tscn"
const CAL_VISUAL_SCENE := "res://core/calibration/calibration_visual.tscn"

static var _instance: UiSession = null

## Paquete de canción elegido (diccionario de SongLibrary.load_package).
var song: Dictionary = {}
var replay_choice: String = REPLAY_AUTO
## Servidor de móviles (JDSource o MockJDSource), arrancado. null = no se ha abierto.
var phone_server: Object = null
var phone_server_error := ""
## Escribir user://fps_log.csv ("t_s,fps") durante la partida.
var fps_log := false
## Resultado de la última partida (lo muestra results).
var last_result: Dictionary = {}

var _own_profile: Profile = null


static func get_state() -> UiSession:
	if is_instance_valid(_instance):
		return _instance
	var s := UiSession.new()
	s.name = NODE_NAME
	s.fps_log = fps_log_from_cmdline()
	_instance = s
	var loop := Engine.get_main_loop()
	if loop is SceneTree:
		(loop as SceneTree).root.add_child.call_deferred(s)
	return s


func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST or what == NOTIFICATION_PREDELETE:
		shutdown()


## Para el servidor de móviles (al salir del juego).
func shutdown() -> void:
	if phone_server != null:
		phone_server.call(&"stop")
		phone_server = null
	if _instance == self:
		_instance = null


# ------------------------------------------------------------------ perfil

## El perfil compartido: el del autoload SongClock (así la calibración y el emparejamiento escriben
## en el mismo objeto). Sin autoload, uno propio cargado de disco.
func profile() -> Profile:
	var loop := Engine.get_main_loop()
	if loop is SceneTree:
		var clock: Node = (loop as SceneTree).root.get_node_or_null(^"SongClock")
		if clock != null:
			var p: Variant = clock.get("profile")
			if p is Profile:
				return p as Profile
	if _own_profile == null:
		_own_profile = Profile.load_or_default()
	return _own_profile


# ------------------------------------------------------------------ servidor de móviles

## Devuelve el servidor de móviles arrancado (lo crea con Profile.phone_port). null si no arranca
## (mensaje en phone_server_error).
func ensure_phone_server() -> Object:
	if phone_server != null:
		return phone_server
	phone_server_error = ""
	var p := profile()
	var src: Object = EngineFactory.create_phone_server(p.phone_port, 0)
	if src == null:
		phone_server_error = "No se pudo crear el servidor de móviles."
		return null
	if not bool(src.call(&"start")):
		phone_server_error = "No se pudo abrir el puerto %d: %s" % [p.phone_port, str(src.call(&"last_error"))]
		return null
	phone_server = src
	return src


static func devices_of(src: Object) -> Array:
	if src == null or not src.has_method("devices"):
		return []
	var devs: Variant = src.call(&"devices")
	if devs is Array:
		return devs
	return []


## Número de móviles conectados de una fuente.
static func connected_devices(src: Object) -> int:
	var n := 0
	for d: Variant in devices_of(src):
		if d is Dictionary and bool((d as Dictionary).get("connected", false)):
			n += 1
	return n


## Aplica a la fuente las extremidades guardadas en el perfil (por nombre de dispositivo).
func apply_saved_limbs(src: Object) -> void:
	if src == null:
		return
	var p := profile()
	for d: Variant in devices_of(src):
		if not (d is Dictionary):
			continue
		var dev: Dictionary = d
		var dev_name := str(dev.get("name", ""))
		if p.limb_by_device.has(dev_name):
			var want := int(p.limb_by_device[dev_name])
			if int(dev.get("limb", LIMB_NONE)) != want:
				src.call(&"assign_limb", int(dev.get("id", 0)), want)


## Asigna una extremidad a un dispositivo y la guarda en el perfil.
func set_device_limb(src: Object, id: int, dev_name: String, limb: int) -> void:
	if src == null:
		return
	src.call(&"assign_limb", id, limb)
	var p := profile()
	p.limb_by_device[dev_name] = limb
	var err := p.save(Profile.DEFAULT_PATH)
	if err != OK:
		push_warning("UiSession: no se pudo guardar el perfil: %s" % error_string(err))


static func limb_label(limb: int) -> String:
	if limb >= 0 and limb < LIMB_LABELS.size():
		return LIMB_LABELS[limb]
	return "Sin asignar"


# ------------------------------------------------------------------ fuente de la partida

## Elige la fuente de entrada para `pkg`: servidor de móviles si hay alguno conectado; si no, una
## sesión grabada (replay); si no, la simulación. Devuelve
## {source, kind, label, owned (la partida debe pararla al salir), start_at (s de tiempo audible en que
##  arrancar un replay; −1 = ya arrancada)}.
func pick_source(pkg: Dictionary) -> Dictionary:
	if phone_server != null and connected_devices(phone_server) > 0:
		apply_saved_limbs(phone_server)
		return {
			"source": phone_server, "kind": SOURCE_PHONES, "owned": false, "start_at": -1.0,
			"label": "Móviles conectados: %d" % connected_devices(phone_server),
		}
	var path := replay_path_for(pkg)
	if path != "":
		var info := UiSessionLibrary.read_header(path)
		var rep: Object = EngineFactory.create_replay(path, 1.0)
		if rep != null:
			# El replay emite cada muestra (t − t_primera) s después de start(). Con time_base "song"
			# se arranca cuando el tiempo audible vale t_primera + input_offset (engine_mapping resta
			# input_offset), así las muestras caen en su tiempo de canción.
			var start_at := 0.0
			if str(info.get("time_base", "song")) == "song":
				start_at = float(info.get("first_t", 0.0)) + profile().input_offset
			return {
				"source": rep, "kind": SOURCE_REPLAY, "owned": true, "start_at": start_at,
				"label": "Sesión grabada: %s" % path.get_file(),
			}
	var mock: Object = EngineFactory.create_mock_source()
	mock.call(&"start")
	return {
		"source": mock, "kind": SOURCE_MOCK, "owned": true, "start_at": -1.0,
		"label": "Simulación (sin móvil)",
	}


## Ruta de la sesión a reproducir para `pkg` según replay_choice ("" = ninguna).
func replay_path_for(pkg: Dictionary) -> String:
	if replay_choice == REPLAY_NONE:
		return ""
	if replay_choice != REPLAY_AUTO:
		return replay_choice if FileAccess.file_exists(replay_choice) else ""
	var sid := str(pkg.get("id", ""))
	if sid.is_empty():
		return ""
	for s: Variant in UiSessionLibrary.scan():
		var info: Dictionary = s
		if str(info.get("song_id", "")) == sid and str(info.get("time_base", "")) == "song":
			return str(info["path"])
	return ""


# ------------------------------------------------------------------ escenas

## Abre una escena de calibración de D pasándole la fuente compartida (si la hay) y el perfil.
## Se hace a mano (no change_scene_to_file) para poder asignar `source` antes de add_child.
## La escena queda como escena actual: su "Volver" vuelve a la escena principal (main_menu).
func open_calibration(tree: SceneTree, path: String) -> void:
	var packed := load(path) as PackedScene
	if packed == null:
		push_warning("UiSession: no se puede cargar %s" % path)
		return
	var sc: Node = packed.instantiate()
	if phone_server != null and "source" in sc:
		sc.set("source", phone_server)
	if "profile" in sc:
		sc.set("profile", profile())
	var old := tree.current_scene
	tree.root.add_child(sc)
	tree.current_scene = sc
	if old != null:
		old.queue_free()


## ¿Se pidió el log de fps por línea de órdenes? (`godot --path game -- --fps-log` o JD_FPS_LOG=1)
static func fps_log_from_cmdline() -> bool:
	return OS.get_cmdline_user_args().has("--fps-log") or OS.get_environment("JD_FPS_LOG") == "1"
