# Informe del agente D · Sincronía y configuración en Godot (B2, B9)

Todo en `game/core/` (res://core/). GDScript tipado para Godot 4.4. **No se ha podido ejecutar**:
no hay Godot en el entorno; revisado a mano dos veces.

## Qué hay

| Archivo | Contenido |
|---|---|
| `song_clock_logic.gd` | `SongClockLogic` (RefCounted), exactamente como `contracts/game_logic_api.md` |
| `calibration_logic.gd` | `CalibrationLogic.estimate_offset` / `detect_shakes` (contrato). detect_shakes ordena por `t_ns`, lleva el estado por `id` y fusiona cruces a < `min_sep` |
| `profile.gd` | `Profile` (campos del contrato, `save`/`load`, `reset_defaults`, `to_dict`, `load_or_default`). Formato JSON (`format:"jd-profile"`) con precisión completa aunque se llame `.cfg`; corrupto → valores por defecto y `false` |
| `SongClock.gd` | Autoload (ya registrado en `project.godot`). Sin `class_name` (el nombre global es el del autoload) |
| `engine_factory.gd` | `EngineFactory`: estáticos `has_extension`, `create_engine`, `create_phone_server(port, code)`, `create_replay`, `create_recorder`, `create_mock_source`, `now_ns`, `now_s`, `call_static`, `is_mock`, `describe`; `force_mock` para depurar |
| `mock_engine.gd` | `MockJDEngine` con la interfaz de JDEngine (load_song lee `choreo.json` con errores con ruta JSON; juez simulado: Miss si la extremidad no se movió en la ventana, si no nota pseudoaleatoria con semilla) |
| `mock_source.gd` | `MockJDSource` con la interfaz de JDSource: servidor de móviles con un móvil falso "Pixel (simulado)" (right_hand, 200 Hz), `endpoint`, `pairing_code`, reproducción `.jsonl`, grabador; extras `simulate_rhythm`, `inject_shake`, `set_seed`. Se mantiene en archivo propio (un `class_name` por archivo) |
| `calibration/click_track.gd` | `ClickTrack.make()` (AudioStreamWAV 16 bit generado en ejecución, con bucle opcional) y `click_times()` |
| `calibration/calibration_input.{gd,tscn}` | Calibración audio + entrada → `input_offset` |
| `calibration/calibration_visual.{gd,tscn}` | Calibración de pantalla → `visual_offset` |

### SongClock (API)
`play(stream, from_s=0)`, `stop()`, `seek(t)`, `pause()/resume()/set_paused(b)/is_paused()`, `is_playing()`,
`attach_player(p)` / `detach_player()` (seguir el AudioStreamPlayer de otra escena; lo usa `ui/ui_clock.gd`),
`reset(t=0)`, `raw_position()`, `song_time()`, `audible()`, `visual()`, `engine_mapping(now_mono=NAN)`,
`input_to_song(t_mono_s)`, `sample_to_song(t_ns)`, `now_ns()`, `now_mono_s()`, `apply_profile(p)`,
`reload_profile(path)`, `save_profile(path)`; propiedades `audio_offset/visual_offset/input_offset`
(reenvían a `logic`), `logic`, `profile`, `player`; señales `song_started`, `song_finished` (solo fin natural).

- Posición bruta = `get_playback_position() + get_time_since_last_mix() − get_output_latency()` → `logic.step()`.
- `process_priority = -1000`: se actualiza antes que las escenas.
- `engine_mapping()`: mientras suena devuelve `logic.engine_mapping(mono del instante del step)`; el
  argumento se ignora porque canción y reloj avanzan juntos dentro del fotograma (así no importa cuándo lo
  pida E). Parado/en pausa usa el argumento (o ahora).
- Tras `seek()` ignora hasta 10 fotogramas en que la posición bruta siga siendo la antigua.

### Reloj (JDEngine.now_ns)
`EngineFactory.now_ns()`: si existe `JDEngine` instancia una vez la clase y llama `now_ns` sobre esa
instancia (en Godot un método estático de ClassDB se puede invocar sobre una instancia; es lo más barato);
si no se puede instanciar, usa `ClassDB.class_call_static` (existe desde 4.4, se llama vía `callv` tras
`ClassDB.has_method`, así compila igual). Sin extensión: `Time.get_ticks_usec()·1000`. MockJDSource,
MockJDEngine y SongClock usan este mismo reloj, así que muestras simuladas y canción comparten base.

### Calibración
- **Audio + entrada**: 16 clics a 100 BPM tras 2 s. Sacudidas = `detect_shakes(poll_samples)` (umbral
  25 m/s²); Espacio como alternativa (mide el teclado: se indica). Eventos → tiempo audible con
  `t_mono + engine_mapping() + input_offset` (= mapping sin input_offset), `estimate_offset` → si `ok`,
  "Guardar" escribe `profile.input_offset`. El retraso de audio no cubierto por `get_output_latency` queda
  incluido en `input_offset` (con un solo test no se separan); `audio_offset` se deja como esté (0).
  Con `MockJDSource` programa sacudidas simuladas (≈ +22 ms esperados: 30 ms de latencia − ~8 ms por cruzar
  el umbral antes del pico).
- **Pantalla**: metrónomo en bucle (8 clics, 0,75 s), cuadro que destella con `SongClock.visual()` y barra
  que cruza el centro en el clic; HSlider −150…+300 ms (y ±5 ms) aplica `visual_offset` en vivo; "Volver"
  sin guardar lo deshace.
- Instanciación: `.tscn` mínimos (un Control a pantalla completa con el script; la UI se construye en
  `_ready`). Opcional antes de `add_child`: `source` (fuente ya arrancada, no se para al salir), `profile`,
  `profile_path`. Señal `closed(saved)`. Si es la escena actual, "Volver" va a la escena principal; si nadie
  escucha `closed`, hace `queue_free()`. Sin autoload crean un SongClock local hijo.

## Compatibilidad con E (game/ui/)
`ui_clock.gd` encuentra `attach_player`, `detach_player`, `reset`, `apply_profile`, `now_mono_s`,
`engine_mapping(now)`; `ui_engine_access.gd` encuentra en `engine_factory.gd` `create_engine`,
`create_phone_server`, `create_replay`, `create_mock_source`, `now_ns` (estáticos). Sin choques con el contrato.

## Cosas a verificar al abrir en Godot
1. Que todo compile sin errores (Proyecto → recargar; mirar el panel de errores). Puntos dudosos:
   - `Profile.load()` se llama igual que la función global `load()`: debería ser solo el aviso
     SHADOWED_GLOBAL_IDENTIFIER. Si fuese error, el contrato tendría que renombrarlo (p. ej. `load_from`).
   - Referencias circulares `class_name` (EngineFactory ↔ MockJDSource/MockJDEngine) y `preload` de
     `SongClock.gd` usado como tipo en las escenas de calibración.
   - Propiedades con `get/set` en el autoload (`audio_offset` …).
2. Con la extensión: `EngineFactory.clock_description()` debe decir "JDEngine.now_ns (instancia)" (o
   class_call_static); comprobar que `now_ns` de godot-rust está registrado como estático y es invocable
   así. Comparar `EngineFactory.now_ns()` con `t_ns` de muestras reales: deben estar a < 50 ms de ahora.
3. Abrir `res://core/calibration/calibration_input.tscn` (F6) sin extensión: aparece "Pixel (simulado)"
   ~1 s después; Empezar → a los ~12 s resultado ≈ +20 ms con ~16 golpes. Probar también solo Espacio.
4. Con móvil real: un golpe seco supera 25 m/s²; si no, bajar el umbral (constante en `detect_shakes`).
5. `calibration_visual.tscn`: el destello y el clic deben poder alinearse; el bucle no debe dar saltos
   raros al volver a 0 (la posición bruta salta −6 s y `step` lo acepta).
6. Que el clic generado suene limpio (AudioStreamWAV 44,1 kHz mono 16 bit, `encode_s16`).
7. `user://profile.cfg` se crea al guardar y se recarga al arrancar (SongClock `_ready`).
8. `get_playback_position()` en pausa por `get_tree().paused`: SongClock hereda el modo de proceso
   (no avanza en pausa). Revisar si algún menú en pausa necesita el reloj.
9. Rendimiento: `detect_shakes` se recalcula cada 10 fotogramas sobre ≤ ~3000 muestras durante la calibración.

## Qué queda / propuestas de contrato
- Sin pruebas ejecutadas (las de lógica pura las hace T2 en headless).
- Propuesta: documentar en `game_logic_api.md` la API del autoload SongClock (arriba) y
  `EngineFactory` (los nombres que ya usa E).
- Propuesta: si `Profile.load` diera problemas, renombrar a `load_from`.
