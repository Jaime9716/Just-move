# Informe del agente T1 · Pruebas de bajo nivel del motor y del prototipo

Son pruebas **de caja negra e independientes**. Solo usan las APIs públicas (`jd_core`, la API C `jd_engine`,
`jdproto` y los bins) y datos deterministas generados por T1 (generador LCG con semillas fijas, señales analíticas).
Las únicas excepciones son los golden y las sesiones de B, que se usan solo para la paridad y para la comparación con los golden.

## Cómo ejecutarlo

```
cd engine && CARGO_TARGET_DIR=../build/T1 cargo test                       # todo el workspace
CARGO_TARGET_DIR=../build/T1 cargo test -p jd_core -p jd_capi -- --ignored  # solo los BUG documentados (fallan)
CARGO_TARGET_DIR=../build/T1 cargo test --release -p jd_core --test t1_robustness perf -- --nocapture
cd .. && PYTHONPATH=proto python -m pytest proto/tests -q                    # compila jd_judge en build/T1
```

Resultado aquí: workspace en verde. T1 Rust: 65 pasan y 3 ignorados (`BUG`). pytest: **62 pasan**, sin xfail.
`cargo build --tests` no da warnings. `t1_capi` tarda unos 32 s porque reproduce las 9 sesiones en tiempo real y en paralelo.

## Archivos

| Archivo | Contenido |
|---|---|
| `engine/jd_core/tests/t1_util.rs` | Generadores: golpes gaussianos, *move* no periódico, reposo con ruido, LCG; carga de golden |
| `engine/jd_core/tests/t1_signal.rs` | B3 |
| `engine/jd_core/tests/t1_features.rs` | B4 |
| `engine/jd_core/tests/t1_judge.rs` | B5 v1.1 (offline y tiempo real) |
| `engine/jd_core/tests/t1_choreo.rs` | B6 |
| `engine/jd_core/tests/t1_session.rs` | B7: sesiones, replay con reloj falso, recorder |
| `engine/jd_core/tests/t1_protocol.rs` | Vectores Kotlin↔Rust, codificador independiente, basura |
| `engine/jd_core/tests/t1_robustness.rs` | Robustez, determinismo, golden por `judge_offline`, rendimiento |
| `engine/jd_capi/tests/t1_capi.rs` | API C de punta a punta |
| `proto/tests/conftest.py`, `test_t1_api.py`, `test_t1_determinism.py`, `test_t1_parity.py` | Python y paridad Python↔Rust |

## Matriz requisito → test

