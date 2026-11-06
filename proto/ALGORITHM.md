# Juez v1.1 en el prototipo Python: precisiones, historia de v1 → v1.1

`proto/jdproto/` implementa **contracts/judge_algorithm_v1.md v1.1** (config por defecto,
`lag_search_scale = 2.0`). Los golden de `data/sessions/synthetic/golden/` están generados con **v1.1**.
El contrato anterior sigue accesible con `config={"variant": "v1"}` (o `lag_search_scale = 1.0`;
`--variant v1` en el CLI). Por decisión del arquitecto, la **§1 de este documento es normativa**: el
motor Rust (agente C) debe seguirla para reproducir los golden. La §2–§3 cuentan el problema de v1 que
motivó v1.1.

## 1. Precisiones de implementación (normativas desde v1.1)

### B3 · Señal
- `t_song = round(t·1e9)·1e-9 + mono_to_song_s` (en f64), igual que `t_ns` del motor.
- Muestras de cada extremidad ordenadas por `t` (orden estable). Cada extremidad, estado propio.
- `dt`: primera muestra 0.005; después `t − t_anterior`, limitado a [0.0005, 0.05].
- Filtro paso bajo: **se inicializa con la primera muestra** (`y = x`). Después `y += k·(x − y)`.
- Muestra sin `a` o sin `g`: se usa el último valor **bruto** recibido de ese campo (si nunca llegó:
  `a = (0, G, 0)`, `g = (0, 0, 0)`).
- Primera muestra: `q` = rotación mínima que lleva `a_filtrado` a +Y (eje `â × Y`, ángulo
  `atan2(|â × Y|, â·Y)`; si `|â × Y| < 1e-12`: identidad si `â·Y > 0`, si no 180° sobre X); **después
  se aplican los mismos pasos que a cualquier otra muestra** (integrar con dt = 0.005, corregir, lin_accel).
- Integración: `q ← normalize(q ⊗ exp(½·ω·dt))` con `ω = gyro filtrado` (marco del sensor), es decir
  `dq = (cos(θ/2), ω̂·sin(θ/2))`, `θ = |ω|·dt`; si `θ ≤ 1e-12` no se multiplica (sí se normaliza).
- Puerta de gravedad sobre `|a_filtrado|`. Corrección: `up = R(q)·â`, eje `up × Y` (= `(−up_z, 0, up_x)`),
  ángulo `COMP_ALPHA · atan2(|up × Y|, up_y)`; se aplica en el marco del mundo: `q ← normalize(r ⊗ q)`.
  Si `|up × Y| ≤ 1e-12` no hay corrección.
- `lin_accel = R(q_final)·a_filtrado − (0, G, 0)` con la `q` ya corregida.

### B4 · Rasgos
- La trama `k` (entero, puede ser negativo) tiene centro `t_c = k·FRAME_HOP_S` (calculado como
  `(k as f64) * hop`) y contiene las muestras con `|t − t_c| ≤ FRAME_WIN_S/2 + 1e-9`.
  Tramas sin muestras no existen.
- `energy = sqrt(mean(|lin|²))`, `peak = max |lin|`, `dir = mean(lin)/|mean(lin)|` o `(0,0,0)` si
  `|mean(lin)| < DIR_MIN_NORM`, `quat` = la de la última muestra de la trama.
- Picos sobre la serie de muestras `m_i = |lin_i|`: candidato si `m_i > m_{i−1}` **y** `m_i ≥ m_{i+1}`
  (la primera y la última muestra nunca lo son), `m_i ≥ PEAK_MIN` y
  `m_i ≥ PEAK_REL × mediana(energy de las tramas con t_c ∈ (t_i − 1 s, t_i])` (sin tramas en ese
  intervalo, solo cuenta PEAK_MIN; mediana estándar: media de los dos centrales si son pares).
- Separación: candidatos en orden de tiempo; si `t_i − t_último_aceptado < PEAK_MIN_SEP_S`, el
  candidato **sustituye** al último aceptado solo si es **estrictamente mayor**; si no, se descarta.
  (En streaming, un pico es definitivo cuando han pasado PEAK_MIN_SEP_S sin otro mayor.)
