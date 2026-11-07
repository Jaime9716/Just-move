# ui_clock.gd — adaptador al reloj de canción (agente E).
# Usa el autoload SongClock del agente D (contracts/game_logic_api.md: audible(), visual(),
# engine_mapping(now)) y le entrega el AudioStreamPlayer de la partida. Si no hay autoload o no
# acepta el reproductor, usa un SongClockLogic local con la misma fórmula (docs/02).
class_name UiClock
extends RefCounted

const AUTOLOAD_NAME := "SongClock"
## Métodos (de 1 argumento) con los que el autoload podría aceptar el reproductor.
const ATTACH_METHODS := ["attach_player", "attach_music", "set_music_player", "set_player", "set_music", "attach", "bind_player", "use_player"]
const DETACH_METHODS := ["detach_player", "detach_music", "detach", "release_player", "clear_player"]
## Propiedades equivalentes (el SongClock antiguo tenía `var music`).
const PLAYER_PROPS := ["music", "player", "audio_player", "music_player"]

var node: Node = null
var local := SongClockLogic.new()
var player: AudioStreamPlayer = null
var using_autoload := false
var _attached_prop := ""
var _input_offset := 0.0


func _init(tree: SceneTree, p: AudioStreamPlayer, profile: Profile) -> void:
	player = p
	local.audio_offset = profile.audio_offset
	local.visual_offset = profile.visual_offset
	local.input_offset = profile.input_offset
	_input_offset = profile.input_offset
	var n: Node = tree.root.get_node_or_null(AUTOLOAD_NAME)
	if n != null and n.has_method("audible") and n.has_method("visual") and _attach(n, p):
		node = n
		using_autoload = true
		_apply_offsets(n, profile)


func describe() -> String:
	return "SongClock (autoload)" if using_autoload else "reloj local (sin autoload SongClock compatible)"


## Llamar justo después de player.play().
func started() -> void:
	local.reset(0.0)
	if using_autoload and node.has_method("reset"):
		var argc := node.get_method_argument_count("reset")
		node.callv("reset", [0.0] if argc >= 1 else [])


## Llamar en cada _process ANTES de leer el tiempo (solo hace algo en modo local:
## el autoload se actualiza en su propio _process, que corre antes que la escena).
func tick(delta: float) -> void:
	if using_autoload or player == null or not player.playing:
		return
	var raw := player.get_playback_position() + AudioServer.get_time_since_last_mix() - AudioServer.get_output_latency()
	local.step(raw, delta)


func audible() -> float:
	if using_autoload:
		return float(node.call("audible"))
	return local.audible()


func visual() -> float:
	if using_autoload:
		return float(node.call("visual"))
	return local.visual()


## Reloj monotónico (s) en la base de JDEngine.now_ns().
func now_mono_s() -> float:
	if using_autoload:
		for m: String in ["now_mono_s", "mono_now_s"]:
			if node.has_method(m):
				return float(node.call(m))
	return UiEngineAccess.now_s()


## Valor para JDEngine.set_time_mapping.
func engine_mapping(now_mono: float) -> float:
	if using_autoload:
		if node.has_method("engine_mapping"):
			if node.get_method_argument_count("engine_mapping") >= 1:
				return float(node.call("engine_mapping", now_mono))
			return float(node.call("engine_mapping"))
		# Autoload sin engine_mapping: song − now − input − audio = audible − now − input.
		return audible() - now_mono - _input_offset
	return local.engine_mapping(now_mono)


## Suelta el reproductor (se libera con la escena; el autoload no debe quedarse con él).
func release() -> void:
	if not using_autoload or not is_instance_valid(node):
		return
	for m: String in DETACH_METHODS:
		if node.has_method(m) and node.get_method_argument_count(m) == 0:
			node.call(m)
			return
	if _attached_prop != "":
		node.set(_attached_prop, null)


func _attach(n: Node, p: AudioStreamPlayer) -> bool:
	for m: String in ATTACH_METHODS:
		if n.has_method(m) and n.get_method_argument_count(m) == 1:
			n.call(m, p)
			return true
	for prop: String in PLAYER_PROPS:
		if prop in n:
			n.set(prop, p)
			_attached_prop = prop
			return true
	return false


func _apply_offsets(n: Node, profile: Profile) -> void:
	for m: String in ["apply_profile", "use_profile", "set_profile"]:
		if n.has_method(m) and n.get_method_argument_count(m) == 1:
			n.call(m, profile)
			return
	var target: Object = n
	if "logic" in n and n.get("logic") is SongClockLogic:
		target = n.get("logic")
	for key: String in ["audio_offset", "visual_offset", "input_offset"]:
		if key in target:
			target.set(key, profile.get(key))
