# SongClock.gd — autoload "SongClock" (B2, agente D). Reloj maestro = posición del audio que suena.
# Ver docs/02-sincronizacion.md, contracts/clock.h y contracts/game_logic_api.md.
#
# - Tiene su propio AudioStreamPlayer (play/stop/seek/pausa) y también puede seguir a un reproductor
#   de otra escena (attach_player / detach_player, lo usa game/ui/ui_clock.gd).
# - Posición bruta = get_playback_position() + AudioServer.get_time_since_last_mix()
#   − AudioServer.get_output_latency(); se suaviza con SongClockLogic.step().
# - Reloj monotónico = EngineFactory.now_ns() (JDEngine.now_ns() si hay extensión, si no
#   Time.get_ticks_usec()). engine_mapping() es el valor para JDEngine.set_time_mapping():
#   song_time_s(muestra) = t_ns · 1e-9 + engine_mapping().
# - Offsets (audio/visual/input) del Profile (user://profile.cfg) al arrancar.
#
# Sin class_name a propósito: el nombre global "SongClock" es el del autoload.
extends Node

signal song_started
## Solo cuando el audio termina solo (no tras stop()).
signal song_finished

const PROFILE_PATH := "user://profile.cfg"
## Tras seek(), la posición bruta puede seguir dando el valor antiguo unos fotogramas.
const SEEK_SETTLE_TOLERANCE_S := 0.25
const SEEK_SETTLE_MAX_FRAMES := 10

var logic := SongClockLogic.new()
var profile: Profile = null
## Reproductor que se sigue ahora (el propio o uno enganchado con attach_player).
var player: AudioStreamPlayer = null

var audio_offset: float:
	get:
		return logic.audio_offset
	set(value):
		logic.audio_offset = value

var visual_offset: float:
	get:
		return logic.visual_offset
	set(value):
		logic.visual_offset = value

var input_offset: float:
	get:
		return logic.input_offset
	set(value):
		logic.input_offset = value

var _own_player: AudioStreamPlayer = null
var _connected_player: AudioStreamPlayer = null
var _was_playing := false
var _mono_at_step := 0.0
var _seek_pending := false
var _seek_target := 0.0
var _seek_frames := 0


func _ready() -> void:
	# Antes que cualquier escena: quien lea visual()/audible() en su _process ve el valor del fotograma.
	process_priority = -1000
	_own_player = AudioStreamPlayer.new()
	_own_player.name = "Music"
	add_child(_own_player)
	_use_player(_own_player)
	reload_profile()
	_mono_at_step = now_mono_s()


func _process(delta: float) -> void:
	if not is_instance_valid(player):
		_use_player(_own_player)
	var playing_now := player.playing
	if playing_now and not _was_playing:
		song_started.emit()
	_was_playing = playing_now
	if playing_now and not player.stream_paused:
		var raw := raw_position()
		if _seek_pending:
			_seek_frames += 1
			if absf(raw - _seek_target) <= SEEK_SETTLE_TOLERANCE_S or _seek_frames >= SEEK_SETTLE_MAX_FRAMES:
				_seek_pending = false
				logic.step(raw, delta)
			else:
				logic.reset(logic.song_time() + delta)
		else:
			logic.step(raw, delta)
	_mono_at_step = now_mono_s()


# ------------------------------------------------------------------ reproducción

## Reproduce `stream` con el reproductor propio desde from_s (s).
func play(stream: AudioStream, from_s: float = 0.0) -> void:
	_use_player(_own_player)
	_own_player.stream = stream
	_own_player.stream_paused = false
	_own_player.play(from_s)
	_seek_pending = false
	logic.reset(from_s - AudioServer.get_output_latency())
	_mono_at_step = now_mono_s()
	_was_playing = true
	song_started.emit()


func stop() -> void:
	if is_instance_valid(player):
		player.stop()
	_was_playing = false
	_seek_pending = false


func seek(t: float) -> void:
	if not is_instance_valid(player):
		return
	player.seek(t)
	_seek_target = t - AudioServer.get_output_latency()
	_seek_pending = true
	_seek_frames = 0
	logic.reset(_seek_target)


func set_paused(paused: bool) -> void:
	if is_instance_valid(player):
		player.stream_paused = paused


