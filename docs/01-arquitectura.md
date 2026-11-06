# 01 · Arquitectura del motor de movimiento

> Visión general del sistema. Alcance actual: **Joy-Con en Windows x86-64**, después Linux.
> Webcam, móvil, ESP32 y Android quedan para el futuro, pero la arquitectura ya los contempla.

## Principio central

El juego nunca habla directamente con un sensor. Habla con un *motor de movimiento* que convierte
cualquier entrada a un formato común.

```
 [Joy-Con]   [Webcam]*   [Móvil atado]*   [ESP32 + IMU]*      (* = futuro)
     \           |             |              /
      ───── Adaptadores de fuente ─────
                     │
      Normalización (reloj común, filtros, ejes)
                     │
      Extracción de rasgos (velocidad, energía, ángulos…)
                     │
      Juez  ◄── archivo de coreografía
                     │
      Juego (música, UI, puntuación)
```

**Por qué:** cada sensor mide cosas distintas. Si el juego dependiera de uno, añadir otro obligaría a
reescribirlo. Con esta separación, una entrada nueva es solo un adaptador nuevo.

## Capas

1. **Adaptadores de fuente** — interfaz común `start()`, `stop()`, `poll() → muestras con marca de tiempo`.
   Incluye un adaptador **falso que reproduce grabaciones** para probar sin bailar.
2. **Normalización** — reloj único (el de la música), filtros (Madgwick / complementario para IMU,
   One Euro para cámara en el futuro), ejes unificados, asignación a extremidad.
3. **Extracción de rasgos** — energía, dirección, picos de aceleración, orientación
   (y posición/ángulos solo con cámara).
4. **Juez** — por cada paso, ventana alrededor del beat → similitud → Perfect / Good / Miss.
5. **Juego** — música, gráficos y menús; no sabe qué sensor se usa.

## Tecnología

| Pieza | Elección | Por qué |
|---|---|---|
| Juego / UI / audio | Godot 4 | Gratis (MIT), exporta a Windows/Linux/Android, extensible en C++ (GDExtension) |
| Motor de movimiento | Librería C++ independiente | Se prueba sola, no te ata a Godot |
| Mandos y sensores | SDL3 | Una API para Joy-Con (giroscopio + acelerómetro) en Windows y Linux; más tarde cámara y Android |
| Pose (futuro) | ONNX Runtime + BlazePose/RTMPose/MoveNet | Un motor de inferencia en todas las plataformas |
| Sensores externos (futuro) | UDP / WebSocket con protocolo propio | Cualquier aparato puede ser un sensor |

## Entradas

| Fuente | Da | No da | Sirve para |
|---|---|---|---|
| Joy-Con | IMU 6 ejes, timing preciso | Posición | Golpes, giros, ritmo |
| Webcam* | Posición de todo el cuerpo | Timing fino (60–150 ms, 30 fps) | Poses |
| Móvil atado* | IMU por UDP | Posición | Igual que Joy-Con |
| ESP32 + IMU* | IMU barata (5–10 €) | Posición | 4 extremidades a la vez |

## Puntuar con sensores distintos

Una IMU no reconstruye posición de forma fiable; la cámara no mide bien el ritmo fino.
**Decisión:** la coreografía guarda **una referencia por tipo de sensor** y el juez compara con la de su mismo tipo:

- IMU: correlación o DTW de aceleración/giro en la ventana + detección de picos en el beat.
- Cámara: similitud de ángulos de articulaciones normalizados.
- Rasgos comunes (energía, momento del pico, dirección) como respaldo entre tipos.

## Formato de coreografía

JSON con canción, BPM, desfase, lista de pasos (inicio, duración, extremidades, icono) y referencias por
tipo de sensor. Permite un **grabador**: bailas la canción y se genera la coreografía.

## Riesgos generales

- Rendimiento de la pose en móviles baratos (futuro).
- Emparejado de Joy-Con según el adaptador Bluetooth.
- Iluminación y encuadre de cámara (futuro).
