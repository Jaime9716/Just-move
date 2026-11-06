# Contrato · Protocolo móvil → PC (mando por Wi-Fi)

El móvil Android (app de mando, agente P) envía su acelerómetro y giroscopio al PC (servidor del
motor, agente A) por **UDP** en la red local. Todo en **little-endian**. Puerto por defecto **47800**.

## Cabecera común (16 bytes)

| Offset | Tipo | Campo |
|---|---|---|
| 0 | `[u8;4]` | magic `"JDM1"` |
| 4 | `u8` | type (tabla abajo) |
| 5 | `u8` | version = 1 |
| 6 | `u16` | reservado (0) |
| 8 | `u32` | device_id (aleatorio, persistente por instalación de la app) |
| 12 | `u32` | seq (creciente por emisor; detecta pérdidas) |

## Mensajes

| type | Nombre | Sentido | Cuerpo |
|---|---|---|---|
| 1 | HELLO | móvil→PC | `u32 pairing_code`, `u8 name_len`, `name_len` bytes UTF-8 (modelo del móvil) |
| 2 | WELCOME | PC→móvil | `u32 source_id`, `u8 limb` (JD_Limb), `u8 accepted` (0 = código incorrecto) |
| 3 | SAMPLES | móvil→PC | `u16 count`, `u16 reservado`, luego `count` × muestra (36 bytes) |
| 4 | SYNC_REQ | PC→móvil | `i64 t_pc_send` |
| 5 | SYNC_RESP | móvil→PC | `i64 t_pc_send`, `i64 t_phone_recv`, `i64 t_phone_send` |
| 6 | STATUS | móvil→PC | `f32 battery` (0..1, −1 desconocido), `u16 sensor_rate_hz`, `u16 dropped` |
| 7 | BYE | ambos | vacío |
| 8 | DISCOVER | móvil→broadcast | vacío (el PC responde con WELCOME `accepted=0` y `source_id=0` para darse a conocer) |

**Muestra (36 bytes):** `i64 t_ns` (reloj del móvil), `u8 flags` (JD_SAMPLE_HAS_*), `[u8;3]` relleno,
`f32 accel[3]` (m/s², incluye gravedad), `f32 gyro[3]` (rad/s).

## Reglas

- **Emparejado:** el PC muestra IP:puerto y un código de 4 dígitos. El móvil envía HELLO hasta recibir
  WELCOME (reintento cada 500 ms). Sin WELCOME aceptado, el PC ignora SAMPLES de ese device_id.
- **Muestras:** sensores a 200 Hz (`SENSOR_DELAY_FASTEST` limitado a 200 Hz); se agrupan y se envía un
  paquete cada ~10 ms (≤ 8 muestras). Android entrega acelerómetro y giroscopio en eventos separados:
  la app combina cada muestra de acelerómetro con la última de giroscopio (|Δt| ≤ 5 ms → flags ambos).
- **Reloj del móvil:** `t_ns` en la base de `SystemClock.elapsedRealtimeNanos()`. `SensorEvent.timestamp`
  suele usar esa base; si al recibirlo difiere más de 1 s de `elapsedRealtimeNanos()`, la app corrige con
  la diferencia medida.
- **Sincronización de reloj (NTP simplificado):** el PC envía SYNC_REQ cada 250 ms durante los primeros
  5 s y luego cada 1 s. El móvil responde de inmediato. En el PC, al recibir en `t_pc_recv`:
  `rtt = (t_pc_recv − t_pc_send) − (t_phone_send − t_phone_recv)`,
  `offset = ((t_phone_recv − t_pc_send) + (t_phone_send − t_pc_recv)) / 2`.
  Se usa el `offset` de la respuesta con **menor rtt** de las últimas 16; y una recta (deriva) si hay ≥ 8.
  Muestra en el reloj del PC: `t_pc = t_phone − offset`. `JD_MotionSample.t_ns = t_pc`.
- **Calidad:** el PC expone el rtt mínimo actual (ms) en la información del dispositivo; > 30 ms → aviso.
- **Timeout:** sin paquetes de un móvil en 3 s → desconectado; vuelve con HELLO sin reiniciar el juego.
- **Extremidad por defecto:** el primer móvil emparejado = `right_hand` (se sujeta en la mano derecha,
  pantalla hacia arriba); los siguientes, `left_hand`, `right_foot`, `left_foot`. Cambiable desde el juego.
- **Ejes del móvil (Android):** X a la derecha de la pantalla, Y hacia arriba de la pantalla, Z saliendo de la
  pantalla. Coincide con el convenio de `motion_sample.h` (X derecha, Y arriba, Z hacia el jugador).
