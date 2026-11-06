# Informe del agente A · Fuente móvil (B1) y sesiones (B7)

## Qué hice

| Archivo | Contenido |
|---|---|
| `engine/jd_core/src/session.rs` | `load_session` / `save_session` según `contracts/session_format.md`, más `parse_session_str`, `header_line`, `sample_line_with_t`, `format_ns_as_seconds`, `now_iso8601_utc` |
| `engine/jd_core/src/replay.rs` | `ReplaySource` (MotionSource): reproducción en tiempo real desde `start()`, `speed` configurable, reloj inyectable (`with_clock`), `assign_limb` |
| `engine/jd_core/src/recorder.rs` | `RecorderSource`: envuelve `Box<dyn MotionSource>`, reenvía `poll()` y escribe JSONL poco a poco (BufWriter, flush cada 500 ms) |
| `engine/jd_core/src/phone/` (antes `phone.rs`) | `protocol.rs` (codificar y decodificar mensajes), `clock_sync.rs` (offset y deriva), `server.rs` (`PhoneServerSource`), `sim.rs` (simulador de móvil, lo usan el bin y los tests) |
| `engine/jd_capi/src/sources.rs` | Todas las `jd_source_*` de `engine_api.h`, `JD_DeviceInfoC` `#[repr(C)]` (96 bytes, offsets comprobados en un test) |
| `engine/jd_tools/src/bin/phone_sim.rs` | Simula un móvil: HELLO, SYNC, SAMPLES a 200 Hz con sacudidas periódicas, STATUS; también `--discover` |
| `engine/jd_tools/src/bin/jd_record.rs` | Servidor más grabación JSONL; muestra endpoint y código; por móvil muestra Hz, rtt, deriva, pérdidas y batería |
| `engine/jd_core/tests/a_phone_loopback.rs` | Integración en 127.0.0.1: tiempos ±2 ms (offset de +3 días y 50 ppm), código incorrecto, DISCOVER y basura, timeout y reconexión (HELLO nuevo y reanudación con el mismo socket), `assign_limb` |

### Detalles de comportamiento

- **Sesiones:** la cabecera es la primera línea que no está vacía y no empieza por `#`. Se exigen `format` = `jd-session` y `version` = 1. Si falta `time_base`, se toma `"song"`. `song_id` nulo se lee como `""` (y al guardar, `""` se escribe como `null`). Si una muestra no trae `limb`, se usa el del dispositivo de la cabecera. Los flags salen de la presencia de `a` y `g`. `t_ns = round(t*1e9)`. Los errores tienen la forma `ruta:línea: mensaje`. Al guardar, `t` sale exacto al ns a partir de los ns enteros, así que la ida y vuelta es idéntica byte a byte.
- **Replay:** `t_ns = t0 + (t − t_primera)/speed`, es decir, el instante equivalente y no el del `poll()`, para evitar el jitter de fotograma. Las muestras se ordenan de forma estable por t. `kind` = `"replay"`.
- **Recorder:** escribe la cabecera con la primera muestra; para entonces ya conoce los dispositivos y si se llamó a `set_mapping`. Con mapping se graba `time_base:"song"` y t = t_ns·1e-9 + el mapping vigente. Sin mapping, `"mono"`, en segundos desde `start()`. Al hacer `stop()` o `Drop`, la línea 1 se reescribe si cambiaron los dispositivos o las extremidades. Si `set_mapping` llega después de una cabecera `"mono"`, la base no cambia y se anotan `mono_to_song_s` y `mono_start_ns` como campos extra.
- **Sincronización de reloj:** se aplican las fórmulas del contrato, con una ventana de 16 y como ancla la respuesta de menor rtt. Se descartan las respuestas con rtt < 0 o > 2 s y las de `t_pc_send` no plausible. **Deriva:** una recta ajustada solo con la ventana de 16 (4 a 16 s) resultaba demasiado ruidosa con jitter de Wi-Fi: hasta 250 ppm de error, peor que no corregir, y los tests lo mostraron. Por eso:
  - cada bloque de 8 respuestas aporta su mínimo de rtt a un historial de 32 bloques (unos 4 min);
  - la recta se ajusta por mínimos cuadrados ponderados (σ ≈ rtt/5);
  - la pendiente solo se usa si su error típico es menor de 30 ppm y el historial abarca 15 s o más. Hasta entonces la pendiente es 0.
  Sigue cumpliendo el contrato (deriva con ≥ 8 respuestas), pero con más memoria.
