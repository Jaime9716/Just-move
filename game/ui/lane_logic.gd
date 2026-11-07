# lane_logic.gd — lógica pura del carril de iconos (contracts/game_logic_api.md, agente E).
# Sin nodos: T2 la prueba en headless.
class_name LaneLogic
extends RefCounted


## Posición vertical de un icono: llega a `target_y` justo cuando visual_t == step_t.
## Los pasos futuros quedan por encima (y menor) si speed > 0.
static func icon_y(step_t: float, visual_t: float, target_y: float, speed: float) -> float:
	return target_y - (step_t - visual_t) * speed


## Índices de los pasos con t en [visual_t − behind, visual_t + ahead] (ambos incluidos).
## `step_times` debe estar ordenado de forma no decreciente (como exige choreo.json).
static func visible_steps(step_times: PackedFloat64Array, visual_t: float, ahead: float, behind: float) -> PackedInt32Array:
	var out := PackedInt32Array()
	var n := step_times.size()
	if n == 0:
		return out
	var lo_t := visual_t - behind
	var hi_t := visual_t + ahead
	if hi_t < lo_t:
		return out
	# Primer índice con t >= lo_t (búsqueda binaria; bsearch con before=true).
	var i := step_times.bsearch(lo_t, true)
	while i < n and step_times[i] <= hi_t:
		out.append(i)
		i += 1
	return out
