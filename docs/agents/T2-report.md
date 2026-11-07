# Informe del agente T2 · Tests de bajo nivel del juego (Godot)

Todo en `game/tests/`. GDScript tipado para Godot 4.4. **No se ha podido ejecutar**: no hay Godot en el
entorno. Revisado a mano dos veces. Los números de los tests de suavizado se han comprobado antes con una
simulación en Python de la misma fórmula (doble precisión, como GDScript).

## Qué hay

| Archivo | Contenido |
|---|---|
| `run_tests.gd` | Runner (`extends SceneTree`), sin dependencias ni referencias a clases del juego. Descubre `res://tests/test_*.gd` con `DirAccess`, ejecuta `test_*` (con `get_method_list`, en orden) y después los `step_test_*` uno por fotograma. `# requires: A, B` en un archivo → SKIP si falta la clase (mira `ProjectSettings.get_global_class_list()` y `ClassDB`). Archivo que no compila o test que no llega a `done()` (error de script) = FALLO. `quit(1)` si algo falla, `quit(0)` si no. Sin `await`; límite global de 100 000 fotogramas / 300 s y `max_frames()` por test por fotogramas; fija `Engine.max_fps = 120` si no hay límite. Si no hay ninguna clase global (proyecto sin importar) falla con un mensaje claro en vez de dar todo por SKIP. `-- --filter=texto`, `-- --verbose`. Borra `user://t2_tests/` al acabar. |
| `test_base.gd` | Base: `assert_true/false/eq/ne/near/lt/le/gt/ge/null/not_null/type/has/has_method`, `fail`, `skip`, `done`, `before_each/after_each`, `max_frames`, carpetas temporales en `user://t2_tests/`. |
| `test_song_clock_logic.gd` | SongClockLogic (16 tests). |
| `test_calibration_logic.gd` | CalibrationLogic.estimate_offset / detect_shakes (16). |
| `test_profile.gd` | Profile (12), solo en `user://t2_tests/profile/` (nunca `user://profile.cfg`). |
| `test_lane_logic.gd` | LaneLogic (8). |
| `test_video_sync_logic.gd` | VideoSyncLogic.correction (4). |
| `test_score_logic.gd` | ScoreLogic (7). |
| `test_engine_mocks.gd` | MockJDEngine / MockJDSource / EngineFactory (13), incluidos 2 que corren contra lo que devuelva la fábrica (nativo en CI). |
| `test_native_extension.gd` | Interfaz de JDEngine/JDSource en ClassDB; SKIP sin la extensión (3). |
| `test_song_library.gd` | SongLibrary con paquetes temporales en `user://t2_tests/songs` (6). |
| `test_songclock_autoload.gd` | Humo MARCADO del autoload SongClock (parado): offsets, `engine_mapping`, `input_to_song`, `sample_to_song` (3). SKIP si no está el autoload. |
| `test_scenes_smoke.gd` | Humo MARCADO de escenas (2, por fotogramas): `main_menu` 30 fotogramas; `gameplay` con `force_mock`, simulación como fuente y `data/songs/demo_metronomo`, 3,5 s reales (pasa la cuenta atrás de 2 s y llega a PLAYING, nunca FAILED). SKIP sin autoload o sin la demo. |
| `README.md` | Cómo ejecutar en Windows (`godot.exe` / `_console.exe`, `--import` previo) y cómo escribir tests. |

## Matriz requisito → test

