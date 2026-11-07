# requires: CalibrationLogic
# Tests de caja negra de CalibrationLogic (contracts/game_logic_api.md, docs/06 fila B9).
extends "res://tests/test_base.gd"

const GRAVITY := 9.81
const RATE_NS := 5_000_000       # 200 Hz


static func _clicks(first: float, period: float, count: int) -> PackedFloat64Array:
	var out := PackedFloat64Array()
	for i in range(count):
		out.append(first + float(i) * period)
	return out


## Eventos = clics + offset + ruido gaussiano (semilla fija).
static func _events(clicks: PackedFloat64Array, offset: float, noise_sd: float, seed_value: int) -> PackedFloat64Array:
	var rng := RandomNumberGenerator.new()
	rng.seed = seed_value
	var out := PackedFloat64Array()
	for c: float in clicks:
		out.append(c + offset + rng.randfn(0.0, noise_sd))
	return out


func _check_result_shape(r: Dictionary) -> void:
	for k: String in ["ok", "offset", "stddev", "n_used"]:
		assert_has(r, k)
	assert_type(r.get("ok"), TYPE_BOOL, "ok es bool")
	assert_type(r.get("n_used"), TYPE_INT, "n_used es int")


# ------------------------------------------------------------------ estimate_offset

func test_known_offset_with_noise() -> void:
	var clicks := _clicks(2.0, 0.6, 16)
	var events := _events(clicks, 0.045, 0.005, 7)
	var r := CalibrationLogic.estimate_offset(clicks, events)
	_check_result_shape(r)
	assert_true(bool(r["ok"]), "16 eventos → ok")
	assert_eq(int(r["n_used"]), 16, "n_used")
	assert_near(float(r["offset"]), 0.045, 0.004, "offset (mediana) ≈ 45 ms")
	assert_gt(float(r["stddev"]), 0.001, "stddev refleja el ruido")
	assert_lt(float(r["stddev"]), 0.012, "stddev ≈ 5 ms")
	done()


func test_player_early_gives_negative_offset() -> void:
	var clicks := _clicks(1.0, 0.5, 10)
	var events := _events(clicks, -0.030, 0.0, 1)
	var r := CalibrationLogic.estimate_offset(clicks, events)
	assert_true(bool(r["ok"]))
	assert_near(float(r["offset"]), -0.030, 1e-9, "+ = tarde, − = pronto")
	assert_near(float(r["stddev"]), 0.0, 1e-9, "sin ruido → stddev 0")
	done()


func test_median_even_count() -> void:
	var clicks := PackedFloat64Array([0.0, 1.0, 2.0, 3.0])
	var events := PackedFloat64Array([0.01, 1.02, 2.03, 3.04])
	var r := CalibrationLogic.estimate_offset(clicks, events)
	assert_true(bool(r["ok"]), "exactamente 4 eventos → ok")
	assert_eq(int(r["n_used"]), 4)
	assert_near(float(r["offset"]), 0.025, 1e-9, "mediana de 4 = media de los 2 centrales")
	# Desviación de {10,20,30,40} ms: 11,2 ms (poblacional) o 12,9 ms (muestral); el contrato no lo fija.
	assert_near(float(r["stddev"]), 0.012, 0.0025, "stddev")
	done()


func test_outliers_discarded() -> void:
	var clicks := _clicks(0.0, 1.0, 12)
	var good := _events(clicks, 0.040, 0.003, 11)
	var events := PackedFloat64Array(good)
	events.append(3.0 + 0.40)     # 0,40 s tarde respecto a su clic más cercano → fuera
	events.append(5.0 - 0.35)     # 0,35 s pronto → fuera
	events.append(-5.0)           # muy lejos de todo → fuera
	events.append(40.0)           # después del último clic → fuera
	var r := CalibrationLogic.estimate_offset(clicks, events)
	assert_eq(int(r["n_used"]), 12, "los 4 atípicos se descartan (max_abs 0,3)")
	assert_true(bool(r["ok"]))
	assert_near(float(r["offset"]), 0.040, 0.003, "offset sin contaminar")
	done()


