# Contrato · Paquete de canción y coreografía (B6)

Una canción es una carpeta en `data/songs/<song_id>/` (en el juego: `user://songs/<song_id>/`):

| Archivo | Obligatorio | Contenido |
|---|---|---|
| `choreo.json` | sí | Coreografía (abajo) |
| `audio.ogg` | sí | Audio Ogg Vorbis, 44.1 o 48 kHz, estéreo |
| `video.ogv` | no | Vídeo del bailarín, Ogg Theora **sin audio**, ≤ 720p, 30 fps (formato nativo de Godot) |
| `ref_imu.jsonl` | no* | Sesión de referencia IMU (ver `session_format.md`, `time_base:"song"`) |
| `meta.json` | no | Origen: `{"source":"youtube","url":"...","imported_at":"...","personal_use_only":true}` |

\* Sin referencia IMU el juez usa solo los rasgos genéricos (energía y pico en el beat).

**Origen de los medios:** el importador `tools/youtube_import/` crea el paquete desde una URL de YouTube
con yt-dlp + ffmpeg, **solo para uso personal** (incumple las condiciones de YouTube; un juego
distribuido no puede incluir esta función ni esos archivos).

## choreo.json

```json
{
  "format": "jd-choreo",
  "version": 1,
  "song": {
    "title": "Mi canción",
    "artist": "Artista",
    "audio": "audio.ogg",
    "video": "video.ogv",          // o null
    "bpm": 120.0,
    "offset_s": 0.0,                // instante del primer beat en el audio
    "duration_s": 180.0
  },
  "steps": [
    {"t": 4.0, "dur": 0.5, "limbs": ["right_hand"], "icon": "punch_up", "kind": "accent"},
    {"t": 4.5, "dur": 1.0, "limbs": ["left_hand","right_hand"], "icon": "wave", "kind": "move"}
  ],
  "references": { "imu": "ref_imu.jsonl" }   // o {}
}
```

- `t`: tiempo de canción (s) del momento clave del paso; `dur`: duración del movimiento (s).
- `kind`: `"accent"` = golpe puntual juzgado por el pico en el beat (timing); `"move"` = movimiento
  continuo juzgado por similitud de la señal en `[t, t+dur]`.
- `steps` ordenados por `t`. `limbs` no vacío. Iconos: texto libre que la UI mapea a imágenes
  (desconocido → icono genérico).
- Validación: errores con ruta JSON y mensaje claro (p. ej. `steps[3].t: debe ser >= steps[2].t`).
