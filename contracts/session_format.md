# Contrato · Formato de sesión grabada (B7)

Archivo de texto UTF-8, **JSON Lines** (`.jsonl`). Línea 1 = cabecera; resto = una muestra por línea.

## Cabecera (línea 1)

```json
{"format":"jd-session","version":1,
 "time_base":"song",            // "song": t es tiempo de canción (s); "mono": segundos desde el inicio de la grabación
 "song_id":"mi_cancion",        // o null
 "recorded_at":"2026-11-06T20:00:00Z",
 "note":"bien bailado, manos",  // texto libre
 "devices":[{"source_id":1,"name":"Joy-Con (L)","kind":"joycon","limb":"left_hand"},
            {"source_id":2,"name":"Joy-Con (R)","kind":"joycon","limb":"right_hand"}]}
```

## Muestra (resto de líneas)

```json
{"t":12.345678,"src":1,"limb":"left_hand","a":[0.1,9.8,0.2],"g":[0.01,-0.02,0.0]}
```

- `t`: segundos (double) según `time_base`.
- `a`: acelerómetro m/s² (incluye gravedad); `g`: giroscopio rad/s. Ejes según `contracts/motion_sample.h`.
- Campos ausentes (`a` o `g`) = la muestra no lo trae (flags).
- Las líneas que empiezan por `#` o vacías se ignoran.

## Conversión a JD_MotionSample

Al cargar en tiempo de canción para el juez: `t_ns = round(t * 1e9)` y `mono_to_song_s = 0`.
La reproducción en tiempo real (`jd_source_create_replay`) emite cada muestra cuando ha pasado
`t - t_primera` desde `start()`, con `t_ns = jd_now_ns()` de ese instante.

Sesiones de referencia de una coreografía se guardan en el paquete de canción (`ref_imu.jsonl`) con `time_base:"song"`.