| Requisito (contrato / docs/06) | Test(s) |
|---|---|
| SongClockLogic: salto > 0,1 s acepta raw (adelante y seek atrás) | `test_jump_forward_accepts_raw`, `test_jump_backward_accepts_raw_seek`, `test_jump_threshold_is_0_1`, `test_seek_then_smooth_again` |
| SongClockLogic: `t = max(t, lerp(t+delta, raw, 0,1))` | `test_smoothing_formula`, `test_jump_threshold_is_0_1` |
| Nunca retrocede salvo seek | `test_never_goes_back_with_small_backward_raw`, `test_monotonic_with_noisy_raw`, y en cada test de 180 s |
| Error del suavizado ≤ 2 ms, sin deriva (bloques de 512 a 48 kHz, 60 fps, 180 s) | `test_smoothing_blocks_48k_60fps_180s`, `_59_94fps_180s`, `_60fps_jitter_180s` (ver nota 1) |
| reset / song_time / step devuelve el suavizado | `test_reset_and_song_time`, `test_step_returns_song_time` |
| Offsets: audible, visual | `test_offsets_default_zero`, `test_offsets_audible_visual`, `test_offsets_from_profile` (autoload) |
| `engine_mapping = song_time − now − input − audio` | `test_engine_mapping`, `test_engine_mapping_constant_while_tracking`, `test_input_to_song_stopped` (autoload, "input_to_song exacto" de docs/06) |
| estimate_offset: offset conocido con ruido | `test_known_offset_with_noise`, `test_player_early_gives_negative_offset` (signo), `test_unsorted_events` |
| estimate_offset: mediana, outliers (`max_abs`) | `test_median_even_count`, `test_outliers_discarded`, `test_max_abs_parameter`, `test_median_robust_to_inlier_outliers` |
| estimate_offset: < 4 eventos → ok false; = 4 → ok | `test_less_than_4_events_not_ok`, `test_empty_inputs`, `test_median_even_count` |
| detect_shakes con señales sintéticas (200 Hz) | `test_detect_shakes_basic`, `_time_base_seconds_from_t_ns`, `_rest_gives_nothing`, `_threshold_param`, `_held_counts_once`, `_min_sep`, `test_detect_shakes_feeds_estimate_offset` (punta a punta) |
| Profile: ida y vuelta exacta (B9) | `test_round_trip_exact` (incluye 0,1 + 0,2 y limb_by_device con UTF-8), `test_round_trip_twice_stable`, `test_default_values_round_trip` |
| Profile: ausente / corrupto → false + valores por defecto | `test_missing_file`, `test_corrupt_not_json`, `_empty`, `_wrong_root_type`, `_wrong_field_type`, `_binary_garbage`, `test_load_after_modification_resets_missing_keys` |
| Profile: phone_port 47800 | `test_defaults`, `test_phone_port_default_and_custom` |
| LaneLogic.icon_y exacta (B8) | `test_icon_y_formula`, `test_icon_y_exact_many`, `test_icon_y_moves_linearly_with_time` |
| LaneLogic.visible_steps | `test_visible_steps_window`, `_ahead_and_behind_are_independent`, `_bounds_inclusive` (decisión de E), `_empty_and_duplicates`, `_matches_brute_force` |
| VideoSyncLogic: umbral por defecto 0,04 y explícito; target = visual ≥ 0 | `test_default_threshold_is_0_04`, `test_explicit_threshold`, `test_target_never_negative`, `test_target_is_visual_t` |
| ScoreLogic: puntos, combo, bonus (tope 50), counts, reset | `test_initial_state`, `test_base_points_with_bonus`, `test_miss_resets_combo_and_gives_zero`, `test_add_returns_points_and_score_accumulates`, `test_bonus_capped_at_50`, `test_reset`, `test_counts_keys_are_ints` |
| Mocks con la interfaz de gdextension_api.md, sin extensión | `test_mock_engine_interface`, `test_mock_source_interface`, `test_mock_phone_server`, `test_mock_replay_*`, `test_mock_recorder_writes_session`, `test_mock_engine_smoke`, `test_mock_engine_pump_mock_source`, `test_mock_engine_load_song_errors_have_json_path` |
| EngineFactory: mock forzado, = ClassDB, reloj | `test_factory_forced_mock`, `test_factory_matches_classdb`, `test_factory_clock` |
| "Cada simulación pasa los mismos tests que la real" (docs/06) | `test_factory_engine_smoke` (mismo `_engine_smoke` que el mock; en CI con el motor nativo), `test_mock_has_native_methods`, `test_native_classes_methods`, `test_native_statics` |
| SongLibrary con paquete temporal | `test_load_valid_package`, `test_video_optional`, `test_missing_audio_invalid`, `test_invalid_packages_have_clear_errors` (ruta JSON en el mensaje), `test_scan_finds_temp_package`, `test_limb_names` |
| Humo de escenas | `step_test_main_menu`, `step_test_gameplay` |

