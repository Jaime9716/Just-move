# ui_fallback_source.gd — fuente simulada vacía (agente E). Mismos métodos que JDSource.
# Solo si engine_factory.gd (agente D) no da una simulación mejor.
class_name UiFallbackSource
extends RefCounted

var _running := false


func start() -> bool:
	_running = true
	return true


func stop() -> void:
	_running = false


func endpoint() -> String:
	return "(simulado)"


func pairing_code() -> int:
	return 0


func devices() -> Array:
	return []


func assign_limb(_id: int, _limb: int) -> void:
	pass


func set_recorder_mapping(_mono_to_song_s: float) -> void:
	pass


func poll_samples(_max: int = 512) -> Array:
	return []


func last_error() -> String:
	return ""