- `is_peak = true` en la trama `k = floor(t_pico/hop + 0.5)`; su `peak_t_song = t_pico`.
  En el resto `peak_t_song = 0.0`.

### B5 · Juez
- Orden de resultados: paso, y dentro de cada paso extremidades únicas en orden
  left_hand, right_hand, left_foot, right_foot.
- **Accent:** se usan los `peak_t_song` de las tramas con `is_peak` de esa extremidad. Pico válido si
  `|peak_t − t| ≤ gw + 1e-9`; el más cercano (empate → el anterior). `|err| ≤ pw + 1e-9` → Perfect.
  Miss → `similarity = 0`, `timing_error_ms = 0`.
- **Ventana de un move:** tramas con `k ∈ [ceil(t/hop − 1e-6), floor((t+dur)/hop + 1e-6)]`.
  "Hay referencia" ⇔ existe al menos una trama de referencia de esa extremidad en esa ventana.
- Desfases `L = m·hop`, `|m| ≤ floor(LAG_SEARCH_SCALE·gw/hop + 1e-6)` (v1.1: `LAG_SEARCH_SCALE = 2`,
  manos m ∈ [−10, 10], pies m ∈ [−15, 15]; v1: [−5, 5] y [−7, 7]). La nota sigue usando pw/gw. Orden de evaluación `m = 0, −1, +1, −2, +2, …`; un desfase solo
  sustituye al mejor si su score es **estrictamente mayor** (empate → menor |L|, y a igual |L| el negativo).
- Emparejado: pares (ref `k`, jugador `k+m`) solo donde existen ambas tramas. Sin pares → score 0.
- `ncc`: medias y varianzas poblacionales; si `var_r < 1e-6` o `var_p < 1e-6` → 0; si no
  `Σ dr·dp / sqrt(Σ dr² · Σ dp²)` recortado a [0, 1].
- `dcos`: media de cosenos en pares con ambos `dir` distintos de `(0,0,0)`, recortada a [0, 1].
- Puerta de energía con las medias de `E_r` y `E_p` **de los pares de L\***:
  si `0.25·mean(E_r) > 0` y `mean(E_p) < 0.25·mean(E_r)` → `score *= mean(E_p)/(0.25·mean(E_r))`.
- Move sin referencia: media de `energy` de las tramas del jugador en la ventana (0 si no hay).
- Todas las comparaciones de ventanas de tiempo usan tolerancia 1e-9 s (los umbrales de similitud no).
- Emisión (v1.1): `t + dur + LAG_SEARCH_SCALE·gw_escalado + EMIT_MARGIN_S` (`judge.emit_time()`).

### Precisión numérica (para comparar con Rust)
Python calcula todo en f64. Si Rust guarda `FilteredSample`/`FeatureFrame` en f32 los resultados
difieren en ~1e-5: compara `similarity` y `timing_error_ms` con tolerancia (p. ej. 1e-3 y 0.01 ms) y
el `grade` exacto. En los golden v1.1 el margen mínimo de una similitud a un umbral que cambie la nota
es 0.003 (perfect_noref) y la diferencia mínima entre el mejor y el segundo mejor desfase, cuando no son
empates exactos a 0, es 3e-4 (wrong_moves, paso 6): suficiente para f32, pero es el caso a vigilar.

## 2. Comportamientos de docs/06 con v1 (contrato anterior; datos sintéticos)

| Sesión | Esperado (docs/06) | v1 P/G/M | ¿Cumple? |
|---|---|---|---|
| perfect | mayoría Perfect | 27/0/0 | sí |
| late_40ms | Good/Perfect | 20/7/0 | sí |
| late_150ms | Miss | **0/10/17** | **no**: los 10 moves dan Good |
| still | Miss | 0/0/27 | sí |
| wrong_moves | Miss mayoritario | 0/0/27 | sí |
| early_80ms | (Good) | 3/24/0 | — |
| noisy | (mayoría aciertos) | 27/0/0 | — |
| foot_only | pie acierta, manos Miss | 4/0/23 | sí |

