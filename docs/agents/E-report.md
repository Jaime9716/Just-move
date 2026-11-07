# Informe del agente E · UI de juego (B8)

Escrito para Godot 4.4, GDScript tipado. **No se ha podido ejecutar**: no hay Godot en el entorno;
revisado a mano dos veces. Usa tal cual lo de D (`SongClock` autoload, `EngineFactory`, `Profile`,
`MockJDEngine`/`MockJDSource`, escenas de calibración).

## Qué hay

### Lógica pura (`game/ui/`, RefCounted, contrato `game_logic_api.md`)
| Archivo | Contenido |
|---|---|
| `lane_logic.gd` | `LaneLogic.icon_y`, `visible_steps` (búsqueda binaria; `[visual_t − behind, visual_t + ahead]` ambos incluidos) |
| `video_sync_logic.gd` | `VideoSyncLogic.correction` (contrato) + extras del plan B: `plan_b()` → `none/hold/catch/lost`, `catch_up_speed()` |
| `score_logic.gd` | `ScoreLogic` (Perfect 100, Good 50, Miss 0; bonus con el combo **ya incrementado**: el primer Perfect da 101), `total()` |
| `song_library.gd` | `SongLibrary.scan()` en `user://songs/` + `<repo>/data/songs` (gana user://), `load_package(dir)` valida lo mínimo (format, version, song.title, duration_s, audio existe, steps ordenados, limbs válidos, kind) y devuelve rutas absolutas. Vídeo ausente no es error |
| `ui_session_library.gd` | `UiSessionLibrary.scan()` de sesiones `.jsonl` en `user://sessions` y `<repo>/data/sessions` (cabecera, song_id, time_base, primera t) |

### Interfaz
| Archivo | Contenido |
|---|---|
| `ui/ui_session.gd` | `UiSession`: **nodo persistente propio** colgado de `/root` (no autoload). Guarda canción elegida, elección de replay, último resultado y el **servidor de móviles compartido** (`ensure_phone_server()` con `Profile.phone_port`); lo para al cerrar. `pick_source()`: móviles conectados → replay → `EngineFactory.create_mock_source()`. Perfil compartido = `SongClock.profile`. `open_calibration()` instancia las escenas de D pasándoles `source` y `profile` |
| `ui/step_icons.gd` | 17 pictogramas dibujados con primitivas (sin imágenes ni nada con copyright), alias de nombres y **icono genérico** para lo desconocido |
| `ui/lane_view.gd` | Carril de 4 columnas (mano izq, pie izq, pie der, mano der), un solo `_draw` por fotograma, barras para `kind:"move"`, color por nota recibida |
| `ui/ui_style.gd` | Ayudas para construir la UI por código |
| `scenes/main_menu.{gd,tscn}` | Escena principal: lista de canciones (inválidas en gris con sus errores en el tooltip), info, selector "entrada si no hay móvil" (automática / simulación / sesión concreta), casilla log de fps, Jugar, Conectar móvil, Calibrar entrada, Calibrar pantalla, Recargar, Salir |
| `scenes/phone_pair.{gd,tscn}` | Endpoint y código de 4 dígitos en grande; lista de móviles con estado, batería, `sync_rtt_ms` (aviso naranja si > 30 ms) y OptionButton mano/pie → `assign_limb` + `Profile.limb_by_device[nombre]` guardado. Las asignaciones guardadas se reaplican solas al reconectar |
| `scenes/gameplay.{gd,tscn}` | Partida (abajo) |
| `scenes/results.{gd,tscn}` | Puntos, combo máximo, Perfect/Good/Miss, acierto, fuente usada, modo de vídeo; Repetir / Menú |
| `scenes/icon_gallery.{gd,tscn}` | Galería de iconos (F6) para revisarlos, incluido el genérico |

Eliminados del intento anterior (duplicaban `game/core`): `ui_clock.gd`, `ui_engine_access.gd`,
`ui_fallback_engine.gd`, `ui_fallback_source.gd`. (El comentario de `SongClock.gd` y el informe de D
aún mencionan `ui_clock.gd`; ya no existe: la partida usa el autoload directamente.)

### Partida (`gameplay.gd`)
1. Motor `EngineFactory.create_engine()` → `load_song(dir)` → `set_config(perfect_ms, good_ms, foot_scale)` del perfil.
2. Audio `AudioStreamOggVorbis.load_from_file(audio.ogg)` reproducido con `SongClock.play()` tras 2 s de cuenta atrás.
   Duración = `get_length()` del ogg (o `duration_s`).
3. Cada fotograma: `set_time_mapping(SongClock.engine_mapping(now))`, `pump(source)`,
   `update(SongClock.audible())` → `ScoreLogic.add`, color en el carril, popup PERFECT/GOOD/MISS + puntos
   en la columna de la extremidad. Carril con `SongClock.visual()`.
4. Fin: `SongClock.song_finished`, o red de seguridad (audible > duración + 1,5 s, o el audio dejó de
   sonar); se hace un último `update(duración + 60)` para recoger los pasos pendientes. Esc = menú.
5. Fuente: móviles si hay alguno conectado en el servidor compartido; si no, replay (sesión con el mismo
   `song_id` y `time_base:"song"`, o la elegida en el menú), que se arranca cuando
   `audible ≥ t_primera + input_offset` para que las muestras caigan en su tiempo de canción; si no, simulación.
   Replay y simulación se paran al salir; el servidor de móviles no.
6. `user://fps_log.csv` con cabecera `t_s,fps` (t desde el inicio de la escena, fps = 1/delta) si se
   marca la casilla o se lanza con `-- --fps-log` / `JD_FPS_LOG=1`.

### Vídeo esclavo del reloj
`VideoStreamTheora.file = video.ogv` en un `VideoStreamPlayer` (`volume = 0`, `expand`, dentro de un
AspectRatioContainer 16:9). **`stream_position` es asignable en 4.4** (`set_stream_position` → `seek` del
playback), pero hasta donde sé **`VideoStreamPlaybackTheora::seek` no está implementado en 4.4** (avisa
una vez "Seeking in Theora videos is not implemented yet"). Por eso:
- Plan A: `VideoSyncLogic.correction(stream_position, visual, 0.07)` → asignar `stream_position`
  (enfriamiento 0,5 s). Tras un seek de > 0,25 s se comprueba durante 8 fotogramas si la posición llegó.
- Plan B (automático si el seek no surte efecto): `plan_b()` → vídeo adelantado: `paused = true` hasta
  que el reloj lo alcanza; atrasado: `speed_scale` hasta 1,25 (o 2,0 si va > 2 s tarde).
- Umbral 0,07 s y no 0,04 (el del contrato por defecto): `stream_position` de Theora avanza a saltos
  de 1/30 s, con 0,04 se corregiría casi cada fotograma. Constante `VIDEO_THRESHOLD_S`.

## Autoloads necesarios
Solo `SongClock` (ya en `project.godot`). **G no tiene que añadir nada**: `UiSession` es un nodo propio que
se cuelga de `/root` con `call_deferred` y sobrevive a `change_scene_to_file`.
`run/main_scene="res://scenes/main_menu.tscn"` ya está bien.

## Cosas a verificar al abrir en Godot
1. Que todo compile (panel de errores). Dudosos: lambdas en `sort_custom` que llaman a estáticos;
   `static var _instance: UiSession` autorreferente; `AudioStreamOggVorbis.load_from_file` (estático, 4.2+).
2. `main_menu` sin extensión: aparece "Demo metrónomo" (data/songs) y el estado dice "motor SIMULADO".
3. Conectar móvil sin extensión: endpoint y código grandes; ~1 s después "Pixel (simulado)", RTT ~6–10 ms,
   batería 85 %. Cambiar a "Pie izquierdo", volver, reabrir: se mantiene; `user://profile.cfg` tiene
   `limb_by_device`. Con extensión y móvil real: RTT > 30 ms en naranja.
4. Calibrar entrada/pantalla desde el menú: abren las escenas de D con la fuente compartida; "Volver"
   regresa al menú; el servidor de móviles sigue abierto después.
5. Partida con la demo: cuenta atrás, iconos que llegan a la diana justo en el clic, popups y combo,
   resultados al acabar. Esc vuelve al menú sin errores (fuente propia parada, música parada).
6. **Vídeo**: (a) que `VideoStreamTheora.file` con ruta absoluta (user:// globalizada) cargue;
   (b) que no suene; (c) la línea de depuración arriba a la derecha muestra "seek" o "plan B" y la deriva:
   debe quedarse en ±70 ms; (d) comprobar si existe `VideoStreamPlayer.speed_scale` en 4.4 (se usa con
   `"speed_scale" in _video`; si no existe, el plan B solo puede pausar y un vídeo atrasado no se recupera
   → convendría re-codificar o subir a Godot 4.5, donde Theora tiene seek).
7. Rendimiento (P9): 60 fps con vídeo 720p; revisar `user://fps_log.csv` con la casilla marcada.
8. Replay: copiar una sesión con `song_id` = id de la canción y `time_base:"song"`; elegirla en el menú y
   comprobar que las notas cuadran con la sesión (y que con `input_offset` ≠ 0 no se desplazan).
9. Que el popup de varias extremidades a la vez no se tape (columnas distintas) y el genérico salga para
   iconos desconocidos (`icon_gallery.tscn`).
10. Cerrar la ventana con el servidor abierto: sin avisos de puerto ocupado al relanzar.

## Qué queda / no verificado
- Nada ejecutado (sin Godot). Las pruebas de lógica pura son de T2 (headless).
- Sin pausa en partida (Esc abandona). Sin selector de dificultad ni velocidad de carril en la UI
  (`LaneView.speed`).
- `battery` se interpreta 0–1 (contrato) y, por si acaso, > 1 como porcentaje.

## Propuestas de contrato
- `VideoSyncLogic.correction`: documentar que la UI usa umbral 0,07 s por la cuantización de Theora.
- `game_logic_api.md`: documentar el bonus de combo con el combo ya incrementado (1.º Perfect = 101).
