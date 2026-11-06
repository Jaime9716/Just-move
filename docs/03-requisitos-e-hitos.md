# 03 · Requisitos mínimos e hitos

> Copia local del documento vivo: https://claude.ai/artifact/NnR4oVqUA3CGrU3jsbaLLc
> Fecha: 2026-11-06

## Alcance

Primera versión jugable en **Windows x86-64 con Joy-Con** por Bluetooth: una canción, puntuación por
movimiento y música sincronizada. Linux es la segunda plataforma; webcam, móvil, ESP32 y Android después.
El motor usa adaptadores de fuente intercambiables desde el primer día.

## Requisitos funcionales

| ID | Requisito | Criterio de aceptación |
|---|---|---|
| F1 | Conectar Joy-Con izquierdo y derecho por Bluetooth | Detecta los dos y los asigna a mano izquierda y derecha |
| F2 | Leer acelerómetro y giroscopio de cada Joy-Con | Muestras a ~200 Hz con marca de tiempo de SDL3 |
| F3 | Asignar cada Joy-Con a una extremidad | El jugador elige mano o pie en un menú |
| F4 | Adaptador de fuente con interfaz común | `start()`, `stop()`, `poll()`; el juego no conoce el sensor |
| F5 | Grabar y reproducir sesiones de sensores | Una sesión grabada da la misma puntuación al reproducirla |
| F6 | Reloj de canción basado en el audio | Imagen y juicio usan tiempo de canción, nunca `delta` acumulado |
| F7 | Normalizar y filtrar las señales IMU | Orientación con Madgwick o filtro complementario |
| F8 | Extraer rasgos por extremidad | Energía, picos de aceleración, dirección y orientación |
| F9 | Formato de coreografía en JSON | Canción, BPM, desfase, pasos con tiempo, extremidad y referencia IMU |
| F10 | Juez por paso | Similitud en la ventana del beat → Perfect / Good / Miss |
| F11 | Grabador de coreografías | Bailar la canción con Joy-Con genera el archivo de referencia |
| F12 | Calibración de audio + entrada | Sacudir al ritmo de clics fija `input_offset` |
| F13 | Calibración de pantalla | Deslizador hasta que destello y clic coinciden fija `visual_offset` |
| F14 | Pantalla de juego mínima | Iconos colocados por `SongClock.visual()`, puntuación y resultado |
| F15 | Guardar calibración por equipo | Los offsets persisten entre sesiones |
| F16 | Multijugador local (prioridad más baja, H7) | Varios jugadores en la misma partida, cada uno con sus Joy-Con y su puntuación |

## Requisitos no funcionales (objetivos iniciales)

| ID | Requisito | Objetivo |
|---|---|---|
| N1 | Plataforma principal | Windows 10/11 x86-64 |
| N2 | Plataforma secundaria | Linux x86-64 (driver `hid-nintendo`) |
| N3 | Error de sincronía audio–imagen tras calibrar | ≤ 10 ms, sin deriva |
| N4 | Error temporal de una muestra IMU | ≤ 10 ms |
| N5 | Fotogramas por segundo | 60 fps estables en portátil de gama media |
| N6 | Ventana de acierto con IMU | Perfect ± 50 ms, Good ± 100 ms (ajustable) |
| N7 | Tiempo monotónico | El tiempo de canción no retrocede salvo en un seek |
| N8 | Motor independiente del juego | La librería C++ compila y pasa tests sin Godot |
| N9 | Pruebas sin bailar | Tests automáticos con sesiones grabadas |
| N10 | Licencias | Dependencias libres aptas para uso comercial (Godot MIT, SDL3 zlib) |

## Decisiones técnicas

| Decisión | Elección | Por qué |
|---|---|---|
| Juego, UI y audio | Godot 4 | MIT, multiplataforma, GDExtension en C++ |
| Motor de movimiento | Librería C++ + GDExtension | Se prueba sola, no ata a Godot |
| Lectura de Joy-Con | SDL3 | Una API para Windows y Linux |
| Reloj maestro | Posición del audio en Godot | El audio no deriva; `delta` sí |
| Tiempo de cada muestra | `sensor_timestamp` de SDL3 traducido por antigüedad | Ubica cada muestra aunque se procese tarde |
| Bailarín | `AnimationPlayer.seek()`, no vídeo | Fotograma exacto |
| Puntuación | Referencia IMU grabada (correlación/DTW + picos) | La IMU no da posición fiable |
| Prototipo | Python (hidapi o pysdl3 + numpy) | Iterar rápido lo más incierto |

Por confirmar: soporte de Joy-Con de Switch 2 en la versión de SDL3 usada.

## Hitos

Un hito se cierra solo cuando cumple su puerta. H0–H4 = MVP en Windows.

### Prioridad 1 · Windows x86-64 + Joy-Con
- [ ] **H0 · Prototipo en Python** — leer Joy-Con, grabar sesiones, probar el juez.
      *Puerta:* la puntuación se siente justa con sesiones grabadas reales.
- [ ] **H1 · Lectura de Joy-Con en C++ con SDL3** (F1–F5).
      *Puerta:* 2 Joy-Con a ~200 Hz y reproducción idéntica.
- [ ] **H2 · Reloj de canción en Godot** (F6, F12, F13, F15).
      *Puerta:* sincronía ≤ 10 ms sin deriva.
- [ ] **H3 · Motor de puntuación y coreografía** (F7–F11).
      *Puerta:* las sesiones grabadas dan la puntuación esperada.
- [ ] **H4 · Primera canción jugable — MVP** (F14).
      *Puerta:* partida completa con calibración, en Windows.

### Prioridad 2 · Linux x86-64
- [ ] **H5 · Port a Linux** (N2, hid-nintendo + PipeWire).
      *Puerta:* la misma sesión grabada da la misma puntuación.

### Prioridad 3 · Resto
- [ ] **H6 · Futuro** — webcam, móvil, ESP32 y Android como nuevos adaptadores.

### Prioridad 4 · Multijugador
- [ ] **H7 · Multijugador local** (F16) — varios jugadores con Joy-Con, puntuación por jugador.

Plan de trabajo por agentes: ver [04-plan-de-trabajo.md](04-plan-de-trabajo.md).

## Fuera de alcance por ahora

Webcam y pose, móvil como sensor, ESP32, Android, ARM y macOS. El multijugador está planificado como último hito (F16, H7).

## Riesgos

| Riesgo | Impacto | Mitigación |
|---|---|---|
| Emparejado de Joy-Con inestable con algunos adaptadores Bluetooth | No se puede jugar | Probarlo en H1; lista de adaptadores que funcionan |
| Godot y SDL3 compiten por los mandos | Lecturas duplicadas o perdidas | Inicializar en SDL3 solo sensores; comprobar en H1 |
| Latencia de audio variable en Windows | Desfase percibido | Calibración obligatoria (F12, F13) |
| Puntuación injusta | No divierte | Prototipo Python + sesiones grabadas |
| Correas sueltas, sobre todo en pie | Señal ruidosa | Ventanas más amplias para pies; correa firme |
