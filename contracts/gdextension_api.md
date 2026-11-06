# Contrato · Clases de la GDExtension (lo que GDScript ve del motor)

Implementa el agente **G** en Rust con **godot-rust (gdext)** en `engine/jd_godot/`, usando directamente
`jd_core` (no hace falta pasar por la API C). Compatible con Godot 4.4.
Los agentes D, E y T2 programan contra esto. Si la extensión no está cargada,
`ClassDB.class_exists("JDEngine")` es `false` y el juego usa simulaciones (`game/core/mock_engine.gd`, agente D).

## JDSource (RefCounted)

| Método | Descripción |
|---|---|
| `static create_phone_server(port: int = 47800, pairing_code: int = 0) -> JDSource` | servidor de móviles-mando (contracts/phone_protocol.md); code 0 = aleatorio |
| `endpoint() -> String` | "IP:puerto" de la LAN, para mostrar en pantalla |
| `pairing_code() -> int` | código de 4 dígitos |
| `static create_replay(path: String, speed: float = 1.0) -> JDSource` | null si el archivo no existe |
| `static create_recorder(inner: JDSource, out_path: String) -> JDSource` | |
| `start() -> bool` / `stop()` | |
| `devices() -> Array[Dictionary]` | `{id:int, name:String, kind:String, limb:int, connected:bool, battery:float, sync_rtt_ms:float}` |
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

Archivo `game/jd_engine.gdextension`, biblioteca `game/bin/jd_godot.dll` (Windows) / `libjd_godot.so` (Linux).
