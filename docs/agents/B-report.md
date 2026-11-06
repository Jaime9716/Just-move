# Informe del agente B · Prototipo Python del juez (H0)

> **Actualización v1.1:** el arquitecto adoptó la propuesta (contrato v1.1, `LAG_SEARCH_SCALE = 2.0`,
> emisión en `t + dur + LAG_SEARCH_SCALE·gw + EMIT_MARGIN_S`, ALGORITHM.md §1 normativa). jdproto usa
> v1.1 por defecto (v1 con `config={"variant":"v1"}` / `--variant v1`); los golden (incluido
> `perfect_noref`) se regeneraron con v1.1 y los 64 tests pasan.

## Qué hice

- **`proto/jdproto/`**: implementación fiel del juez v1.1 con la API exacta del contrato
  (`session.load_session`, `choreo.load_choreo`, `pipeline.judge_song`) y los módulos `signal.py` (B3),
  `features.py` (B4), `judge.py` (B5), `config.py` (constantes configurables + variantes), CLI
  `python -m jdproto.judge_cli <song_dir> <sesión.jsonl> [--json]` (con `PYTHONPATH=proto`).
- **`proto/jdproto/synth.py` + `proto/synth.py`**: modelo físico de un móvil Android sujeto en la mano
  (pantalla arriba, ~20° hacia el jugador) o atado a la espinilla: aceleración en el mundo + velocidad
  angular en el marco del sensor, orientación integrada a 1 kHz (giroscopio coherente), gravedad,
  sesgos, ruido 0.05 m/s² / 0.01 rad/s, saturación ±8 g / ±2000 °/s, 200 Hz con jitter, humanización
  (±8 ms en golpes, ±15 ms en moves, amplitud ±10 %). Golpes de ~25 m/s² con frenada y giro de muñeca,
  moves de 6–8 m/s² a 1–2 Hz, pisotones con impacto de ~38 m/s². Semillas fijas: regenerar da archivos
  idénticos (comprobado con md5).
- **`data/sessions/synthetic/`**: `song_demo/choreo.json` (30 s, 120 BPM, 20 pasos accent/move de manos y
  pie derecho → 27 resultados) + `song_demo/ref_imu.jsonl`; sesiones `perfect`, `late_40ms`,
  `late_150ms`, `early_80ms`, `wrong_moves`, `still`, `noisy`, `foot_only` (3 móviles: dos manos y el pie
  derecho; `foot_only` solo el pie); `golden/<sesión>.json` = salida de `judge_song` con **v1.1**
  (+ `golden/perfect_noref.json`: perfect juzgada sin referencia, para cubrir el camino "move sin ref").
- **`proto/ALGORITHM.md`**: todas las decisiones que el contrato deja abiertas (C debe copiarlas para
  reproducir los golden), el problema de v1 y el cambio a v1.1 con resultados.
- **`proto/dev_tests/`**: 64 tests pytest (B3 reposo/giro 90°/paso bajo, B4 picos ±1 muestra y
  separación, B5 ventanas, desfases, desempates, puerta de energía, sin referencia, orden, emisión;
  golden bit a bit, comportamientos de docs/06, CLI, validación, determinismo del sintetizador).
- **`proto/README.md`**: cómo ejecutar y cómo juzgar una sesión real grabada con `jd_record`.

## Resultados por sesión (27 resultados cada una; P/G/M = Perfect/Good/Miss)