func test_max_abs_parameter() -> void:
	var clicks := _clicks(0.0, 1.0, 8)
	var events := _events(clicks, 0.12, 0.0, 1)
	var r1 := CalibrationLogic.estimate_offset(clicks, events, 0.3)
	assert_eq(int(r1["n_used"]), 8, "0,12 < max_abs 0,3")
	var r2 := CalibrationLogic.estimate_offset(clicks, events, 0.1)
	assert_eq(int(r2["n_used"]), 0, "0,12 > max_abs 0,1 → todos fuera")
	assert_false(bool(r2["ok"]), "sin eventos válidos → ok false")
	done()


func test_median_robust_to_inlier_outliers() -> void:
	# Dos golpes a +250 ms (dentro de max_abs) no mueven la mediana de 14 buenos.
	var clicks := _clicks(0.0, 1.0, 16)
	var events := PackedFloat64Array()
	for i in range(16):
		var off := 0.25 if (i == 3 or i == 9) else 0.050
		events.append(clicks[i] + off)
	var r := CalibrationLogic.estimate_offset(clicks, events)
	assert_eq(int(r["n_used"]), 16)
	assert_near(float(r["offset"]), 0.050, 1e-9, "mediana robusta")
	done()


func test_less_than_4_events_not_ok() -> void:
	var clicks := _clicks(0.0, 0.5, 10)
	var events := PackedFloat64Array([0.03, 0.53, 1.03])
	var r := CalibrationLogic.estimate_offset(clicks, events)
	_check_result_shape(r)
	assert_false(bool(r["ok"]), "3 eventos → ok false")
	assert_eq(int(r["n_used"]), 3)
	done()


func test_empty_inputs() -> void:
	var r := CalibrationLogic.estimate_offset(PackedFloat64Array(), PackedFloat64Array([0.1, 0.2, 0.3, 0.4, 0.5]))
	_check_result_shape(r)
	assert_false(bool(r["ok"]), "sin clics → ok false")
	assert_eq(int(r["n_used"]), 0)
	var r2 := CalibrationLogic.estimate_offset(_clicks(0.0, 1.0, 5), PackedFloat64Array())
	assert_false(bool(r2["ok"]), "sin eventos → ok false")
	assert_eq(int(r2["n_used"]), 0)
	done()


func test_unsorted_events() -> void:
	var clicks := _clicks(0.0, 1.0, 6)
	var events := PackedFloat64Array([5.02, 0.02, 3.02, 1.02, 4.02, 2.02])
	var r := CalibrationLogic.estimate_offset(clicks, events)
	assert_eq(int(r["n_used"]), 6)
	assert_near(float(r["offset"]), 0.02, 1e-9)
	done()


# ------------------------------------------------------------------ detect_shakes

## Señal sintética a 200 Hz: reposo (gravedad + ruido pequeño) y, en cada instante de `shakes_s`,
## `width` muestras con a.x = peak. t_ns empieza en base_ns.
static func _signal(duration_s: float, shakes_s: PackedFloat64Array, peak: float, width: int, base_ns: int, dev_id: int = 1) -> Array:
	var rng := RandomNumberGenerator.new()
	rng.seed = 99
	var out: Array = []
	var n := int(duration_s * 1e9 / float(RATE_NS))
	var shake_idx := {}
	for s: float in shakes_s:
		var i0 := int(roundf(s * 1e9 / float(RATE_NS)))
		for k in range(width):
			shake_idx[i0 + k] = true
	for i in range(n):
		var a := Vector3(rng.randfn(0.0, 0.2), GRAVITY + rng.randfn(0.0, 0.2), rng.randfn(0.0, 0.2))
		if shake_idx.has(i):
			a.x = peak
		out.append({"t_ns": base_ns + i * RATE_NS, "id": dev_id, "limb": 1, "a": a, "g": Vector3.ZERO})
	return out


func _assert_times(got: PackedFloat64Array, expected: PackedFloat64Array, tol: float, msg: String) -> void:
	if not assert_eq(got.size(), expected.size(), msg + ": número de sacudidas (obtenidas %s)" % str(got)):
		return
	for i in range(expected.size()):
		assert_near(got[i], expected[i], tol, "%s: sacudida %d" % [msg, i])