func pause() -> void:
	set_paused(true)


func resume() -> void:
	set_paused(false)


func is_paused() -> bool:
	return is_instance_valid(player) and player.stream_paused


## Suena (y no está en pausa).
func is_playing() -> bool:
	return is_instance_valid(player) and player.playing and not player.stream_paused


## Sigue a un reproductor de otra escena (p. ej. el de la partida). Llamar a reset(0) tras su play().
func attach_player(p: AudioStreamPlayer) -> void:
	if p == null:
		detach_player()
		return
	if is_instance_valid(_own_player) and _own_player.playing:
		_own_player.stop()
	_use_player(p)
	_was_playing = p.playing


## Vuelve al reproductor propio (no detiene el externo).
func detach_player() -> void:
	_use_player(_own_player)
	_was_playing = _own_player.playing


func reset(t: float = 0.0) -> void:
	logic.reset(t)
	_seek_pending = false
	_mono_at_step = now_mono_s()


# ------------------------------------------------------------------ tiempo

## Posición bruta del audio que sale ahora por la tarjeta (sin suavizar, sin offsets).
func raw_position() -> float:
	if not is_instance_valid(player):
		return logic.song_time()
	return player.get_playback_position() + AudioServer.get_time_since_last_mix() - AudioServer.get_output_latency()


## Tiempo suavizado sin offsets.
func song_time() -> float:
	return logic.song_time()


## Tiempo que oye el jugador.
func audible() -> float:
	return logic.audible()


## Tiempo con el que se dibuja.
func visual() -> float:
	return logic.visual()


## Valor para JDEngine.set_time_mapping (song_time = t_mono_s + mapping).
## Mientras suena, la relación canción↔reloj es fija dentro del fotograma, así que se usa el instante
## en que se calculó song_time() y now_mono_s se ignora. Parado o en pausa, la canción está congelada:
## se usa now_mono (o el instante actual si se omite).
func engine_mapping(now_mono: float = NAN) -> float:
	if is_playing():
		return logic.engine_mapping(_mono_at_step)
	if is_nan(now_mono):
		return logic.engine_mapping(now_mono_s())
	return logic.engine_mapping(now_mono)


## Marca de tiempo monotónica (s) → tiempo de canción (con input_offset y audio_offset).
func input_to_song(t_mono_s: float) -> float:
	return t_mono_s + engine_mapping()


## t_ns de una muestra de JDSource.poll_samples → tiempo de canción.
func sample_to_song(t_ns: int) -> float:
	return float(t_ns) * 1e-9 + engine_mapping()


func now_ns() -> int:
	return EngineFactory.now_ns()


func now_mono_s() -> float:
	return float(EngineFactory.now_ns()) * 1e-9


# ------------------------------------------------------------------ perfil

## Copia los offsets de `p` y lo guarda como perfil actual.
func apply_profile(p: Profile) -> void:
	if p == null:
		return
	profile = p
	logic.audio_offset = p.audio_offset
	logic.visual_offset = p.visual_offset
	logic.input_offset = p.input_offset


## Carga el perfil de disco (valores por defecto si falta) y aplica sus offsets.
func reload_profile(path: String = PROFILE_PATH) -> bool:
	var p := Profile.new()
	var ok := p.load(path)
	apply_profile(p)
	return ok


## Escribe los offsets actuales en el perfil y lo guarda.
func save_profile(path: String = PROFILE_PATH) -> Error:
	if profile == null:
		profile = Profile.new()
	profile.audio_offset = logic.audio_offset
	profile.visual_offset = logic.visual_offset
	profile.input_offset = logic.input_offset
	return profile.save(path)


# ------------------------------------------------------------------ internos

func _use_player(p: AudioStreamPlayer) -> void:
	if is_instance_valid(_connected_player) and _connected_player.finished.is_connected(_on_player_finished):
		_connected_player.finished.disconnect(_on_player_finished)
	player = p
	_connected_player = p
	if is_instance_valid(p) and not p.finished.is_connected(_on_player_finished):
		p.finished.connect(_on_player_finished)


func _on_player_finished() -> void:
	_was_playing = false
	song_finished.emit()
