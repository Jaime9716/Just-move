# mock_engine.gd — simulación de JDEngine (contracts/gdextension_api.md) para cuando la GDExtension
# no está cargada (agente D). Misma interfaz que la clase nativa. La fuente simulada (MockJDSource,
# un móvil falso "Pixel (simulado)") está en game/core/mock_source.gd (un class_name por archivo).
#
# Juez simulado (NO es el juez v1): lee choreo.json, guarda la energía |‖a‖ − g| de las muestras por
# extremidad en tiempo de canción y, cuando un paso termina, da Miss si esa extremidad no se movió en
# la ventana del paso y, si se movió, una nota pseudoaleatoria con semilla (reproducible).
class_name MockJDEngine
extends RefCounted

const LIMB_NAMES := ["left_hand", "right_hand", "left_foot", "right_foot"]
const LIMB_COUNT := 4
const GRAVITY := 9.81
## Retardo (s) tras el final del paso antes de emitir su resultado (como el motor real).
const EMIT_DELAY_S := 0.25
## Margen (s) alrededor de [t, t+dur] en el que se buscan muestras.
const WINDOW_MARGIN_S := 0.2
## Energía mínima (m/s²) para considerar que la extremidad se movió.
const MIN_ENERGY := 0.5
## Historial de muestras que se conserva (s de canción).
const HISTORY_S := 8.0
const POLL_BATCH := 512
const MAX_POLLS_PER_PUMP := 64

## Semilla de los resultados pseudoaleatorios (se reaplica en reset()).
static var default_seed: int = 20261106

var perfect_ms := 50.0
var good_ms := 100.0
var foot_scale := 1.5

var _mapping := 0.0
var _steps: Array = []          # {t: float, dur: float, limbs: PackedInt32Array}
var _next := 0
var _rng := RandomNumberGenerator.new()
# Por extremidad, un Array de floats (Array y no PackedFloat64Array: los Packed se copian al
# sacarlos de un Array y append no modificaría el original).
var _hist_t: Array = []         # tiempo de canción (s)
var _hist_e: Array = []         # energía (m/s²)


func _init() -> void:
	reset()


static func now_ns() -> int:
	return EngineFactory.now_ns()


## "" si OK; si no, el mensaje de error (con ruta JSON).
func load_song(song_dir: String) -> String:
	var path := song_dir.path_join("choreo.json")
	if not FileAccess.file_exists(path):
		return "%s: no existe" % path
	var json := JSON.new()
	if json.parse(FileAccess.get_file_as_string(path)) != OK:
		return "%s:%d: JSON no válido: %s" % [path, json.get_error_line(), json.get_error_message()]
	if typeof(json.data) != TYPE_DICTIONARY:
		return "%s: la raíz debe ser un objeto" % path
	var d: Dictionary = json.data
	if str(d.get("format", "")) != "jd-choreo":
		return "format: debe ser \"jd-choreo\""
	var raw_steps: Variant = d.get("steps", null)
	if typeof(raw_steps) != TYPE_ARRAY:
		return "steps: debe ser una lista"
	var steps: Array = []
	var prev_t := -INF
	var i := 0
	for rs: Variant in raw_steps:
		if typeof(rs) != TYPE_DICTIONARY:
			return "steps[%d]: debe ser un objeto" % i
		var sd: Dictionary = rs
		if not _is_number(sd.get("t", null)):
			return "steps[%d].t: falta o no es un número" % i
		var t := float(sd["t"])
		if t < prev_t:
			return "steps[%d].t: debe ser >= steps[%d].t" % [i, i - 1]
		var dur := float(sd.get("dur", 0.0)) if _is_number(sd.get("dur", 0.0)) else 0.0
		var limbs := PackedInt32Array()
		var raw_limbs: Variant = sd.get("limbs", null)
		if typeof(raw_limbs) != TYPE_ARRAY or (raw_limbs as Array).is_empty():
			return "steps[%d].limbs: debe ser una lista no vacía" % i
		for ln: Variant in raw_limbs:
			var li := LIMB_NAMES.find(str(ln))
			if li < 0:
				return "steps[%d].limbs: extremidad desconocida \"%s\"" % [i, str(ln)]
			limbs.append(li)
		steps.append({"t": t, "dur": maxf(dur, 0.0), "limbs": limbs})
		prev_t = t
		i += 1
	_steps = steps
	reset()
	return ""


