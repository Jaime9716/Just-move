# Informe del agente C · Motor y juez (B3, B4, B5, motor de alto nivel)

> **Actualización v1.1:** implementado `contracts/judge_algorithm_v1.md` v1.1 como comportamiento por
> defecto (`JudgeParams::lag_search_scale = 2.0`; 1.0 = v1) y alineado con `proto/ALGORITHM.md` §1 (normativo):
> B3 — la 1ª muestra también integra/corrige, sensor ausente = último valor bruto, ángulos con atan2;
> B4 — ventana ±WIN/2 + 1e-9, mediana en (t−1, t], pico en la trama floor(t/hop + 0.5), `peak_t_song = 0` sin pico;
> B5 — ventana de move por índices k con 1e-6, búsqueda ±2·gw, solo pares existentes (sin pares → 0), puerta con
> medias de los pares de L*, tolerancia 1e-9 en ventanas, emisión en t + dur + 2·gw + 0.05.
> **Golden v1.1 de B: 243/243 (100 %), Δsimilitud < 1e-5, Δtiming 0 ms; late_150ms = 27 Miss** (el test de
> 150 ms en c_pipeline ahora exige Miss). Rendimiento release: 0.0025 ms/fotograma. Las secciones de abajo
> describen la versión v1; donde contradigan esta nota, manda la nota. La propuesta 2 ya está adoptada (v1.1).

## Qué hice

| Archivo | Contenido |
|---|---|
| `engine/jd_core/src/signal.rs` | B3: `make_signal_processor()` / `make_signal_processor_with(SignalParams)`. Estado por extremidad, LPF IIR de 1er orden (accel+gyro), orientación complementaria (mundo Y arriba, yaw 0), `lin_accel`. Utilidades de cuaterniones públicas (`q_mul`, `q_rotate`…). |
| `engine/jd_core/src/features.rs` | B4: `make_feature_extractor()` / `_with(FeatureParams)`. Tramas 100 ms / 20 ms en k·0.02, energy/peak/dir/quat, picos (PEAK_MIN, PEAK_REL con mediana de 1 s, separación 150 ms), `flush`. En flujo y con resultado idéntico al cálculo offline. |
| `engine/jd_core/src/judge.rs` | B5: `make_judge()` / `_with(JudgeParams)`. Accent, move con referencia (búsqueda de desfase, ncc, coseno de dirección, puerta de energía), move sin referencia, pies ×1.5, emisión por (paso, extremidad). |
| `engine/jd_core/src/engine.rs` | `Engine` con la API pública original + extras: `load_choreo`, `flush`, `choreo()`, `reference_frames()`. Funciones libres `compute_features`, `judge_offline` (= `jdproto.pipeline.judge_song`) y `reference_frames(&Session)`. |
| `engine/jd_capi/src/engine.rs` | Las 9 `jd_engine_*` de `contracts/engine_api.h`. Punteros nulos seguros, `catch_unwind` en la frontera, errores con `write_c_string`. `jd_engine_update` guarda los resultados que no caben en `out` para la llamada siguiente (no se pierde ninguno). |
| `engine/jd_tools/src/bin/jd_judge.rs` | `jd_judge <song_dir> <session.jsonl> [--json]`: tabla por paso + resumen (perfect/good/miss, similitud media, puntuación = (P + 0.5·G)/N). |
| `engine/jd_core/tests/c_pipeline.rs`, `c_perf.rs`, `c_golden.rs` | Pruebas de integración. |

## Cómo probarlo

```
cd engine && CARGO_TARGET_DIR=../build/C cargo test -p jd_core -p jd_capi -p jd_tools
CARGO_TARGET_DIR=../build/C cargo test --release -p jd_core --test c_perf -- --nocapture
../build/C/debug/jd_judge ../data/sessions/synthetic/song_demo ../data/sessions/synthetic/late_40ms.jsonl
```

## Resultados

- Unitarias (signal 6, features 9, judge 6, engine 2, capi 3) e integración (c_pipeline 6, c_golden 2, c_perf 1): todas pasan.
  Cubren: reposo estable; giro de 90° (yaw y roll con gravedad coherente) dentro de ±5°; pico a ±1 muestra
  (sobre muestras filtradas); accent 0/±40 ms → Perfect, ±80 ms → Good, 150 ms → Miss; pies con ventana ×1.5
  (65 ms: mano Good / pie Perfect; 130 ms: mano Miss / pie Good); move idéntico → Perfect; quieto y sin datos → Miss;
  sin referencia nunca Perfect; determinismo bit a bit (y tiempo real en trozos de 16 ms == offline, bit a bit).
