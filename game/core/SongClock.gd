# SongClock.gd  (autoload)
# Reloj maestro basado en la posición real del audio. Ver docs/02-sincronizacion.md
extends Node

var music: AudioStreamPlayer
var audio_offset := 0.0   # calibración de audio (s)
var visual_offset := 0.0  # calibración de pantalla (s)
var input_offset := 0.0   # latencia de los Joy-Con (s)

var _t := 0.0             # tiempo de canción, suavizado
var _mono_to_song := 0.0  # diferencia entre reloj del sistema y de la canción

func _process(delta):
	if not music or not music.playing:
		return
	# Posición real del audio que SUENA ahora mismo
	var raw := music.get_playback_position() \
		+ AudioServer.get_time_since_last_mix() \
		- AudioServer.get_output_latency()

	var predicted := _t + delta
	if abs(raw - predicted) > 0.1:
		_t = raw                                   # salto grande (seek o inicio)
	else:
		_t = max(_t, lerp(predicted, raw, 0.1))    # suavizar sin retroceder

	_mono_to_song = _t - Time.get_ticks_usec() / 1e6

## Tiempo que oye el jugador
func audible() -> float:
	return _t - audio_offset

## Tiempo con el que se dibuja (adelantado lo que tarda la pantalla)
func visual() -> float:
	return audible() + visual_offset

## Marca de tiempo de una muestra (reloj monotónico, s) → tiempo de canción
func input_to_song(t_mono: float) -> float:
	return t_mono - input_offset + _mono_to_song - audio_offset
