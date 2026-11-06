# Contrato · Algoritmo del juez v1 (B3 → B4 → B5)

El agente **B** lo implementa en Python (`proto/jdproto/`) y lo ajusta con datos; el agente **C** lo
implementa en C++ con **los mismos parámetros**. Si B propone cambios, los documenta en
`proto/ALGORITHM.md` y el arquitecto decide si pasan a v2. Todas las constantes son configurables.

## Constantes por defecto

| Nombre | Valor |
|---|---|
| G | 9.80665 m/s² |
| LPF_CUTOFF_HZ | 20 |
| COMP_ALPHA | 0.02 (corrección por muestra) |
| GRAVITY_GATE | aceptar corrección si 0.8·G ≤ \|a\| ≤ 1.2·G |
| FRAME_WIN_S | 0.100 |
| FRAME_HOP_S | 0.020 |
| PEAK_MIN | 8.0 m/s² |
| PEAK_REL | 1.5 × mediana de `energy` del último 1 s |
| PEAK_MIN_SEP_S | 0.150 |
| DIR_MIN_NORM | 0.5 m/s² |
| perfect_window_ms / good_window_ms / foot_window_scale | 50 / 100 / 1.5 |
| W_NCC / W_DIR | 0.6 / 0.4 |
| ENERGY_GATE | 0.25 |
| NOREF_ENERGY_FULL | 6.0 m/s² |
| PERFECT_SIM / GOOD_SIM | 0.75 / 0.5 |
| EMIT_MARGIN_S | 0.05 |

## B3 · Señal (por extremidad; cada `limb` tiene su propio estado)

1. `t_song = t_ns·1e-9 + mono_to_song_s`. `dt` = diferencia con la muestra anterior de esa extremidad
   (primera muestra: dt = 0.005; dt se limita a [0.0005, 0.05]).
2. Filtro paso bajo IIR de 1er orden en accel y gyro: `y += k·(x − y)`, `k = dt / (dt + 1/(2π·LPF_CUTOFF_HZ))`.
3. Orientación `q` (mundo←sensor), mundo con **Y arriba**:
   - Inicial: la rotación mínima que lleva el accel filtrado de la primera muestra a +Y (yaw = 0).
   - Cada muestra: integrar gyro filtrado `q ← q ⊗ exp(½·ω·dt)`; normalizar.
   - Si pasa GRAVITY_GATE: `up_est = R(q)·â`; corregir `q` con una rotación de ángulo
     `COMP_ALPHA·angle(up_est, +Y)` sobre el eje `up_est × +Y`.
4. `lin_accel = R(q)·a_filtrado − (0, G, 0)`.

## B4 · Rasgos (por extremidad)

- Tramas centradas en múltiplos de FRAME_HOP_S (t = k·0.02); cada trama usa las muestras con
  `|t_muestra − t_centro| ≤ FRAME_WIN_S/2`. Una trama se emite cuando ya llegó una muestra con
  `t > t_centro + FRAME_WIN_S/2` (o en flush). Tramas sin muestras no se emiten.
- `energy` = RMS de `|lin_accel|`; `peak` = máx `|lin_accel|`; `dir` = media de `lin_accel`
  normalizada (vector cero si su norma < DIR_MIN_NORM); `quat` = la de la última muestra de la trama.
- Picos (sobre la serie de muestras `|lin_accel|`): máximo local con valor ≥ PEAK_MIN y ≥ PEAK_REL,
  y a ≥ PEAK_MIN_SEP_S del pico anterior aceptado (si hay dos más cerca, gana el mayor).
  `peak_t_song` = instante de la muestra del máximo; `is_peak = true` en la trama cuyo centro es el
  más cercano a ese instante.

## B5 · Juez (un resultado por paso y extremidad del paso)

`pw = perfect_window_ms/1000`, `gw = good_window_ms/1000`; para pies ambos × foot_window_scale.

**Accent:** picos del jugador en esa extremidad dentro de `[t − gw, t + gw]`; se toma el más cercano a `t`.
`err = peak_t − t`. `|err| ≤ pw` → Perfect; `≤ gw` → Good; sin pico → Miss.
`similarity = max(0, 1 − |err|/gw)`, `timing_error_ms = err·1000`.

**Move con referencia** (hay tramas de referencia de esa extremidad):
- Series en `[t, t+dur]`: `E_r`, `D_r` (referencia) y, para cada desfase `L ∈ {−gw … +gw}` en pasos
  de FRAME_HOP_S, `E_p(L)`, `D_p(L)` del jugador en `[t+L, t+dur+L]` (emparejando trama a trama).
- `ncc` = correlación normalizada de media cero de E_r y E_p (0 si alguna varianza < 1e-6), recortada a [0,1].
- `dcos` = media del coseno entre D_r y D_p en tramas donde ambas son no nulas, recortada a [0,1];
  si no hay ninguna, `score = ncc`; si no, `score = W_NCC·ncc + W_DIR·dcos`.
- Se elige `L*` con mayor score (empate → menor |L|).
- Puerta de energía: si `mean(E_p) < ENERGY_GATE·mean(E_r)`, `score *= mean(E_p)/(ENERGY_GATE·mean(E_r))`.
- `score ≥ PERFECT_SIM` y `|L*| ≤ pw` → Perfect; `score ≥ GOOD_SIM` y `|L*| ≤ gw` → Good; si no, Miss.
  `similarity = score`, `timing_error_ms = L*·1000`.

**Move sin referencia:** `similarity = clamp(mean(E_p en [t,t+dur]) / NOREF_ENERGY_FULL, 0, 1)`;
Good si ≥ GOOD_SIM, si no Miss (nunca Perfect). `timing_error_ms = 0`.

**Emisión:** el resultado de un paso se emite en el primer `update(song_time)` con
`song_time ≥ t + dur + gw_escalado + EMIT_MARGIN_S`, en orden de paso y luego de extremidad
(left_hand, right_hand, left_foot, right_foot).

## API del prototipo Python (agente B; la usa T1 en `proto/tests/`)

```python
from jdproto.session import load_session          # -> dict(header=..., samples=[dict(t,src,limb,a,g)])
from jdproto.choreo import load_choreo            # -> dict como choreo.json
from jdproto.pipeline import judge_song           # (choreo: dict, player_samples: list, ref_samples: list|None,
                                                  #  config: dict|None) -> list[dict(step, limb, grade, similarity, timing_error_ms)]
# grade: "miss" | "good" | "perfect"; limb: "left_hand"...; las muestras usan tiempo de canción.
```
CLI: `python -m jdproto.judge_cli <song_dir> <player_session.jsonl> [--json]`.
