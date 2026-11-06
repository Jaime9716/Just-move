# Contrato · Lógica pura del juego en GDScript (testeable sin escenas)

Godot 4.4, GDScript tipado. El proyecto Godot vive en `game/` (`res://` = `game/`).
La lógica va en clases `RefCounted` sin nodos para que T2 las pruebe en headless.
T2 usa un runner propio sin dependencias: `godot --headless --path game -s res://tests/run_tests.gd`.

## Agente D (`game/core/`)

**`song_clock_logic.gd` — `class_name SongClockLogic`**
- `var audio_offset := 0.0`, `var visual_offset := 0.0`, `var input_offset := 0.0`
- `func step(raw_audio_pos: float, delta: float) -> float` — suavizado de docs/02: salto > 0.1 s → acepta raw;
  si no, `t = max(t, lerp(t + delta, raw, 0.1))`. Devuelve el tiempo suavizado.
- `func reset(t: float = 0.0) -> void`
- `func song_time() -> float` (suavizado, sin offsets) · `func audible() -> float` · `func visual() -> float`
- `func engine_mapping(now_mono_s: float) -> float` — `song_time() − now_mono_s − input_offset − audio_offset`
  (valor para `JDEngine.set_time_mapping`).

`SongClock.gd` (autoload, nodo) usa SongClockLogic con el `AudioStreamPlayer` real y
`JDEngine.now_ns()` (o `Time.get_ticks_usec()` si no hay extensión).

**`calibration_logic.gd` — `class_name CalibrationLogic`**
- `static func estimate_offset(click_times: PackedFloat64Array, event_times: PackedFloat64Array, max_abs: float = 0.3) -> Dictionary`
  Empareja cada evento con el clic más cercano, descarta |diferencia| > max_abs; devuelve
  `{ok: bool, offset: float (mediana, + = el jugador va tarde), stddev: float, n_used: int}`; ok = n_used ≥ 4.
- `static func detect_shakes(samples: Array, threshold: float = 25.0, min_sep: float = 0.15) -> PackedFloat64Array`
  `samples` = diccionarios de `JDSource.poll_samples` con `t_ns` y `a`; devuelve tiempos (s) donde |a| cruza el umbral hacia arriba (sacudida del móvil).

**`profile.gd` — `class_name Profile`**
- Campos: `audio_offset`, `visual_offset`, `input_offset` (s, 0), `perfect_ms` (50), `good_ms` (100),
  `foot_scale` (1.5), `limb_by_device: Dictionary` (nombre de dispositivo → limb int), `phone_port` (47800).
- `func save(path: String = "user://profile.cfg") -> Error` · `func load(path := "user://profile.cfg") -> bool`
  (false y valores por defecto si falta o está corrupto).

## Agente E (`game/ui/`)

**`lane_logic.gd` — `class_name LaneLogic`**
- `static func icon_y(step_t: float, visual_t: float, target_y: float, speed: float) -> float`
  = `target_y − (step_t − visual_t)·speed`.
- `static func visible_steps(step_times: PackedFloat64Array, visual_t: float, ahead: float, behind: float) -> PackedInt32Array`

**`video_sync_logic.gd` — `class_name VideoSyncLogic`**
- `static func correction(video_pos: float, visual_t: float, threshold: float = 0.04) -> Dictionary`
  → `{seek: bool, target: float}`; seek si |video_pos − visual_t| > threshold, target = visual_t (≥ 0).

**`score_logic.gd` — `class_name ScoreLogic`**
- Puntos: Perfect 100, Good 50, Miss 0; combo +1 en Perfect/Good, a 0 en Miss;
  bonus = puntos × (1 + min(combo, 50)·0.01) redondeado.
- `func add(grade: int) -> int` (devuelve puntos sumados) · `var score`, `var combo`, `var max_combo`,
  `var counts := {0:0, 1:0, 2:0}` · `func reset()`.
