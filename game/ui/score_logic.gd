# score_logic.gd — puntuación y combo (contracts/game_logic_api.md, agente E).
# Grados del motor: 0 Miss, 1 Good, 2 Perfect.
# Decisión: el bonus usa el combo YA incrementado por este acierto (primer Perfect = 101 puntos).
class_name ScoreLogic
extends RefCounted

const GRADE_MISS := 0
const GRADE_GOOD := 1
const GRADE_PERFECT := 2
const POINTS := {0: 0, 1: 50, 2: 100}
const MAX_COMBO_BONUS := 50

var score: int = 0
var combo: int = 0
var max_combo: int = 0
var counts := {0: 0, 1: 0, 2: 0}


## Suma un resultado y devuelve los puntos sumados (con bonus de combo).
## Un grado desconocido cuenta como Miss.
func add(grade: int) -> int:
	if not POINTS.has(grade):
		grade = GRADE_MISS
	counts[grade] = int(counts[grade]) + 1
	if grade == GRADE_MISS:
		combo = 0
		return 0
	combo += 1
	if combo > max_combo:
		max_combo = combo
	var base: int = POINTS[grade]
	var pts := int(roundf(float(base) * (1.0 + float(mini(combo, MAX_COMBO_BONUS)) * 0.01)))
	score += pts
	return pts


func reset() -> void:
	score = 0
	combo = 0
	max_combo = 0
	counts = {0: 0, 1: 0, 2: 0}


## Total de resultados juzgados.
func total() -> int:
	return int(counts[0]) + int(counts[1]) + int(counts[2])