func set_config(p_perfect_ms: float, p_good_ms: float, p_foot_scale: float) -> void:
	perfect_ms = p_perfect_ms
	good_ms = maxf(p_good_ms, p_perfect_ms)
	foot_scale = p_foot_scale


func set_time_mapping(mono_to_song_s: float) -> void:
	_mapping = mono_to_song_s


## Lee todas las muestras de la fuente (JDSource o MockJDSource). Devuelve cuántas.
func pump(source: Object) -> int:
	if source == null or not source.has_method("poll_samples"):
		return 0
	var total := 0
	for _i in range(MAX_POLLS_PER_PUMP):
		var got: Variant = source.call(&"poll_samples", POLL_BATCH)
		if not (got is Array):
			break
		var arr: Array = got
		for s: Variant in arr:
			if typeof(s) != TYPE_DICTIONARY:
				continue
			var smp: Dictionary = s
			var limb := int(smp.get("limb", 255))
			if limb < 0 or limb >= LIMB_COUNT or not smp.has("t_ns"):
				continue
			var a: Variant = smp.get("a", Vector3.ZERO)
			var mag := (a as Vector3).length() if a is Vector3 else GRAVITY
			var song_t := float(int(smp["t_ns"])) * 1e-9 + _mapping
			var ht: Array = _hist_t[limb]
			var he: Array = _hist_e[limb]
			ht.append(song_t)
			he.append(absf(mag - GRAVITY))
		total += arr.size()
		if arr.size() < POLL_BATCH:
			break
	return total


## Resultados de los pasos cuyo juicio termina antes de song_time_s.
## {step:int, limb:int, grade:int (0 Miss, 1 Good, 2 Perfect), similarity:float, timing_error_ms:float}
func update(song_time_s: float) -> Array[Dictionary]:
	var out: Array[Dictionary] = []
	while _next < _steps.size():
		var st: Dictionary = _steps[_next]
		var t: float = st["t"]
		var dur: float = st["dur"]
		if song_time_s < t + dur + EMIT_DELAY_S:
			break
		var limbs: PackedInt32Array = st["limbs"]
		for limb: int in limbs:
			out.append(_judge(_next, limb, t - WINDOW_MARGIN_S, t + dur + WINDOW_MARGIN_S))
		_next += 1
	_prune(song_time_s - HISTORY_S)
	return out


func reset() -> void:
	_next = 0
	_rng.seed = default_seed
	_hist_t = []
	_hist_e = []
	for _i in range(LIMB_COUNT):
		_hist_t.append([])
		_hist_e.append([])


# ------------------------------------------------------------------ internos

func _judge(step: int, limb: int, from_t: float, to_t: float) -> Dictionary:
	var ts: Array = _hist_t[limb]
	var es: Array = _hist_e[limb]
	var peak := 0.0
	for k in range(ts.size()):
		var tk: float = ts[k]
		if tk >= from_t and tk <= to_t:
			peak = maxf(peak, float(es[k]))
	var scale := foot_scale if limb >= 2 else 1.0
	var grade := 0
	if peak * scale >= MIN_ENERGY:
		var roll := _rng.randf()
		grade = 2 if roll < 0.55 else (1 if roll < 0.85 else 0)
	var err := 0.0
	var sim := 0.0
	if grade == 2:
		err = _rng.randf_range(-perfect_ms, perfect_ms) * 0.9
		sim = _rng.randf_range(0.8, 1.0)
	elif grade == 1:
		var sgn := 1.0 if _rng.randf() < 0.5 else -1.0
		err = sgn * _rng.randf_range(perfect_ms, good_ms)
		sim = _rng.randf_range(0.5, 0.8)
	else:
		sim = _rng.randf_range(0.0, 0.3) if peak > 0.0 else 0.0
	return {"step": step, "limb": limb, "grade": grade, "similarity": sim, "timing_error_ms": err}


func _prune(before_t: float) -> void:
	for limb in range(LIMB_COUNT):
		var ts: Array = _hist_t[limb]
		if ts.is_empty() or float(ts[0]) >= before_t:
			continue
		var cut := ts.bsearch(before_t)
		var es: Array = _hist_e[limb]
		_hist_t[limb] = ts.slice(cut)
		_hist_e[limb] = es.slice(cut)


static func _is_number(v: Variant) -> bool:
	return typeof(v) == TYPE_FLOAT or typeof(v) == TYPE_INT