| Requisito (docs/06 + contratos) | Test(s) | Estado |
|---|---|---|
| B3 reposo estable, lin_accel ≈ 0 | `t1_signal::rest_upright_is_stable_and_lin_accel_zero`, `rest_tilted_lin_accel_zero_from_first_sample` | OK (\|lin\| < 0.15 con ruido 0.05; < 1e-3 inclinado sin ruido) |
| B3 giro de 90° ±5° | `yaw_90_degrees_within_5`, `roll_90_degrees_with_coherent_gravity_within_5` | OK |
| B3 aceleración lineal conocida, LPF exacto, t_song = t_ns·1e-9 + mapping, estado por extremidad, limb none | `known_linear_acceleration_outside_gravity_gate`, `lowpass_step_matches_contract_formula`, `mapping_is_added_to_time`, `limbs_have_independent_state`, `limb_none_is_ignored` | OK |
| B4 tramas en k·0.02, sin huecos, sin tramas vacías, emisión tras cerrar la ventana | `frames_are_aligned_to_hop_and_ordered`, `gap_produces_no_empty_frames`, `frame_emitted_only_after_window_closes` | OK |
| B4 energy/peak/dir/quat según la definición | `frame_stats_match_definition` | OK |
| B4 pico a ±1 muestra (filtrada) y ±10 ms del golpe | `peak_within_one_sample_of_filtered_max_and_10ms_of_truth` | OK |
| B4 PEAK_MIN, PEAK_REL, separación 150 ms (gana el mayor) | `below_peak_min_is_not_a_peak`, `peak_rel_rejects_...`, `min_separation_keeps_the_larger` | OK |
| B4 streaming = offline | `streaming_in_chunks_equals_offline` (trozos 1/3/16/100) | OK |
| B5 accent 0/±40/±80/±150 ms (mano) | `t1_judge::accent_hands_on_time_late_and_early`, `test_t1_api::test_accent_hand` | OK: 0/40 → P, 80 → G, 150 → M |
| B5 pies ×1.5 | `accent_feet_windows_scaled_1_5`, `test_accent_feet_scale` | OK (60 ms: mano G/pie P; 120 ms: M/G) |
| B5 similarity / timing_error del accent, pico más cercano, config propia | `accent_*` | OK |
| B5 move idéntico → Perfect | `move_identical_is_perfect` (mano y pie) | OK (sim > 0.99, L* = 0) |
| B5 move +40 → P, +80 → G, ±150 → **Miss** (v1.1), pie +140 → G, +220 → M | `move_shifted`, `test_move_shifts` | OK |
| B5 quieto / débil (puerta de energía) / sin datos → Miss | `move_still_is_miss_and_weak_is_gated`, `test_move_still_weak_missing` | OK |
| B5 sin referencia nunca Perfect; referencia solo de otra extremidad | `move_without_reference_never_perfect`, `test_move_noref_never_perfect` | OK |
| B5 orden (paso, extremidad) offline | `result_order_step_then_limb`, `test_order_step_then_limb` | OK |
| B5 momento de emisión t+dur+2·gw+0.05 | `emission_time_matches_contract_offline`, `emission_without_data_is_exact_contract_time`, `test_emit_time_v11` | OK |
| B5 tiempo real: ≤ contrato + 0.5 s, mismas notas que offline | `realtime_emission_within_contract_plus_0_5s_and_same_grades` | OK |
| B5 orden de emisión **en tiempo real** | `realtime_emission_order_with_waiting_limb` | **BUG 1** (ignorado) |
| B6 válidos cargan (incluidos los paquetes del repo) | `t1_choreo::valid_minimal_parses_fields`, `repo_packages_are_valid` | OK |
| B6 inválidos con ruta JSON (16 casos), corruptos/vacíos con archivo y línea | `invalid_cases_report_json_path`, `corrupt_and_empty_files_are_rejected_clearly`, `validate_choreo_on_struct`, `test_load_choreo_errors`, `test_load_choreo_corrupt` | OK |
| B7 ida y vuelta exacta (2000 muestras aleatorias, flags parciales, f32 extremos, texto con escapes) y bytes idénticos al volver a guardar | `save_load_roundtrip_is_exact`, `synthetic_sessions_roundtrip` | OK (ver la observación sobre −0.0) |
| B7 reglas del formato y errores con línea | `parse_rules_from_contract`, `invalid_sessions_rejected_with_line`, `test_load_session_errors` | OK |
| B7 replay con reloj falso (velocidad 1 y 2): t_ns, orden, nada antes de start ni tras stop, assign_limb | `replay_with_fake_clock_...`, `replay_assign_limb_and_stop` | OK |
| B7 grabar y reproducir en tiempo de canción | `record_replay_roundtrip_in_song_time`, `t1_capi::recorder_via_capi_writes_song_time` | OK |
| Protocolo: 19 vectores de `android/.../protocol_vectors.json` decodificados con Rust y recodificados (con `encode` y con un codificador escrito desde el contrato), i64 extremos leídos del texto | `t1_protocol::android_vectors_decode_and_reencode` | OK, los 8 tipos |
| Protocolo: disposición de 36 bytes LE, truncados, magic/versión/tipo/longitud malos, 20 000 paquetes basura, nombre no UTF-8 | `sample_layout_is_36_bytes_le`, `malformed_packets_are_errors_not_panics`, `invalid_utf8_name_does_not_panic` | OK |
| Robustez: vacíos (sesión, choreo, sin pasos), referencia ausente | `empty_inputs`, `reference_file_missing_is_an_error` | OK |
| Robustez: NaN/inf en muestras | `nan_samples_do_not_panic_through_engine_push` (sin pánico: OK); `nan_and_inf_samples_do_not_break_the_engine` | **BUG 2** (ignorado) |
| Robustez: saltos hacia atrás, desordenadas, duplicadas, t_ns extremos, limb 77, flags 0, update NaN/atrás | `time_jumps_backwards_and_unordered_samples`, `absurd_timestamps_and_limbs`, `update_with_nan_or_backwards_time_is_harmless`, `test_robust_unordered_and_backwards` | OK (sin pánico, 1 resultado por paso y extremidad, valores finitos) |
| Determinismo bit a bit (offline ×2, trozos 1/8/37/todo, reset) | `offline_twice_is_bit_identical`, `engine_chunking_and_reset_are_bit_identical_to_offline`, `test_judge_twice_identical`, `test_input_order_of_limbs_irrelevant` | OK |
| Golden de B con el motor Rust | `golden_grades_via_engine` (243/243, Δsim < 1e-3, Δerr < 0.05 ms) | OK |
| Rendimiento 4 extremidades × 200 Hz | `perf_four_limbs_200hz` | OK: **release 0.0019 ms/fotograma de media, peor 0.42 ms**; debug 0.0077 ms (objetivo < 1 ms) |
| API C: versión, tamaños ABI, ciclo de vida, config por defecto, varios motores | `t1_capi::version_and_abi`, `lifecycle_and_default_config` | OK |
| API C: load_song con errores y mensaje (ruta, `steps[1].t`, ref_imu), err nulo/len 0/len 1 | `load_song_errors_have_message`, `replay_of_bad_session_reports_error` | OK |
| API C: mensaje de error truncado en UTF-8 válido | `truncated_error_is_valid_utf8` | **BUG 3** (ignorado) |
| API C: update sin canción, todos los punteros nulos (motor y fuentes) | `update_without_song_and_null_pointers` | OK |
| API C: mapping mono→canción (+1000 s y +12345.678 s), reset | `mapping_shifts_song_time` | OK |
| API C: push en fotogramas de 16 ms (max_out 1/3/64) contra los golden | `capi_push_matches_golden` | OK **243/243** |
| API C: **replay en tiempo real** (`jd_source_create_replay` a velocidad 1) + motor contra los golden (≥ 95 %) | `capi_replay_realtime_matches_golden` | OK **243/243** |
| Python: API (`judge_song`, `load_session`, `load_choreo`), CLI `--json` = API = golden, errores del CLI, golden coherentes | `test_t1_api.py` (41 casos) | OK |
| Python: regenerar con `proto/synth.py` en un temporal da archivos idénticos | `test_synth_regenerates_identical_files` | OK (8 sesiones, choreo, ref y 9 golden byte a byte) |
| Paridad Python↔Rust (jd_judge `--json` frente a `judge_song`) en las 8 sesiones de B y en perfect_noref | `test_t1_parity::test_parity_synthetic_sessions`, `test_parity_noref` | OK: grade exacto, Δsim ≤ 1e-3, Δerr ≤ 0.05 ms |
| Paridad en 9 escenarios **nuevos**: idéntico, ±60/−110 ms, débil, jitter, muestras justo en los bordes de trama, sin gyro, quieto, meseta en el pico | `test_parity_new_scenarios` | OK |

