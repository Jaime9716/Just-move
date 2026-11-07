# requires: LaneLogic
# Tests de caja negra de LaneLogic (contracts/game_logic_api.md, docs/06 fila B8: posición exacta).
extends "res://tests/test_base.gd"


func test_icon_y_formula() -> void:
	# target_y − (step_t − visual_t)·speed
	assert_eq(LaneLogic.icon_y(10.0, 8.0, 600.0, 200.0), 200.0, "paso 2 s en el futuro")
	assert_eq(LaneLogic.icon_y(10.0, 10.0, 600.0, 200.0), 600.0, "en la diana justo en su instante")
	assert_eq(LaneLogic.icon_y(5.0, 6.0, 600.0, 100.0), 700.0, "paso pasado: por debajo de la diana")
	assert_eq(LaneLogic.icon_y(3.0, 1.0, 0.0, -50.0), 100.0, "speed negativa invierte el sentido")
	done()


func test_icon_y_exact_many() -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = 5
	var worst := 0.0
	for i in range(1000):
		var st := rng.randf_range(0.0, 300.0)
		var vt := rng.randf_range(-2.0, 300.0)
		var ty := rng.randf_range(0.0, 1080.0)
		var sp := rng.randf_range(50.0, 1200.0)
		var expected := ty - (st - vt) * sp
		worst = maxf(worst, absf(LaneLogic.icon_y(st, vt, ty, sp) - expected))
	assert_le(worst, 1e-9, "error máximo frente a la fórmula")
	done()


func test_icon_y_moves_linearly_with_time() -> void:
	# Con speed px/s, 1/60 s de tiempo visual mueve el icono speed/60 px hacia la diana.
	var y0 := LaneLogic.icon_y(4.0, 3.0, 500.0, 300.0)
	var y1 := LaneLogic.icon_y(4.0, 3.0 + 1.0 / 60.0, 500.0, 300.0)
	assert_near(y1 - y0, 300.0 / 60.0, 1e-9)
	done()


func test_visible_steps_window() -> void:
	var times := PackedFloat64Array([0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
	var r := LaneLogic.visible_steps(times, 2.5, 1.2, 0.7)   # [1,8 ; 3,7]
	assert_type(r, TYPE_PACKED_INT32_ARRAY)
	assert_eq(r, PackedInt32Array([2, 3]), "pasos dentro de [visual − behind, visual + ahead]")
	assert_eq(LaneLogic.visible_steps(times, 10.0, 2.0, 1.0), PackedInt32Array(), "después del último")
	assert_eq(LaneLogic.visible_steps(times, -5.0, 2.0, 1.0), PackedInt32Array(), "antes del primero")
	assert_eq(LaneLogic.visible_steps(times, 3.0, 100.0, 100.0), PackedInt32Array([0, 1, 2, 3, 4, 5, 6]), "todo")
	done()


func test_visible_steps_ahead_and_behind_are_independent() -> void:
	var times := PackedFloat64Array([0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
	assert_eq(LaneLogic.visible_steps(times, 3.0, 2.5, 0.2), PackedInt32Array([3, 4, 5]), "mucho delante, poco detrás")
	assert_eq(LaneLogic.visible_steps(times, 3.0, 0.2, 2.5), PackedInt32Array([1, 2, 3]), "poco delante, mucho detrás")
	done()


func test_visible_steps_bounds_inclusive() -> void:
	# Decisión de E (el contrato no lo fija): los extremos de la ventana se incluyen.
	var times := PackedFloat64Array([0.0, 1.0, 2.0, 3.0, 4.0])
	assert_eq(LaneLogic.visible_steps(times, 2.0, 1.0, 1.0), PackedInt32Array([1, 2, 3]))
	done()


func test_visible_steps_empty_and_duplicates() -> void:
	assert_eq(LaneLogic.visible_steps(PackedFloat64Array(), 1.0, 2.0, 2.0), PackedInt32Array(), "sin pasos")
	var times := PackedFloat64Array([1.0, 2.0, 2.0, 2.0, 3.5])
	assert_eq(LaneLogic.visible_steps(times, 2.1, 0.5, 0.5), PackedInt32Array([1, 2, 3]),
			"pasos simultáneos (mismo t) salen todos")
	done()


func test_visible_steps_matches_brute_force() -> void:
	var rng := RandomNumberGenerator.new()
	rng.seed = 77
	var times := PackedFloat64Array()
	var t := 0.0
	for i in range(400):
		t += rng.randf_range(0.0, 0.8)     # incluye pasos casi simultáneos
		times.append(t)
	var mismatches := 0
	for k in range(300):
		var vt := rng.randf_range(-5.0, t + 5.0)
		var ahead := rng.randf_range(0.1, 4.0)
		var behind := rng.randf_range(0.05, 1.0)
		var expected := PackedInt32Array()
		for i in range(times.size()):
			# Lejos de los bordes (> 1e-9) para no depender de si se incluyen.
			if times[i] > vt - behind + 1e-9 and times[i] < vt + ahead - 1e-9:
				expected.append(i)
		var got := LaneLogic.visible_steps(times, vt, ahead, behind)
		# Quita de got los que caen justo en el borde (±1e-9), si los hubiera.
		var trimmed := PackedInt32Array()
		for i: int in got:
			if times[i] > vt - behind + 1e-9 and times[i] < vt + ahead - 1e-9:
				trimmed.append(i)
		if trimmed != expected or got.size() > expected.size() + 2:
			mismatches += 1
	assert_eq(mismatches, 0, "ventanas aleatorias que no coinciden con la búsqueda lineal")
	done()
