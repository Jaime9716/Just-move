# song_clock_logic.gd — lógica pura del reloj de canción (B2). Sin nodos: se prueba en headless.
# Ver docs/02-sincronizacion.md y contracts/game_logic_api.md.
class_name SongClockLogic
extends RefCounted

## Si la posición bruta se aleja más que esto de la predicha, se acepta tal cual (seek, inicio).
const JUMP_THRESHOLD_S := 0.1
## Fracción con la que la predicción se acerca a la posición bruta en cada paso.
const SMOOTHING := 0.1

var audio_offset := 0.0   ## s; retraso del audio que no cubre get_output_latency (calibración)
var visual_offset := 0.0  ## s; se dibuja adelantado este tiempo (retraso de la pantalla)
var input_offset := 0.0   ## s; retraso de la entrada (móvil, Wi-Fi, procesado)

var _t := 0.0


## Avanza el reloj con la posición bruta del audio que suena ahora y el delta del fotograma.
## Devuelve el tiempo suavizado.
func step(raw_audio_pos: float, delta: float) -> float:
	var predicted := _t + delta
	if absf(raw_audio_pos - predicted) > JUMP_THRESHOLD_S:
		_t = raw_audio_pos
	else:
		_t = maxf(_t, lerpf(predicted, raw_audio_pos, SMOOTHING))
	return _t


func reset(t: float = 0.0) -> void:
	_t = t


## Tiempo de canción suavizado, sin offsets.
func song_time() -> float:
	return _t


## Tiempo que oye el jugador.
func audible() -> float:
	return _t - audio_offset


## Tiempo con el que se dibuja (adelantado lo que tarda la pantalla).
func visual() -> float:
	return audible() + visual_offset


## Valor para JDEngine.set_time_mapping: song_time_s = t_mono_s + mapping.
## now_mono_s debe tomarse en el mismo instante en que se calculó song_time().
func engine_mapping(now_mono_s: float) -> float:
	return _t - now_mono_s - input_offset - audio_offset