func test_detect_shakes_basic() -> void:
	var shakes := PackedFloat64Array([0.5, 1.2, 2.0, 2.6])
	var samples := _signal(3.0, shakes, 30.0, 3, 0)
	var got := CalibrationLogic.detect_shakes(samples)
	assert_type(got, TYPE_PACKED_FLOAT64_ARRAY)
	_assert_times(got, shakes, 0.0051, "4 sacudidas de |a| ≈ 31,6")
	done()


func test_detect_shakes_time_base_seconds_from_t_ns() -> void:
	# t_ns grandes (reloj monotónico real): los tiempos salen en segundos de ese reloj.
	var base := 123_456_000_000_000   # 123456 s
	var samples := _signal(2.0, PackedFloat64Array([1.0]), 30.0, 2, base)
	var got := CalibrationLogic.detect_shakes(samples)
	_assert_times(got, PackedFloat64Array([123456.0 + 1.0]), 0.0051, "t en s = t_ns · 1e-9")
	done()


func test_detect_shakes_rest_gives_nothing() -> void:
	var got := CalibrationLogic.detect_shakes(_signal(3.0, PackedFloat64Array(), 30.0, 3, 0))
	assert_eq(got.size(), 0, "reposo → ninguna sacudida")
	assert_eq(CalibrationLogic.detect_shakes([]).size(), 0, "sin muestras → vacío")
	done()


func test_detect_shakes_threshold_param() -> void:
	var samples := _signal(2.0, PackedFloat64Array([0.5, 1.5]), 30.0, 3, 0)
	assert_eq(CalibrationLogic.detect_shakes(samples, 40.0).size(), 0, "umbral 40 > |a| 31,6")
	assert_eq(CalibrationLogic.detect_shakes(samples, 20.0).size(), 2, "umbral 20 < |a|")
	# Golpe flojo de |a| ≈ 20,3: no supera el umbral por defecto (25).
	var weak := _signal(2.0, PackedFloat64Array([0.5]), 17.8, 3, 0)
	assert_eq(CalibrationLogic.detect_shakes(weak).size(), 0, "golpe flojo < 25")
	done()


func test_detect_shakes_held_counts_once() -> void:
	# Por encima del umbral durante 0,3 s: un solo cruce hacia arriba.
	var samples := _signal(2.0, PackedFloat64Array([0.8]), 30.0, 60, 0)
	_assert_times(CalibrationLogic.detect_shakes(samples), PackedFloat64Array([0.8]), 0.0051, "sacudida larga")
	done()


func test_detect_shakes_min_sep() -> void:
	# Dos cruces a 50 ms (< min_sep 0,15) cuentan como uno; a 300 ms, como dos.
	var close := _signal(2.0, PackedFloat64Array([1.0, 1.05]), 30.0, 2, 0)
	_assert_times(CalibrationLogic.detect_shakes(close), PackedFloat64Array([1.0]), 0.0051, "a 50 ms")
	var apart := _signal(2.0, PackedFloat64Array([1.0, 1.3]), 30.0, 2, 0)
	_assert_times(CalibrationLogic.detect_shakes(apart), PackedFloat64Array([1.0, 1.3]), 0.0051, "a 300 ms")
	# min_sep explícito más pequeño: los de 50 ms ya cuentan como dos.
	_assert_times(CalibrationLogic.detect_shakes(close, 25.0, 0.02), PackedFloat64Array([1.0, 1.05]), 0.0051,
			"min_sep 0,02")
	done()


func test_detect_shakes_feeds_estimate_offset() -> void:
	# De punta a punta: clics cada 0,6 s, el jugador sacude 40 ms tarde.
	var clicks := _clicks(1.0, 0.6, 12)
	var shake_t := PackedFloat64Array()
	for c: float in clicks:
		shake_t.append(c + 0.040)
	var samples := _signal(9.0, shake_t, 30.0, 3, 0)
	var events := CalibrationLogic.detect_shakes(samples)
	assert_eq(events.size(), 12, "una sacudida por clic")
	var r := CalibrationLogic.estimate_offset(clicks, events)
	assert_true(bool(r["ok"]))
	assert_near(float(r["offset"]), 0.040, 0.0051, "offset recuperado (± 1 muestra a 200 Hz)")
	done()
