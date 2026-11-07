# Feedback del mundo real y de los sensores

El mando es un **móvil Android** conectado por **Wi-Fi** (app de mando, `android/`), y las canciones se
importan de **YouTube** (uso personal) o son la demo `demo_metronomo`. Ningún hito se cierra sin tu
feedback. Te lo pediré en estos momentos:

| Momento | Qué te pediré | Pruebas | Para qué se usa |
|---|---|---|---|
| Fase 0 | Grabar 3–5 sesiones reales con `jd_record` (móvil en la mano y en el tobillo, bien y mal bailado) y cómo fue sujetar/atar el móvil | guía `tests/system/guides/fase0_grabar_sesiones.md` | Datos reales con los que trabajan todos los agentes |
| H0 | Si la puntuación del prototipo te parece justa en cada paso | P5 | Ajustar el juez antes de portarlo |
| I1 (H1 + H2) | Emparejado y reconexión del móvil, estabilidad por Wi-Fi, sincronía audio–imagen, calibración; si notas retraso entre música, imagen y movimiento | P1–P4 | Ajustar timestamps, reloj del móvil y calibración |
| I2 (H3) | Puntuación con otra persona y con el móvil en el pie; calidad de las canciones de YouTube y sincronía del vídeo del bailarín | P5–P7 | Ajustar ventanas de acierto, rasgos e importador |
| I3 (H4, MVP) | Wi-Fi saturada, batería baja, pantalla bloqueada; fps; primera vez sin ayuda; si es divertido | P8–P10 | Decidir qué se arregla antes de Linux |
| H5 | Repetir P1–P5 en Linux | P11 | Paridad Windows/Linux |
| Cuando algo falle | Qué hiciste, qué esperabas, qué pasó y la sesión grabada | — | Reproducirlo sin hardware y convertirlo en test |

## Cómo dar feedback

1. Copia `plantilla.md` como `docs/feedback/AAAA-MM-DD-<momento>.md` (p. ej. `2026-11-20-I1.md`).
2. Rellena **Entorno** (modelo de móvil, Wi-Fi 2,4/5 GHz, RTT, audio Bluetooth o no…) y **Canción**
   (si es de YouTube: calidad del audio y del vídeo importados y si el bailarín va sincronizado con la música).
3. Para cada prueba sigue su guía en `tests/system/guides/` y pega la salida de los scripts de
   `tests/system/tools/` (todos admiten `--help` y `--json`). Apunta también la fila en tu copia de
   `tests/system/results_template.csv` (`tests/system/results.csv`).
4. Deja las sesiones en `data/sessions/real/` con nombres descriptivos
   (`2026-11-20_mano-der_bien_demo.jsonl`). Los vídeos de P3 y los `fps_log.csv` pueden ir en
   `tests/system/results/` (no subas vídeos grandes ni canciones de YouTube al repositorio).
5. Resumen de todo el feedback: `python tests/system/tools/feedback_summary.py` (o `--json`).