- **Servidor:**
  - Hilo propio con `recv_from` y un timeout de 2 ms, en vez de sockets no bloqueantes con `sleep`: en Windows `sleep(1ms)` dura hasta 15 ms y estropearía `t_pc_recv`. El juego nunca se bloquea porque `poll()` solo vacía una cola protegida por mutex.
  - En Windows se ignora `ConnectionReset` (WSAECONNRESET tras un ICMP).
  - Las muestras que llegan antes del primer SYNC_RESP se guardan en un búfer (hasta 2000) y se convierten después.
  - `t_ns` es estrictamente creciente por dispositivo; los duplicados y desordenados se descartan por `t_phone`.
  - Las pérdidas se cuentan por huecos de `seq`.
  - Extremidad por defecto: la primera libre entre los móviles conectados, en el orden right_hand, left_hand, right_foot, left_foot; después `none`. Un móvil que reconecta conserva su `source_id` y su extremidad.
  - Al hacer `assign_limb` se reenvía WELCOME para que el móvil vea su extremidad.
  - Si un móvil ya emparejado envía SAMPLES tras más de 3 s de silencio desde la misma dirección, se reanuda sin reiniciar el reloj. Un HELLO tras desconexión sí reinicia la sincronización.
  - Al parar se envía BYE.
  - La IP LAN se obtiene con `connect` a 8.8.8.8:80 sin enviar nada; si falla, se usa 127.0.0.1.
  - `PhoneServerHandle` (clonable) da acceso a endpoint, código y estadísticas aunque la fuente esté envuelta.
- **API C:** cada `JD_Source` guarda la fuente en `Arc<Mutex<dyn MotionSource>>`. Tras `jd_source_create_recorder(inner, ..)` siguen siendo válidos los dos punteros (se destruyen por separado), pero solo se debe hacer poll del recorder. `jd_source_phone_endpoint` también funciona sobre un recorder que envuelve un servidor. `jd_source_poll` guarda lo que no cabe en `max` para la siguiente llamada. Los nombres se truncan en un límite de carácter UTF-8. Ninguna función falla con punteros nulos. El puerto 0 se traduce a 47800 en la API C (en Rust, 0 significa puerto efímero, que usan los tests).

## Cómo probarlo

```
cd engine && CARGO_TARGET_DIR=../build/A cargo test -p jd_core -p jd_capi -p jd_tools   # 87 tests, sin warnings
# a mano, en dos terminales:
../build/A/debug/jd_record /tmp/s.jsonl --seconds 10 --code 2468
../build/A/debug/phone_sim 127.0.0.1:47800 --code 2468 --seconds 8 --drift-ppm 50 --offset-s 1000
```

Lo comprobé a mano: con un código incorrecto el móvil queda rechazado; con el correcto da 200 Hz, rtt de 0,1 ms, batería del 80 % y BYE al terminar (pasa a DESCONECTADO), y el JSONL resultante es válido.

## Qué no pude verificar

- No pude probar con un móvil real ni con Wi-Fi: la precisión real depende de la app P (marcas de tiempo `elapsedRealtimeNanos` y respuesta inmediata a SYNC).
- No pude probar en Windows (ConnectionReset, firewall, resolución del timeout de 2 ms). En el entorno de la nube la "IP LAN" detectada fue 192.0.2.2.
- Ctrl+C en `jd_record`: sin `unsafe` ni crates no se puede capturar SIGINT. Se pierde como mucho el último medio segundo y la reescritura final de la cabecera. Es mejor terminar con Enter o con `--seconds`.

## Pendiente

- Respuesta a DISCOVER por broadcast: probada solo por unicast en loopback.
- Las estadísticas de pérdidas no se exponen en `DeviceInfo` (el contrato no tiene campo); están en `PhoneServerHandle::stats()`.

## Propuestas de contrato

1. `jd_source_create_replay` / `create_recorder`: añadir `char* err, int err_len`, como `jd_engine_load_song`. Hoy, si la sesión no carga, devuelvo una fuente válida que falla en `start()` (-2) con `last_error` relleno; la ruta nula sí devuelve NULL. `create_recorder` devuelve NULL si no puede crear el archivo y deja el error en `last_error(inner)`.
2. `JD_DeviceInfoC`: añadir `uint32_t lost_packets` y `float rate_hz` para la pantalla de calidad del mando.
3. `session_format.md`: documentar los campos extra opcionales `mono_to_song_s` y `mono_start_ns` de la cabecera, y que `src`/`limb` pueden omitirse en una muestra (se heredan de la cabecera).
4. `phone_protocol.md`: aclarar que el PC reenvía WELCOME (aceptado) cuando cambia la extremidad, y que el móvil debe volver a enviar HELLO si no recibe nada del PC durante 3 s (es lo que hace el simulador).