### Problema P1 · un move 150 ms tarde puntúa Good
En v1 el rango de búsqueda del desfase `L` coincide con la ventana de nota (±gw). Si el jugador va más
tarde que gw, el mejor `L` queda pegado al borde (+100 ms), la similitud ahí sigue siendo alta (0.7–0.9,
la envolvente de energía es suave) y como `|L*| ≤ gw` la nota es Good. Es decir, **v1 no distingue
"100 ms tarde" de "300 ms tarde"** en los moves. Ejemplo (late_150ms, paso 5, mano derecha):

```
L (ms):   0    20   40   60   80   100 | 120  140  160  180  200
score:  0.17 0.26 0.41 0.56 0.70 0.82 | 0.92 0.99 0.99 0.93 0.83   (v1 solo ve hasta 100)
```

En los pies pasa igual (gw = 150 ms → L* = 140 ms → Good), aunque ahí 150 ms está dentro de la ventana.

### Otras observaciones (no bloquean)
- **Sesgo de los accent:** el paso bajo de 20 Hz retrasa el máximo ~1 muestra (5 ms); con la forma del
  golpe sintético, el error medio de los 16 accent de `perfect` es +7 ms. Inocuo con pw = 50 ms; se podría restar en la
  calibración del jugador (offset de entrada).
- **COMP_ALPHA = 0.02 por muestra** a 200 Hz es una constante de tiempo de ~0.25 s: en movimientos
  largos con |a| dentro de la puerta (0.8–1.2 G) la inclinación estimada se va hacia la aceleración
  total y parte de la gravedad pasa a `lin_accel`. No afecta a los resultados sintéticos (referencia y
  jugador sufren lo mismo), pero conviene medirlo con grabaciones reales (hipótesis no verificada).
- Los datos sintéticos de referencia y de jugador salen del mismo modelo: la similitud de un move bien
  hecho es ~1.0 y la de uno equivocado ~0.25. Con personas reales las similitudes serán menores; los
  umbrales `PERFECT_SIM/GOOD_SIM` hay que ajustarlos con sesiones reales (T3, docs/07).

## 3. v1.1 (antes "variante v1b", adoptada por el arquitecto)

**Cambio único:** buscar `L` en `±LAG_SEARCH_SCALE·gw` con `LAG_SEARCH_SCALE = 2` (manos ±200 ms,
pies ±300 ms), manteniendo la regla de nota (`|L*| ≤ pw` Perfect, `|L*| ≤ gw` Good, si no Miss), y
retrasar la emisión a `t + dur + LAG_SEARCH_SCALE·gw + EMIT_MARGIN_S`. Así el juez encuentra el
desfase real y, si es mayor que gw, da Miss. Coste: 21 desfases en lugar de 11 por move y extremidad.

| Sesión | v1 P/G/M | v1.1 P/G/M (golden) |
|---|---|---|
| perfect | 27/0/0 | 27/0/0 |
| late_40ms | 20/7/0 | 20/7/0 |
| late_150ms | 0/10/17 | **0/0/27** |
| early_80ms | 3/24/0 | 3/23/1 |
| wrong_moves | 0/0/27 | 0/0/27 |
| still | 0/0/27 | 0/0/27 |
| noisy | 27/0/0 | 27/0/0 |
| foot_only | 4/0/23 | 4/0/23 |
| perfect (sin ref) | 16/10/1 | 16/10/1 (el Miss es el side_step del pie: energía 2.5 < 3 m/s²) |

El único cambio fuera de late_150ms es early_80ms paso 6 (pump, mano derecha): el jugador de esa
ejecución ya iba 40 ms adelantado respecto a la referencia, así que el desfase real es −120 ms; v1 lo
recortaba a −100 ms (Good) y v1.1 lo mide bien (Miss). Es el comportamiento buscado.

Regenerar los golden: `python3 proto/synth.py` (añade `--variant v1` para ver también la tabla v1).
