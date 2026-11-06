# 02 · Sincronización de imagen, música y Joy-Con

## Regla principal

**La música es el reloj maestro.** En cada fotograma se calcula en qué segundo de la canción estamos y
todo se dibuja y se juzga a partir de ese número. Nunca se avanza el tiempo sumando `delta`
(deriva respecto a la tarjeta de sonido).

## Tres retrasos a corregir

| Retraso | Origen | Valor típico | Corrección |
|---|---|---|---|
| Audio | Búfer del sistema (WASAPI / PipeWire) + altavoces | 20–200 ms | `get_output_latency()` + `audio_offset` |
| Pantalla | Monitor / tele | 5–100 ms | Dibujar adelantado `visual_offset` |
| Entrada | Bluetooth Joy-Con + procesado | 15–40 ms | Restar `input_offset` |

## Implementación

Código: [`src/godot/SongClock.gd`](../src/godot/SongClock.gd) (autoload).

- `audible()` → tiempo que oye el jugador.
- `visual()` → tiempo con el que se dibuja.
- `input_to_song(t_mono)` → convierte la marca de tiempo de una muestra de Joy-Con a tiempo de canción.

Se suaviza porque `get_playback_position()` avanza a saltos (uno por bloque de audio mezclado).

## Uso en la imagen

```gdscript
# Icono de un paso que baja hacia la línea de acierto
position.y = target_y - (step.time - SongClock.visual()) * speed

# Bailarín de referencia animado, fijado a la canción
$AnimationPlayer.seek(SongClock.visual(), true)
```

Se prefiere **animación con `seek`** a vídeo: `VideoStreamPlayer` lleva su propio reloj y deriva.

## Joy-Con en el mismo reloj

En la extensión C++, SDL3 da `sensor_timestamp` (ns, reloj de SDL):

1. Antigüedad = `SDL_GetTicksNS() - sensor_timestamp`.
2. Tiempo de la muestra en Godot = `Time.get_ticks_usec() - antigüedad`.
3. Pasar por `SongClock.input_to_song()`.

## Calibración (obligatoria, guardada por equipo)

1. **Audio + entrada:** clics a ritmo; el jugador sacude el Joy-Con en cada clic → `input_offset`.
2. **Pantalla:** destello + clic; deslizador hasta que coinciden → `visual_offset`.
