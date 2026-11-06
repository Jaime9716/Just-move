# Contrato · Clases de la GDExtension (lo que GDScript ve del motor)

Implementa el agente **G** en `game/gdextension/` envolviendo `contracts/engine_api.h`.
Los agentes D, E y T2 programan contra esto. Si la extensión no está cargada,
`ClassDB.class_exists("JDEngine")` es `false` y el juego usa simulaciones (`game/core/mock_engine.gd`, agente D).

## JDSource (RefCounted)

| Método | Descripción |
|---|---|
| `static create_joycon() -> JDSource` | null si no hay SDL3 |
| `static create_replay(path: String, speed: float = 1.0) -> JDSource` | null si el archivo no existe |
| `static create_recorder(inner: JDSource, out_path: String) -> JDSource` | |
| `start() -> bool` / `stop()` | |
| `devices() -> Array[Dictionary]` | `{id:int, name:String, kind:String, limb:int, connected:bool, battery:float}` |
| `assign_limb(id: int, limb: int)` | limb = valores de JD_Limb (0 mano izq … 3 pie der) |
| `set_recorder_mapping(mono_to_song_s: float)` | solo grabadores |
| `poll_samples(max: int = 512) -> Array[Dictionary]` | `{t_ns:int, id:int, limb:int, a:Vector3, g:Vector3}` — consume las muestras (para calibración y depuración; no usar a la vez que `JDEngine.pump`) |
| `last_error() -> String` | |

## JDEngine (RefCounted)

| Método | Descripción |
|---|---|
| `static now_ns() -> int` | `jd_now_ns()`; el reloj monotónico oficial |
| `load_song(song_dir: String) -> String` | "" si OK, si no el mensaje de error |
| `set_config(perfect_ms: float, good_ms: float, foot_scale: float)` | |
| `set_time_mapping(mono_to_song_s: float)` | cada fotograma, desde SongClock |
| `pump(source: JDSource) -> int` | lee todas las muestras de la fuente y las pasa al motor; devuelve cuántas |
| `update(song_time_s: float) -> Array[Dictionary]` | `{step:int, limb:int, grade:int, similarity:float, timing_error_ms:float}`; grade 0 Miss, 1 Good, 2 Perfect |
| `reset()` | |

Archivo `game/jd_engine.gdextension`, biblioteca `game/bin/jd_gdext.<plataforma>.dll|so` y
`jd_engine.dll|so` junto a ella.