## FALLOS encontrados (no corregidos; tests marcados `#[ignore = "BUG: ..."]`)

1. **Agente C, `engine/jd_core/src/judge.rs` (`DefaultJudge::update`): en tiempo real los resultados salen fuera del orden (paso, extremidad).**
   La extensión de tiempo real espera hasta 0,5 s las tramas de una extremidad que ya envió datos, pero no
   retiene los resultados posteriores. Por eso una extremidad sin datos, o ya lista, se emite antes que otra
   anterior que aún espera.
   Reproducción: paso 0 `[left_hand, right_hand]`; solo hay datos de left_hand y llegan con 0,6 s de latencia.
   Se emite `(0, right_hand)` antes que `(0, left_hand)`.
   Contrato B5: "en orden de paso y luego de extremidad". Afecta a la UI (D/E) si da por hecho ese orden.
   Test: `t1_judge::realtime_emission_order_with_waiting_limb`.
   Arreglo sugerido: no emitir el ítem i mientras haya un ítem anterior pendiente que ya venció, o documentar en el contrato que el orden solo vale offline.
2. **Agente C, `engine/jd_core/src/signal.rs`: una sola muestra con NaN o ±inf en el acelerómetro estropea esa extremidad el resto de la canción.**
   El NaN entra en el LPF IIR y en el cuaternión y ya no sale de ahí. No hay pánico, pero todos los pasos
   siguientes de esa extremidad dan Miss: 0 de 3 Perfect en `nan` y en `inf`. Con NaN en el gyro sí se recupera.
   Un paquete UDP corrupto o un sensor defectuoso bastan para provocarlo, porque `phone/protocol.rs` acepta cualquier f32.
   Test: `t1_robustness::nan_and_inf_samples_do_not_break_the_engine`.
   Arreglo sugerido: descartar o sustituir componentes no finitos (como "sensor ausente") en `step()`, o filtrarlos en el servidor del móvil.
