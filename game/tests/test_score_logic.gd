# requires: ScoreLogic
# Tests de caja negra de ScoreLogic (contracts/game_logic_api.md).
# Grados: 0 Miss, 1 Good, 2 Perfect. Bonus = puntos × (1 + min(combo, 50)·0,01) redondeado.
# El contrato no dice si "combo" es el de antes o el de después del acierto; E usa el de DESPUÉS
# (primer Perfect = 101) y estos tests lo fijan así (ver docs/agents/T2-report.md).
extends "res://tests/test_base.gd"

const MISS := 0
const GOOD := 1
const PERFECT := 2


static func _expected(base: int, combo_after: int) -> int:
	return int(roundf(float(base) * (1.0 + float(mini(combo_after, 50)) * 0.01)))


func _assert_counts(s: ScoreLogic, miss: int, good: int, perfect: int, msg: String) -> void:
	assert_eq(int(s.counts.get(0, -1)), miss, msg + ": counts[0] (Miss)")
	assert_eq(int(s.counts.get(1, -1)), good, msg + ": counts[1] (Good)")
	assert_eq(int(s.counts.get(2, -1)), perfect, msg + ": counts[2] (Perfect)")


func test_initial_state() -> void:
	var s := ScoreLogic.new()
	assert_eq(s.score, 0)
	assert_eq(s.combo, 0)
	assert_eq(s.max_combo, 0)
	_assert_counts(s, 0, 0, 0, "inicio")
	done()


func test_base_points_with_bonus() -> void:
	var s := ScoreLogic.new()
	assert_eq(s.add(PERFECT), 101, "1.er Perfect: 100 × 1,01")
	assert_eq(s.add(PERFECT), 102, "2.º Perfect: 100 × 1,02")
	assert_eq(s.add(PERFECT), 103)
	assert_eq(s.add(GOOD), 52, "Good con combo 4: 50 × 1,04")
	assert_eq(s.score, 101 + 102 + 103 + 52)
	assert_eq(s.combo, 4)
	done()


func test_miss_resets_combo_and_gives_zero() -> void:
	var s := ScoreLogic.new()
	s.add(PERFECT)
	s.add(GOOD)
	s.add(PERFECT)
	var before := s.score
	assert_eq(s.add(MISS), 0, "Miss suma 0")
	assert_eq(s.combo, 0, "Miss → combo 0")
	assert_eq(s.max_combo, 3, "max_combo se conserva")
	assert_eq(s.score, before, "Miss no cambia la puntuación")
	assert_eq(s.add(PERFECT), 101, "tras Miss el bonus vuelve a empezar")
	assert_eq(s.combo, 1)
	_assert_counts(s, 1, 1, 3, "tras la secuencia")
	done()


func test_add_returns_points_and_score_accumulates() -> void:
	var s := ScoreLogic.new()
	var seq := [PERFECT, GOOD, MISS, GOOD, GOOD, PERFECT, MISS, MISS, PERFECT, PERFECT, GOOD]
	var combo := 0
	var total := 0
	var best := 0
	for g: int in seq:
		var base := 0
		if g == MISS:
			combo = 0
		else:
			combo += 1
			base = 100 if g == PERFECT else 50
		best = maxi(best, combo)
		var want := 0 if g == MISS else _expected(base, combo)
		var got := s.add(g)
		assert_eq(got, want, "add(%d) con combo %d" % [g, combo])
		total += got
	assert_eq(s.score, total, "score = suma de lo devuelto por add")
	assert_eq(s.combo, combo, "combo final")
	assert_eq(s.max_combo, best, "max_combo")
	_assert_counts(s, 3, 4, 4, "conteos")
	done()


func test_bonus_capped_at_50() -> void:
	var s := ScoreLogic.new()
	var last := 0
	for i in range(60):
		last = s.add(PERFECT)
		if i + 1 == 49:
			assert_eq(last, 149, "combo 49")
		if i + 1 == 50:
			assert_eq(last, 150, "combo 50: bonus máximo")
	assert_eq(last, 150, "combo 60: sigue en 150")
	assert_eq(s.combo, 60)
	assert_eq(s.max_combo, 60)
	assert_eq(s.add(GOOD), 75, "Good con combo 61: 50 × 1,5")
	done()


func test_reset() -> void:
	var s := ScoreLogic.new()
	for g: int in [PERFECT, PERFECT, MISS, GOOD]:
		s.add(g)
	s.reset()
	assert_eq(s.score, 0)
	assert_eq(s.combo, 0)
	assert_eq(s.max_combo, 0)
	_assert_counts(s, 0, 0, 0, "tras reset")
	assert_eq(s.add(PERFECT), 101, "tras reset vuelve a empezar")
	done()


func test_counts_keys_are_ints() -> void:
	var s := ScoreLogic.new()
	s.add(GOOD)
	assert_true(s.counts.has(1), "clave int 1")
	assert_eq(s.counts.size(), 3, "solo las claves 0, 1, 2")
	done()