- **Rendimiento** (4 extremidades × 200 Hz × 60 s, fotogramas de 16 ms, release): **0.0027 ms de media**, peor 0.04 ms
  (debug: 0.024 ms). Objetivo < 1 ms.
- **Golden de B** (`data/sessions/synthetic/golden/`, 9 casos, 243 resultados): **243/243 grades iguales (100 %)**,
  máx |Δsimilitud| = 2·10⁻⁵ (redondeo de B a 5 decimales), máx |Δtiming| = 0.000 ms. El motor en tiempo real
  (`load_song` + desfase mono→canción + fotogramas de 16 ms) también da 27/27 en `late_40ms`.
- Comprobado con un programa en C compilado contra `contracts/engine_api.h` y `libjd_engine.so` (tamaños y API).

## Interpretaciones locales del contrato v1 (coinciden con B según los golden)

- B3: la primera muestra de cada extremidad solo inicializa (no integra gyro); la puerta de gravedad usa |accel filtrado|;
  si `flags` indica que falta un sensor se mantiene su valor filtrado previo; `flags == 0` = trae ambos.
- B4: máximo local = m[i] > m[i−1] y m[i] ≥ m[i+1] (primera/última muestra nunca); mediana de energía de tramas con
  centro en [t_i − 1, t_i] (sin tramas → solo PEAK_MIN); separación secuencial (un candidato a < 150 ms del último
  aceptado lo sustituye si es mayor); trama más cercana con empate → la anterior; en tramas sin pico
  `peak_t_song` = instante del máximo de la trama.
- B5: move con referencia si hay tramas de referencia de la extremidad con centro en [t, t+dur]; emparejado por índice de
  trama; trama del jugador ausente = energía 0 y dirección nula; empate de score → menor |L| y luego L negativo;
  varianza poblacional; en cada `update` se emiten todos los resultados vencidos, ordenados por (paso, extremidad).

## Extensión de tiempo real (local, no está en el contrato)

Las tramas salen del extractor con retraso (~0.06 s, hasta ~0.25 s tras un pico, porque un pico solo es definitivo cuando
pasan 150 ms sin otro mayor). Un accent a +100 ms se emitiría a t+0.15 s antes de que su pico exista. Por eso el juez,
si **esa extremidad ya ha enviado tramas**, espera además a tener tramas hasta la ventana del paso, como mucho
`JudgeParams::max_wait_s = 0.5` s más. Sin datos de esa extremidad, o con todas las tramas empujadas (offline), el
instante de emisión es exactamente el del contrato. Con `max_wait_s = 0` se obtiene el contrato puro.

## Observaciones / propuestas de contrato (v2)

1. **Emisión en tiempo real**: añadir al contrato la espera a las tramas descrita arriba (o un `watermark` explícito en
   `FeatureExtractor`), o el juego verá Miss en accents tardíos.
2. **Move desplazado más que gw**: con 150 ms de retraso en manos, L* satura en +100 ms y el score sigue alto
   (≈ 0.9) → Good. docs/06 espera Miss. Propuesta: buscar L en ±(gw + 2·hop) y dar Miss si |L*| > gw.
3. **Retraso del LPF**: el filtro de 20 Hz retrasa los picos ~5–7 ms (timing_error sistemático +5 ms). Propuesta:
   restar el retraso de grupo conocido (≈ 1/(2π·fc)) a `peak_t_song`, o filtrar en fase cero solo para la referencia.
4. La tarea pedía "accent 40 ms → Good"; con pw = 50 ms el contrato da Perfect (docs/06 dice "Good/Perfect"). Se prueba
   el contrato: 40 ms Perfect, 80 ms Good.
5. `Judge` no tiene forma de cambiar la configuración sin recargar: `Engine::set_config` reinicia la partida.

## Qué queda / no verificado

- No probado con datos de móvil reales (solo sintéticos de B y propios).
- La GDExtension (G) debe envolver estas funciones; no se ha compilado aquí.
- `sources.rs` (A) genera un aviso `non_camel_case_types` (`JD_Source`), no es de C.