| Sesión | Qué es | Esperado (docs/06) | v1.1 (golden) | v1 (anterior) | ¿v1.1 cumple? |
|---|---|---|---|---|---|
| perfect | otra ejecución humana de la coreografía | mayoría Perfect | 27/0/0 | 27/0/0 | sí |
| late_40ms | perfect +40 ms | Good/Perfect | 20/7/0 | 20/7/0 | sí |
| late_150ms | perfect +150 ms | Miss | 0/0/27 | 0/10/17 | sí (v1 no: moves → Good) |
| early_80ms | perfect −80 ms | — | 3/23/1 | 3/24/0 | — |
| wrong_moves | gestos equivocados | Miss mayoritario | 0/0/27 | 0/0/27 | sí |
| still | móviles quietos en la mano | Miss | 0/0/27 | 0/0/27 | sí |
| noisy | ruido ×8, 2 % pérdidas | — | 27/0/0 | 27/0/0 | — |
| foot_only | solo pie derecho | pie acierta, manos Miss | 4/0/23 | 4/0/23 | sí |
| perfect_noref | perfect sin referencia | moves ≤ Good | 16/10/1 | 16/10/1 | sí |

Similitudes típicas: move bien hecho ≈ 0.97–1.00, move equivocado ≈ 0.17–0.28, quieto ≈ 0.

**Problema de v1 (P1):** en los moves el desfase L se busca solo en ±gw, así que con 150 ms de retraso
el mejor L queda en el borde (+100 ms) con similitud 0.7–0.9 → Good. **v1.1** (adoptada): buscar en
±2·gw y mantener la regla de nota por |L*|; corrige late_150ms. El único otro cambio es early_80ms paso 6
(desfase real −120 ms → Miss). v1 sigue disponible con `config={"variant":"v1"}` / `--variant v1`.

## Cómo probarlo

```bash
python3 proto/synth.py --variant v1       # regenera datos y golden v1.1, imprime la tabla (y la de v1)
python3 -m pytest -q proto/dev_tests
PYTHONPATH=proto python3 -m jdproto.judge_cli data/sessions/synthetic/song_demo data/sessions/synthetic/late_150ms.jsonl
```

## Para el agente C (Rust) y T1

- Lee `proto/ALGORITHM.md` §1: inicialización del filtro, primera muestra, tramas indexadas por k,
  criterio de máximo local, mediana de PEAK_REL en (t−1 s, t], sustitución de picos, orden de desfases
  0,−1,+1,… con "estrictamente mayor", emparejado solo de tramas existentes, puerta de energía con los
  pares de L*, tolerancias 1e-9 / 1e-6.
- Los golden se calculan en f64 leyendo los `.jsonl` del disco. Compara `grade` exacto y `similarity` /
  `timing_error_ms` con tolerancia (sugerido 1e-3 y 0.01 ms). Margen mínimo a un umbral que cambie
  nota: 0.003 (perfect_noref); diferencia mínima entre mejores desfases no empatados: 3e-4 (wrong_moves paso 6).
- v1.1: desfases m ∈ [−10, 10] (manos) / [−15, 15] (pies); emisión con 2·gw.
- Las muestras sintéticas nunca caen en el borde de una trama (fase por dispositivo + jitter ±0.4 ms).

## Qué queda / qué no pude verificar

- **Sin datos reales**: no hay móvil ni `jd_record` ejecutable aquí; el modelo físico es plausible pero
  referencia y jugador salen del mismo modelo, así que las similitudes son optimistas. Los umbrales
  PERFECT_SIM/GOOD_SIM y PEAK_MIN deben ajustarse con grabaciones reales (T3).
- No medí cuánto se desvía la inclinación estimada durante moves largos por COMP_ALPHA = 0.02/muestra
  con la puerta 0.8–1.2 G (hipótesis en ALGORITHM.md).
- No comprobé la regla de emisión en streaming (el prototipo es offline); `judge.emit_time()` la expone.
- El paquete demo no tiene `audio.ogg` (el juez no lo usa); `load_choreo` no comprueba archivos.

## Propuestas de contrato

1. ~~`LAG_SEARCH_SCALE = 2`~~ y ~~precisiones de ALGORITHM.md §1 normativas~~: **adoptadas en v1.1**.
2. `session_format.md`: el ejemplo de `devices` aún usa `"kind":"joycon"`; con móviles sería
   `"kind":"phone"` (los sintéticos usan `"phone"`).
