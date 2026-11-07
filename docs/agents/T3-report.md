# Informe del agente T3 · Pruebas de alto nivel en el mundo real y feedback

## Qué hice

| Archivo | Contenido |
|---|---|
| `tests/system/PLAN.md` | Plan P1–P11 reescrito para el **móvil Android por Wi-Fi**, con medidas, criterios numéricos, herramienta e hito. Sustituye a la tabla de `docs/07` (Joy-Con) |
| `tests/system/guides/` | `00_preparacion.md` (herramientas, red, rutas `user://` en Windows), `fase0_grabar_sesiones.md` (3–5 sesiones con `jd_record`, mano y tobillo, bien y mal, marca de 3 sacudidas, en `data/sessions/real/`), y `P1_…` a `P11_…`, una guía paso a paso por prueba |
| `tests/system/tools/session_stats.py` | P2: por `src` da tasa, Δt (mediana, p99, máx), huecos > 50 ms con su instante, pérdidas estimadas, t monotónico, rangos de acel./giro, valores imposibles, \|a\| en el tramo más quieto (≈ 9,81) y APROBADO/SUSPENDIDO. Acepta varios archivos y comodines |
| `tests/system/tools/calib_stats.py` | P4: valores en ms (argumentos), `.txt`/`.csv` (`;` o `,`, columna `offset_ms`…) o copias de `profile.cfg` (`--key input_offset/visual_offset`). Da media, desviación, rango, tendencia y veredicto (rango < 10 ms) |
| `tests/system/tools/av_sync_from_video.py` | P3: ffmpeg extrae el brillo de cada fotograma (con su pts real, vía `showinfo`, `--roi`) y el audio (t0 por `ashowinfo`). Detecta destellos con umbral local al 50 % (tolera colores distintos y fotogramas omitidos) y clics (envolvente con paso alto). Empareja, descarta atípicos (MAD) y da el desfase al principio y al final (25 % de las parejas en cada extremo), la deriva en ms/min y la dispersión. Opciones `--slowdown`, `--camera-offset-ms` y `--tol-ms` |
| `tests/system/tools/fps_log_stats.py` | P9: lee `t_s,fps` de `user://fps_log.csv`. Da el fps medio (fotogramas/tiempo), la mediana, el 1 % peor, el p99 y el máximo del tiempo de fotograma, los tirones > 50 ms con su segundo, el peor segundo y el veredicto. Avisa si el monitor va a > 60 Hz (`--target`) |
| `tests/system/tools/feedback_summary.py` | Resume `docs/feedback/*.md` (sin contar README ni plantilla): entorno, canción, pruebas, sensaciones y fallos de cada archivo. Da el último estado de P1–P11, las pruebas pendientes, la diversión media, los móviles y Wi-Fi usados y los fallos |
| `tests/system/tests/` | 24 tests pytest. Usan sesiones JSONL sintéticas y vídeos sintéticos de ffmpeg (lavfi `geq` + `aevalsrc`) con desfases de **+30 ms y −20 ms** (h264+aac), +5 ms (aprueba), una deriva de 1 ms/s, cámara lenta ×8 a 30 fps, `--camera-offset-ms` + `--roi`. Hay además una prueba **extremo a extremo con `jd_record` + `phone_sim` reales** (se salta si no están compilados en `build/T3/debug` o en `$JD_TOOLS_DIR`) |
| `tests/system/results_template.csv` | `id,fecha,entorno,medida,criterio,cumple,notas`, con una fila guía por prueba (F0, P1…P11, P3a/P3b) |
| `tests/system/results/.gitignore` | Evita subir los vídeos de P3 |
| `docs/feedback/README.md`, `plantilla.md` | Adaptados al móvil (modelo, Android, versión de la app, sujeción, Wi-Fi 2,4/5 GHz, distancia, RTT, batería) y a las canciones de YouTube (importación, calidad del audio y del vídeo, sincronía del bailarín al principio, a mitad y al final, pasos automáticos) |

