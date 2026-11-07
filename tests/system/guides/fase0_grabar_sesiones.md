# Fase 0 · Grabar 3–5 sesiones reales con `jd_record`

**Objetivo:** datos reales del acelerómetro y giroscopio de TU móvil con los que trabajan todos los
agentes (juez, filtros, tests). Tiempo: ~30 min.

## Qué grabar

| # | Archivo (en `data\sessions\real\`) | Móvil en | Cómo | Duración |
|---|---|---|---|---|
| 1 | `AAAA-MM-DD_mano-der_reposo.jsonl` | mano derecha, sobre la mesa | quieto, pantalla arriba | 60 s |
| 2 | `AAAA-MM-DD_mano-der_bien_demo.jsonl` | mano derecha (pantalla hacia ti) | bailando `demo_metronomo` lo mejor que puedas | 35 s |
| 3 | `AAAA-MM-DD_mano-der_mal_demo.jsonl` | mano derecha | a destiempo (medio beat tarde) o movimientos al azar | 35 s |
| 4 | `AAAA-MM-DD_pie-der_bien_demo.jsonl` | tobillo derecho (calcetín, cinta o brazalete; pantalla hacia fuera) | pisotones/patadas al ritmo | 35 s |
| 5 (opcional) | `AAAA-MM-DD_mano-der_bien_<cancion>.jsonl` | mano derecha | una canción de YouTube importada | la canción |

## Pasos

1. Abre la **música** en otro programa, sin el juego (el juego y `jd_record` usan el mismo puerto: **cierra
   el juego**). Para la demo: `data\songs\demo_metronomo\audio.ogg` (VLC o el reproductor de Windows).
2. En PowerShell, desde la raíz del repo:
   ```powershell
   & "<TOOLS>\jd_record.exe" data\sessions\real\2026-11-20_mano-der_bien_demo.jsonl --seconds 40 --song demo_metronomo --note "bien bailado, mano derecha, Pixel 7, 5 GHz"
   ```
   Para el pie, pon **"pie derecho"** en `--note` y en el nombre del archivo (jd_record aún no permite
   elegir la extremidad: la cabecera dirá `right_hand`; T3 la corrige al analizar).
3. Muestra **IP:puerto** y un **código**: escríbelos en la app del móvil → "Conectar". En la terminal
   verás una línea por segundo con `200 Hz`, `rtt`, `perdidos` y `batería`. Si no conecta: `BUILD.md` §5.
4. **Marca de sincronía:** cuando la terminal ya muestre datos, pulsa Play en la música y, **en el primer
   clic**, da **3 sacudidas secas** al ritmo (o 3 pisotones). Así se puede alinear la sesión con la canción.
5. Baila. Termina con Enter o espera `--seconds`. (Con Ctrl+C se pierde el último medio segundo.)
6. Comprueba la sesión al momento:
   ```powershell
   python tests\system\tools\session_stats.py data\sessions\real\2026-11-20_mano-der_bien_demo.jsonl
   ```
   Debe dar ~200 Hz y pocos o ningún hueco. Si suspende, repítela más cerca del router y apunta ambas.

## Qué contarme (docs/feedback/AAAA-MM-DD-Fase0.md)

- Modelo de móvil, Android, Wi-Fi (banda, distancia), RTT que mostró `jd_record`.
- Cómo sujetaste/ataste el móvil y si fue cómodo o se movía (sobre todo en el tobillo).
- La salida de `session_stats.py` de cada sesión.
- **No** subas canciones de YouTube; las sesiones `.jsonl` sí (son solo números).
