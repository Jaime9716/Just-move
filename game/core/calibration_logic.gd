# calibration_logic.gd — lógica pura de calibración (B9). Ver contracts/game_logic_api.md.
class_name CalibrationLogic
extends RefCounted

## Mínimo de eventos emparejados para dar el resultado por bueno.
const MIN_EVENTS := 4


## Empareja cada evento con el clic más cercano y descarta los que se alejan más de max_abs (s).
## Devuelve {ok, offset (mediana; + = el jugador va tarde), stddev, n_used}.
static func estimate_offset(click_times: PackedFloat64Array, event_times: PackedFloat64Array, max_abs: float = 0.3) -> Dictionary:
	var clicks: PackedFloat64Array = click_times.duplicate()
	clicks.sort()
	var diffs := PackedFloat64Array()
	if clicks.size() > 0:
		for e: float in event_times:
			var nearest: float = _nearest(clicks, e)
			var d: float = e - nearest
			if absf(d) <= max_abs:
				diffs.append(d)
	var n := diffs.size()
	var offset := 0.0
	var stddev := 0.0
	if n > 0:
		diffs.sort()
		var mid := n >> 1
		if n % 2 == 1:
			offset = diffs[mid]
		else:
			offset = (diffs[mid - 1] + diffs[mid]) * 0.5
		var mean := 0.0
		for d: float in diffs:
			mean += d
		mean /= float(n)
		var acc := 0.0
		for d: float in diffs:
			acc += (d - mean) * (d - mean)
		stddev = sqrt(acc / float(n))
	return {"ok": n >= MIN_EVENTS, "offset": offset, "stddev": stddev, "n_used": n}


## Tiempos (s, reloj de las muestras) en que |a| cruza `threshold` hacia arriba.
## `samples` = diccionarios de JDSource.poll_samples ({t_ns:int, id:int, a:Vector3, ...}).
## Se ordenan por t_ns; el estado "por encima/por debajo" se lleva por dispositivo (id) y empieza
## "por debajo". Dos sacudidas a menos de min_sep (s) cuentan como una.
static func detect_shakes(samples: Array, threshold: float = 25.0, min_sep: float = 0.15) -> PackedFloat64Array:
	var out := PackedFloat64Array()
	var ordered: Array = []
	for s: Variant in samples:
		if typeof(s) == TYPE_DICTIONARY and (s as Dictionary).has("t_ns") and (s as Dictionary).has("a"):
			ordered.append(s)
	ordered.sort_custom(func(x: Dictionary, y: Dictionary) -> bool: return int(x["t_ns"]) < int(y["t_ns"]))
	var above_by_id := {}
	var last_t := -INF
	for s: Dictionary in ordered:
		var mag := _magnitude(s["a"])
		if mag < 0.0:
			continue
		var id: int = int(s.get("id", 0))
		var was_above: bool = above_by_id.get(id, false)
		var is_above := mag >= threshold
		above_by_id[id] = is_above
		if is_above and not was_above:
			var t := float(int(s["t_ns"])) * 1e-9
			if t - last_t >= min_sep:
				out.append(t)
				last_t = t
	return out


# --- auxiliares ---

static func _nearest(sorted_clicks: PackedFloat64Array, x: float) -> float:
	# Búsqueda binaria del clic más cercano.
	var i := sorted_clicks.bsearch(x)
	if i <= 0:
		return sorted_clicks[0]
	if i >= sorted_clicks.size():
		return sorted_clicks[sorted_clicks.size() - 1]
	var lo: float = sorted_clicks[i - 1]
	var hi: float = sorted_clicks[i]
	return lo if (x - lo) <= (hi - x) else hi


static func _magnitude(a: Variant) -> float:
	if typeof(a) == TYPE_VECTOR3:
		return (a as Vector3).length()
	if typeof(a) == TYPE_ARRAY or typeof(a) == TYPE_PACKED_FLOAT32_ARRAY or typeof(a) == TYPE_PACKED_FLOAT64_ARRAY:
		if a.size() >= 3:
			return Vector3(float(a[0]), float(a[1]), float(a[2])).length()
	return -1.0
