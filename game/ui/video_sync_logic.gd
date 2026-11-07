# video_sync_logic.gd — corrección de deriva del vídeo del bailarín (agente E).
# El vídeo es ESCLAVO de SongClock.visual(): si se separa más de `threshold`, se recoloca.
class_name VideoSyncLogic
extends RefCounted


## Contrato: seek si |video_pos − visual_t| > threshold; target = visual_t (nunca negativo).
static func correction(video_pos: float, visual_t: float, threshold: float = 0.04) -> Dictionary:
	var target := maxf(visual_t, 0.0)
	return {"seek": absf(video_pos - visual_t) > threshold, "target": target}


## Extra (plan B, para cuando el seek de Theora no funciona): decide qué hacer sin seek.
## Devuelve {action: String, amount: float}:
##  "none"   → dentro del umbral;
##  "hold"   → el vídeo va adelantado `amount` s: pausarlo hasta que el reloj lo alcance;
##  "catch"  → el vídeo va atrasado `amount` s (< max_catch): acelerar con speed_scale;
##  "lost"   → atrasado más de max_catch: no se puede recuperar sin seek.
static func plan_b(video_pos: float, visual_t: float, threshold: float = 0.04, max_catch: float = 2.0) -> Dictionary:
	var diff := video_pos - visual_t
	if absf(diff) <= threshold:
		return {"action": "none", "amount": 0.0}
	if diff > 0.0:
		return {"action": "hold", "amount": diff}
	if -diff <= max_catch:
		return {"action": "catch", "amount": -diff}
	return {"action": "lost", "amount": -diff}


## Extra: speed_scale para recuperar `behind` segundos en ~`horizon` segundos (limitado a [1, max_speed]).
static func catch_up_speed(behind: float, horizon: float = 1.0, max_speed: float = 1.25) -> float:
	if behind <= 0.0 or horizon <= 0.0:
		return 1.0
	return clampf(1.0 + behind / horizon, 1.0, max_speed)
