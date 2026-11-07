# requires: SongClockLogic
# Tests de caja negra de SongClockLogic (contracts/game_logic_api.md, docs/02, docs/06 fila B2).
extends "res://tests/test_base.gd"

const SR := 48000.0
const BLOCK := 512.0
const BLOCK_S := BLOCK / SR            # 10,667 ms: get_playback_position avanza a estos saltos
const FPS_DT := 1.0 / 60.0


## Posición de audio "a bloques": la del último bloque de 512 muestras mezclado.
static func _blocky(true_t: float) -> float:
	return floorf(true_t / BLOCK_S) * BLOCK_S


func test_reset_and_song_time() -> void:
	var c := SongClockLogic.new()
	assert_eq(c.song_time(), 0.0, "recién creado")
	c.reset(12.5)
	assert_eq(c.song_time(), 12.5, "reset(t)")
	c.reset()
	assert_eq(c.song_time(), 0.0, "reset() sin argumento")
	done()


func test_step_returns_song_time() -> void:
	var c := SongClockLogic.new()
	var r := c.step(0.01, FPS_DT)
	assert_eq(r, c.song_time(), "step devuelve el tiempo suavizado")
	done()


func test_jump_forward_accepts_raw() -> void:
	var c := SongClockLogic.new()
	c.reset(1.0)
	assert_eq(c.step(5.0, FPS_DT), 5.0, "salto adelante > 0,1 s: se acepta la posición bruta")
	done()


func test_jump_backward_accepts_raw_seek() -> void:
	var c := SongClockLogic.new()
	c.reset(10.0)
	assert_eq(c.step(2.0, FPS_DT), 2.0, "seek hacia atrás > 0,1 s: se acepta la posición bruta")
	done()


func test_jump_threshold_is_0_1() -> void:
	# Predicho = t + delta = 1.0. 1,15 está a 0,15 (> 0,1) → salto; 1,08 está a 0,08 → suavizado.
	var c := SongClockLogic.new()
	c.reset(1.0)
	assert_eq(c.step(1.15, 0.0), 1.15, "a 0,15 s del predicho: salto")
	c.reset(1.0)
	var t := c.step(1.08, 0.0)
	assert_near(t, 1.0 + 0.08 * 0.1, 1e-12, "a 0,08 s del predicho: lerp(pred, raw, 0,1)")
	done()


func test_smoothing_formula() -> void:
	# t = max(t, lerp(t + delta, raw, 0,1))
	var c := SongClockLogic.new()
	c.reset(2.0)
	var t := c.step(2.05, 0.02)
	assert_near(t, lerpf(2.02, 2.05, 0.1), 1e-12)
	done()


func test_never_goes_back_with_small_backward_raw() -> void:
	var c := SongClockLogic.new()
	c.reset(1.0)
	assert_ge(c.step(0.95, 0.0), 1.0, "delta 0 y raw 50 ms atrás: no retrocede")
	c.reset(1.0)
	assert_ge(c.step(0.91, 0.0), 1.0, "raw 90 ms atrás: no retrocede")
	# Muchos pasos con raw congelado (audio atascado) y delta 0: nunca baja.
	c.reset(3.0)
	var prev := c.song_time()
	var ok := true
	for i in range(200):
		var t := c.step(2.95, 0.0)
		if t < prev:
			ok = false
		prev = t
	assert_true(ok, "raw atrasado repetido: monótono")
	done()


func test_monotonic_with_noisy_raw() -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = 1234
	var c := SongClockLogic.new()
	var true_t := 0.0
	var prev := 0.0
	var backs := 0
	for i in range(3600):
		var dt := FPS_DT * rng.randf_range(0.7, 1.3)
		true_t += dt
		var raw := _blocky(true_t) + rng.randf_range(-0.008, 0.008)
		var t := c.step(raw, dt)
		if t < prev:
			backs += 1
		prev = t
	assert_eq(backs, 0, "pasos en que el reloj retrocedió (sin seek)")
	done()


