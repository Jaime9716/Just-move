# ui_fallback_engine.gd — motor simulado MÍNIMO de la UI (agente E).
# Solo se usa si no están ni la GDExtension ni game/core/engine_factory.gd (agente D), para poder
# abrir y probar las pantallas. Emite un resultado pseudoaleatorio (semilla fija) por paso y extremidad.
class_name UiFallbackEngine
extends RefCounted

const EMIT_DELAY_S := 0.3

var _steps: Array = []
var _next := 0
var _rng := RandomNumberGenerator.new()
var _mapping := 0.0


static func now_ns() -> int:
	return Time.get_ticks_usec() * 1000


func load_song(song_dir: String) -> String:
	var pkg := SongLibrary.load_package(song_dir)
	if not bool(pkg["ok"]):
		var errs: PackedStringArray = pkg["errors"]
		return "\n".join(errs)
	_steps = pkg["steps"]
	reset()
	return ""


func set_config(_perfect_ms: float, _good_ms: float, _foot_scale: float) -> void:
	pass


func set_time_mapping(mono_to_song_s: float) -> void:
	_mapping = mono_to_song_s


## Consume las muestras de la fuente (no las juzga).
func pump(source: Object) -> int:
	if source == null or not source.has_method("poll_samples"):
		return 0
	var got: Variant = source.call("poll_samples", 4096)
	return (got as Array).size() if got is Array else 0


func update(song_time_s: float) -> Array:
	var out: Array = []
	while _next < _steps.size():
		var st: Dictionary = _steps[_next]
		if song_time_s < float(st["t"]) + float(st["dur"]) + EMIT_DELAY_S:
			break
		var limbs: PackedInt32Array = st["limbs"]
		for limb: int in limbs:
			var roll := _rng.randf()
			var grade := 2 if roll < 0.55 else (1 if roll < 0.85 else 0)
			var err := _rng.randf_range(-45.0, 45.0) if grade > 0 else 0.0
			out.append({
				"step": _next, "limb": limb, "grade": grade,
				"similarity": 0.0 if grade == 0 else _rng.randf_range(0.5, 1.0),
				"timing_error_ms": err,
			})
		_next += 1
	return out


func reset() -> void:
	_next = 0
	_rng.seed = 20261106