**Nota 1 (criterio de 2 ms).** Con la posición a bloques de 512 muestras, el valor bruto va de media medio
bloque (5,33 ms) por detrás del audio real, y el suavizado converge a ese sesgo constante: medido contra el
audio real el error es ~5–6 ms, nunca ≤ 2 ms. El test exige lo que el suavizado sí controla:
|t − (t_real − medio bloque)| ≤ 2 ms (simulado: 0,91 ms a 60 fps, 0,71 ms a 59,94, ~1,2 ms con ±5 % de
jitter), sesgo medio ≤ medio bloque + 1 ms, deriva entre los primeros y los últimos 10 s ≤ 1 ms
(simulado 0,46 ms) y cero retrocesos. En el juego real `SongClock.gd` suma `get_time_since_last_mix()`,
que quita casi todo ese sesgo; el resto lo absorbe la calibración.

## Discrepancias con el contrato vistas en el código de D/E (no arregladas)

1. **ScoreLogic, bonus**: el contrato no dice si `combo` es el de antes o el de después del acierto. E usa
   el de después (primer Perfect = 101). Los tests fijan esa interpretación. Propuesta: escribirlo en
   `game_logic_api.md` (E también lo pide).
2. **VideoSyncLogic**: la clase respeta el 0,04 por defecto, pero `gameplay.gd` la llama con 0,07
   (`VIDEO_THRESHOLD_S`) por la cuantización de Theora. No es una violación (el umbral es un parámetro),
   pero conviene documentarlo.
3. **MockJDSource.start()** (D) pone `_last_error = ""` antes de comprobar si la carga del replay falló,
   así que un `.jsonl` corrupto arranca con `start() == true` y `last_error() == ""` (y no emite nada). El
   contrato solo fija null si el archivo no existe. No hay test que lo exija; arreglo sugerido: no borrar
   `_last_error` en `start()` para `KIND_REPLAY`.
4. **CalibrationLogic.stddev**: D usa la desviación poblacional (÷ n). El contrato no lo fija; el test
   acepta las dos (tolerancia 2,5 ms).
5. **LaneLogic.visible_steps**: extremos incluidos (decisión de E, no está en el contrato). Hay un test
   que lo fija (`test_visible_steps_bounds_inclusive`); si el contrato decide otra cosa, cambiar ese test.
6. **SongClock.engine_mapping(now)** (autoload, fuera del contrato): mientras suena ignora el argumento
   y usa el instante del último `step`. Es a propósito (informe de D) pero difiere de
   `SongClockLogic.engine_mapping(now)`; los tests del autoload solo lo prueban parado.
7. **Profile.load** se llama igual que la función global `load()` (D ya lo marca). Si en Godot 4.4 fuera
   un error y no un aviso, `test_profile.gd` (y todo lo que usa Profile) saldría "no compila": la
   propuesta de D (`load_from`) obligaría a cambiar el contrato y estos tests.
8. Menor: `SongClock.gd` todavía menciona `ui/ui_clock.gd`, que E borró (E ya lo indica).

## Qué no se ha podido verificar

- Nada se ha ejecutado (sin Godot). Puntos dudosos de sintaxis o comportamiento 4.4 a mirar en la primera CI:
  - `load()` de un test que no compila: espero un `GDScript` con `can_instantiate() == false` (o null);
    las dos cosas se tratan como "no compila".
  - Un error de script dentro de un test hace que `call()` devuelva null y el runner sigue; por eso el
    test debe llamar a `done()`.
  - `ProjectSettings.get_global_class_list()` con `-s` (debería leer `.godot/global_script_class_cache.cfg`).
  - Autoload `SongClock` con `-s`: en 4.x se añade al árbol; si no, sus tests y los de escenas salen SKIP.
  - Audio en headless (driver Dummy): `gameplay` puede quedarse en PLAYING o acabar pronto y cambiar a
    `results`; el test acepta ambas cosas si llegó a PLAYING.
  - `test_mock_phone_server` / replay / grabador usan `OS.delay_msec(20–60)`: dependen de tiempo real,
    con márgenes amplios.
  - `test_factory_engine_smoke` con el motor nativo espera que `update(1000)` sin muestras dé 4 Miss y
    que `reset()` vuelva a emitirlos. Si el juez nativo no emite Miss sin datos, ese test lo dirá.
- La prueba de humo no puede capturar los `ERROR:` del registro (Godot 4.4 no tiene `Logger`): revisar la
  salida a ojo.

## Cómo probarlo

```
godot --headless --path game --import          # una vez
godot --headless --path game -s res://tests/run_tests.gd
```
En Windows: `game/tests/README.md`. En CI: job `godot` (ya lo llama si existe `run_tests.gd`).