## Error del suavizado (docs/06, B2): audio a bloques de 512 muestras a 48 kHz y delta de 60 fps
## durante 180 s. La posición a bloques va de media medio bloque (5,33 ms) por detrás del audio real;
## ese sesgo constante no es error del suavizado (lo compensa la calibración). Se exige:
## |t − (t_real − medio bloque)| ≤ 2 ms, sesgo medio ≤ medio bloque + 1 ms y sin deriva.
func _check_smoothing(dt_of: Callable, label: String) -> void:
	var c := SongClockLogic.new()
	var true_t := 0.0
	var prev := 0.0
	var backs := 0
	var max_dev := 0.0
	var frames := int(180.0 * 60.0)
	var warmup := 60                       # 1 s para converger
	var sum_err := 0.0
	var n := 0
	var first_sum := 0.0
	var first_n := 0
	var last_sum := 0.0
	var last_n := 0
	for i in range(frames):
		var dt: float = dt_of.call(i)
		true_t += dt
		var t := c.step(_blocky(true_t), dt)
		if t < prev:
			backs += 1
		prev = t
		if i < warmup:
			continue
		var err := t - true_t
		max_dev = maxf(max_dev, absf(err + BLOCK_S * 0.5))
		sum_err += err
		n += 1
		if i < warmup + 600:
			first_sum += err
			first_n += 1
		if i >= frames - 600:
			last_sum += err
			last_n += 1
	var mean := sum_err / float(n)
	var first_mean := first_sum / float(first_n)
	var last_mean := last_sum / float(last_n)
	assert_eq(backs, 0, label + ": retrocesos")
	assert_le(max_dev, 0.002, label + ": error máximo respecto al centro del bloque (≤ 2 ms)")
	assert_le(absf(mean), BLOCK_S * 0.5 + 0.001, label + ": sesgo medio (≤ medio bloque + 1 ms)")
	assert_le(absf(last_mean - first_mean), 0.001, label + ": deriva entre los primeros y los últimos 10 s")
	assert_le(absf(true_t - c.song_time()), BLOCK_S + 0.002, label + ": al final sigue pegado al audio")


func test_smoothing_blocks_48k_60fps_180s() -> void:
	var dt_of := func(_i: int) -> float: return FPS_DT
	_check_smoothing(dt_of, "60 fps")
	done()


func test_smoothing_blocks_48k_59_94fps_180s() -> void:
	var dt_of := func(_i: int) -> float: return 1.0 / 59.94
	_check_smoothing(dt_of, "59,94 fps")
	done()


func test_smoothing_blocks_48k_60fps_jitter_180s() -> void:
	# Fotogramas irregulares (±5 %, determinista), el reloj del audio sigue siendo el real.
	var dt_of := func(i: int) -> float: return FPS_DT * (1.0 + 0.05 * sin(float(i) * 0.7))
	_check_smoothing(dt_of, "60 fps ±5 %")
	done()


func test_seek_then_smooth_again() -> void:
	var c := SongClockLogic.new()
	var true_t := 0.0
	for i in range(120):
		true_t += FPS_DT
		c.step(_blocky(true_t), FPS_DT)
	# seek a 60 s
	true_t = 60.0
	assert_eq(c.step(_blocky(true_t), FPS_DT), _blocky(true_t), "seek: acepta la posición bruta")
	for i in range(120):
		true_t += FPS_DT
		c.step(_blocky(true_t), FPS_DT)
	assert_le(absf(c.song_time() - (true_t - BLOCK_S * 0.5)), 0.002, "tras el seek vuelve a suavizar bien")
	done()


func test_offsets_default_zero() -> void:
	var c := SongClockLogic.new()
	assert_eq(c.audio_offset, 0.0)
	assert_eq(c.visual_offset, 0.0)
	assert_eq(c.input_offset, 0.0)
	c.reset(7.0)
	assert_eq(c.audible(), 7.0, "sin offsets audible = song_time")
	assert_eq(c.visual(), 7.0, "sin offsets visual = song_time")
	done()


func test_offsets_audible_visual() -> void:
	var c := SongClockLogic.new()
	c.reset(10.0)
	c.audio_offset = 0.08
	c.visual_offset = 0.03
	c.input_offset = 0.05
	assert_near(c.song_time(), 10.0, 1e-12, "song_time sin offsets")
	assert_near(c.audible(), 10.0 - 0.08, 1e-12, "audible = song_time − audio_offset")
	assert_near(c.visual(), 10.0 - 0.08 + 0.03, 1e-12, "visual = audible + visual_offset (dibujar adelantado)")
	done()


func test_engine_mapping() -> void:
	var c := SongClockLogic.new()
	c.reset(42.0)
	c.audio_offset = 0.07
	c.input_offset = 0.025
	c.visual_offset = 0.5   # no debe influir
	var now := 1234.5
	var m := c.engine_mapping(now)
	assert_near(m, 42.0 - 1234.5 - 0.025 - 0.07, 1e-9, "song_time − now − input_offset − audio_offset")
	# Una muestra tomada "ahora" cae en el tiempo audible menos el retraso de entrada.
	assert_near(now + m, c.audible() - c.input_offset, 1e-9, "t_mono + mapping")
	done()


func test_engine_mapping_constant_while_tracking() -> void:
	# Si audio y reloj monotónico avanzan juntos, el mapping es constante (salvo el suavizado).
	var c := SongClockLogic.new()
	var mono0 := 5000.0
	var true_t := 0.0
	var first := NAN
	var max_diff := 0.0
	for i in range(1200):
		true_t += FPS_DT
		c.step(_blocky(true_t), FPS_DT)
		var m := c.engine_mapping(mono0 + true_t)
		if i == 120:
			first = m
		elif i > 120:
			max_diff = maxf(max_diff, absf(m - first))
	assert_le(max_diff, 0.002, "variación del mapping con audio y reloj al mismo ritmo")
	done()