3. **Arquitecto, `engine/jd_capi/src/lib.rs` (`write_c_string`): el mensaje de error truncado puede no ser UTF-8.**
   Corta por bytes, así que `jd_engine_load_song` con un `err_len` pequeño puede partir un carácter multibyte
   (las rutas con ñ o tildes son normales en español). `engine_api.h` dice "Todas las cadenas son UTF-8", y
   Godot (`String::utf8`) mostraría basura.
   `sources.rs` (A) ya trunca bien con `truncate_utf8`.
   Test: `t1_capi::truncated_error_is_valid_utf8`.

En Python (B) no encontré fallos: no hay ningún `xfail`.

## Observaciones (no son fallos del contrato, pero conviene saberlas)

- **Recorder:** si `jd_source_recorder_set_mapping` llega después de la primera muestra, toda la grabación
  queda en `time_base:"mono"`, con `mono_to_song_s` como campo extra. Está documentado por A. El juego debe
  fijar el mapping **antes** del primer `poll` (mi test lo hace así); si no, la grabación no sirve directamente como referencia.
- **`flags == 0`** se interpreta como "trae accel y gyro" (C, signal.rs). Según `motion_sample.h`, 0 sería
  "no trae ninguno". Es inocuo con las fuentes actuales, pero conviene aclararlo en el contrato.
- **Sesiones:** `save_session` escribe −0.0 como 0 y los no finitos como 0. La ida y vuelta es exacta
  numéricamente, pero no bit a bit para −0.0. Es inocuo.
- **Validación de choreo en Python (B)** es menos estricta que la de Rust/F: no comprueba rango de bpm,
  extremidades repetidas ni `t + dur ≤ duration_s`. Un choreo puede pasar en `jdproto` y fallar en el motor.
  La paridad de validadores la da `tools/choreo_validate.py` (F).
- El LPF retrasa los picos unos +5 ms (visible en `timing_error_ms`). Ya lo mencionaron B y C. Con 40 ms el
  margen hasta pw es de ~5 ms, así que un accent a +45 ms reales ya es Good.

## Qué no pude verificar

- Datos de móvil reales y el servidor UDP bajo pérdidas reales (los cubren A y T3).
- Compilación y ejecución en Windows. El test de replay en tiempo real usa `sleep(16 ms)`, que en Windows
  puede durar unos 15 ms más: la cadencia cambia pero el criterio no.
- La GDExtension (G): solo probé la API C en Rust, llamando a los `extern "C"` con punteros crudos.
