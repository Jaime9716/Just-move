# Plan de pruebas de alto nivel (mundo real) · mando = móvil Android por Wi-Fi

Sustituye a la tabla de `docs/07-plan-pruebas-alto-nivel.md` (escrita para Joy-Con). **Tú ejecutas** cada
prueba en tu PC Windows con tu móvil Android siguiendo su guía (`guides/`); los scripts de `tools/`
miden y dicen APROBADO/SUSPENDIDO; apuntas el resultado en `results.csv` (copia de
`results_template.csv`) y tu impresión en `docs/feedback/` (copia de `plantilla.md`). T3 analiza tus datos.

Antes de nada: **Fase 0** (`guides/fase0_grabar_sesiones.md`): grabar 3–5 sesiones reales.

| ID | Prueba | Procedimiento (resumen) | Medida | Criterio de aceptación | Herramienta | Hito |
|---|---|---|---|---|---|---|
| P1 | Emparejado y reconexión | Emparejar el móvil (IP:puerto + código), código erróneo, cortar Wi-Fi del móvil 10 s, cerrar y abrir la app, segundo móvil si hay | Tiempo hasta "Conectado", tiempo de reconexión, extremidad conservada | Empareja en < 30 s la 1ª vez; código erróneo rechazado; reconecta en < 5 s tras volver la Wi-Fi **sin reiniciar el juego** y con la misma extremidad; el juego no se cuelga | cronómetro + pantalla "Conectar móvil" | H1 |
| P2 | Estabilidad de muestras y pérdidas por Wi-Fi | `jd_record` 10 min en reposo y 10 min en movimiento, a 3 m del router; repetir a 10 m / otra habitación | Tasa por dispositivo, huecos > 50 ms, pérdidas, t monotónico, rangos, RTT | 200 Hz ± 5 %; 0 huecos > 50 ms (tolerable ≤ 2 en 10 min a 10 m); pérdidas ≤ 1 %; t monotónico; sin valores imposibles; RTT < 30 ms | `tools/session_stats.py` | H1 |
| P3 | Sincronía audio–imagen | Grabar con OTRO móvil (cámara lenta) la pantalla y el sonido: (a) escena "Calibrar pantalla" (destellos + clics) 3 min; (b) canción `demo_metronomo` larga (vídeo del bailarín con destellos) | Desfase destello–clic al principio y al final, deriva | (a) \|desfase\| ≤ 10 ms al principio y al final; (b) vídeo del bailarín ≤ 40 ms (nunca > 70 ms, umbral de corrección del juego) y sin deriva creciente | `tools/av_sync_from_video.py` | H2 |
| P4 | Repetibilidad de calibración | "Calibrar entrada" 5 veces seguidas con el móvil en la mano (y 5 en el pie) | Desfase medido en cada intento | Rango (máx − mín) < 10 ms; desviación de cada intento ("±") < 15 ms | `tools/calib_stats.py` | H2 |
| P5 | Justicia de la puntuación | `demo_metronomo` (y una de YouTube) 4 veces: bien, a destiempo (medio beat tarde), pasos equivocados, quieto | Perfect/Good/Miss y % de acierto de la pantalla de resultados | Bien → ≥ 70 % Perfect+Good; destiempo y pasos equivocados → ≤ 40 %; quieto → ≥ 95 % Miss; orden bien > equivocado ≥ quieto; te parece justo | pantalla de resultados + `results.csv` | H0, H3 |
| P6 | Otra persona | Alguien que no conoce el juego baila la misma canción dos veces (con su mano o tu móvil) | % de acierto 1ª y 2ª vez, opinión | La 2ª vez ≥ la 1ª (+5 puntos o más); dice que es justo (≥ 3/5) | resultados + feedback | H3 |
| P7 | Móvil en el pie/tobillo | Repetir P5 con el móvil atado al tobillo (calcetín/brazalete), asignado a "pie" | Igual que P5 para el pie; ¿se suelta? | Mismo orden que P5 (bien > mal ≥ quieto); bien → ≥ 60 %; quieto → ≥ 95 % Miss; el móvil no se suelta en una canción | resultados + `session_stats.py` (sesión de pie) | H3 |
| P8 | Robustez | (a) Wi-Fi saturada (vídeo 4K/descarga en otro aparato); (b) batería < 15 % y ahorro de energía; (c) bloquear la pantalla / cambiar de app a mitad de canción y volver; (d) movimientos bruscos | Pérdidas/RTT con carga; comportamiento al bloquear; ¿avisa? | Con carga: pérdidas ≤ 5 % y sin huecos > 200 ms o aviso naranja de RTT; al bloquear/cambiar de app: el juego avisa "desconectado" y sigue sin colgarse; al volver, reconecta en < 5 s; nunca se cuelga ni se cierra | `session_stats.py`, observación | H4 |
| P9 | Rendimiento | Canción completa de YouTube (vídeo 720p) con "log de fps" activado | fps medio, 1 % peor, tirones | fps medio ≥ 58,8; ≥ 99 % de fotogramas ≥ 54 fps; 0 fotogramas > 50 ms (tras los 2 primeros s) | `tools/fps_log_stats.py` | H4 |
| P10 | Primera vez | Alguien nuevo: instalar APK, abrir juego, conectar, calibrar y terminar una canción **sin ayuda** | Tiempo total y por fase, dudas | < 5 min desde abrir el juego (APK ya instalada) hasta la pantalla de resultados, sin preguntar | cronómetro + feedback | H4 |
| P11 | Linux | Repetir P1–P5 en Linux (`scripts/build_linux.sh`) | Las de P1–P5 | Mismos criterios que en Windows; diferencias ≤ 5 ms en P3/P4 | las mismas | H5 |

## Reglas generales

- Anota siempre el **entorno**: modelo de móvil y versión de Android, banda Wi-Fi (2,4/5 GHz), distancia
  al router, PC por cable o Wi-Fi, salida de audio (¡Bluetooth añade 100–250 ms!), monitor/tele.
- Cada medida es una fila de `results.csv` (`id,fecha,entorno,medida,criterio,cumple,notas`).
- Sesiones a `data/sessions/real/`; vídeos de P3 y `fps_log.csv` a `tests/system/results/` (no se
  suben vídeos grandes ni canciones de YouTube al repositorio).
- Si algo falla: rellena "Fallos" en el feedback y guarda la sesión: es lo que permite reproducirlo sin móvil.
- Todos los scripts: `python tests/system/tools/<script>.py --help`; `--json` para salida analizable;
  código de salida 0 = aprobado, 1 = suspendido, 2 = error.

## Guías

| Guía | Prueba |
|---|---|
| `guides/00_preparacion.md` | Preparación común (herramientas, red, rutas) |
| `guides/fase0_grabar_sesiones.md` | Fase 0 |
| `guides/P1_emparejado.md` … `guides/P11_linux.md` | Una por prueba |

Tests de los scripts (datos sintéticos, ffmpeg): `python -m pytest tests/system/tests -q`.