## Cómo probarlo

```
cd engine && CARGO_TARGET_DIR=../build/T3 cargo build -p jd_tools     # opcional, para el test extremo a extremo
cd .. && python3 -m pytest tests/system/tests -q                     # 24 passed (~20 s)
python3 tests/system/tools/session_stats.py data/sessions/synthetic/*.jsonl
```
Comprobado también junto a `proto/tests`: 86 passed, sin choques de `conftest`. Los helpers están en `t3_helpers.py`.

## Hallazgos

1. **P3: hay dos criterios.** El juego corrige el vídeo del bailarín solo cuando se desvía más de 70 ms
   (`VIDEO_THRESHOLD_S`, agente E). Exigirle 10 ms no tiene sentido, así que P3 se divide en
   (a) la interfaz/carril ("Calibrar pantalla"): ≤ 10 ms; y (b) el vídeo del bailarín: ≤ 40 ms y nunca más de 70.
2. **El `video.ogv` de `demo_metronomo`, leído con ffmpeg 6.1:** decodifica 865 de 900 fotogramas, y en
   algunos beats el fotograma del destello sale fechado un fotograma (33 ms) antes. Los paquetes del
   Ogg tienen ese pts. Si Godot avanza por número de fotograma, podría verse un error parecido. Para **F/E**:
   revisar los argumentos de Theora en `jd_media.video_codec_args` (¿se omiten fotogramas duplicados?)
   y comprobarlo en P3b.
3. `jd_record` **no permite elegir la extremidad**: las sesiones de pie quedan con `right_hand` en la
   cabecera. En la guía se pide anotar "pie" en `--note` y en el nombre del archivo.
4. `jd_record` termina al instante si stdin está cerrado (EOF = "Enter"). Para lanzarlo desde scripts hay
   que dejarle un stdin abierto (lo hace el test).
5. La calibración no se guarda en ningún log: el usuario apunta a mano "+NN ms (± SS)". `fps_log.csv` se
   sobrescribe en cada partida, y las guías piden copiarlo.

## Qué no pude verificar

- Nada con un móvil real, Wi-Fi, Windows ni Godot (entorno en la nube). Los umbrales de P5–P8 (70 %,
  40 %, 95 % Miss, 5 % de pérdidas) son una primera propuesta y se ajustarán con el feedback de Fase 0/H0.
- `av_sync_from_video.py` solo está probado con vídeo sintético y con la demo remezclada. Con vídeos
  reales de móvil pueden aparecer cosas que no están cubiertas: VFR, edit lists, autoexposición, rolling
  shutter o cámara lenta sin audio. Si falla, el script da un error claro y sugiere `--roi`, 60 fps o `--slowdown`.
  El sesgo A/V de la propia cámara no se mide (`--camera-offset-ms` si se conoce).
- En el CSV de calibración, `,` se acepta como decimal solo si el separador es `;` o si hay un valor por línea.

## Propuestas (para otros agentes; no son contratos)

- **A**: `jd_record --limb right_foot` (o la extremidad que asigne el servidor), para que las sesiones de pie
  tengan bien la cabecera. Que `jd_record` siga grabando si stdin llega a EOF (esperar a `--seconds`).
- **D**: añadir cada calibración a `user://calib_log.csv` (`fecha,tipo,offset_ms,stddev_ms,n,fuente`).
  Así P4 se analizaría con `calib_stats.py` sin apuntar nada a mano (el script ya lee la columna `offset_ms`).
- **E**: opción para que `fps_log.csv` no se sobrescriba (añadir la fecha al nombre), y grabar la sesión
  de la partida (`RecorderSource` con mapping → `time_base:"song"`) en `user://sessions/`. Con eso P5–P7
  se reproducirían con `jd_judge` sin `jd_record` aparte.
- **G**: incluir `tests/system/tests` en `scripts/run_tests.sh` y en la CI (solo necesita python, numpy y ffmpeg).
