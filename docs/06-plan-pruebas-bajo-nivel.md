# 06 · Plan de pruebas de bajo nivel (software)

Comprueba cada bloque por separado y sus uniones, **sin hardware**: los Joy-Con se sustituyen por
sesiones grabadas (B7) y señales sintéticas. Lo ejecutan los agentes **T1 (motor)** y **T2 (juego)**,
y corre en el CI de Windows en cada pull request.

**Herramientas:** GoogleTest (C++), pytest (Python), GUT (Godot headless).

| Bloque | Qué se prueba | Tipo | Criterio de aceptación |
|---|---|---|---|
| B1 · Adquisición | Conversión de eventos SDL simulados a muestras; reconexión | Unitaria con SDL simulado | Ejes y unidades correctos; `t_ns` siempre creciente |
| B2 · Tiempo | SongClock con posición de audio simulada a saltos, seek y pausas | Unitaria (GUT) | No retrocede salvo seek; error del suavizado ≤ 2 ms; `input_to_song` exacto |
| B3 · Señal | Filtros con señales sintéticas (reposo, seno, escalón, giro conocido) | Unitaria | Reposo estable; giro de 90° con error ≤ 5° |
| B4 · Rasgos | Picos y energía con golpes en instantes conocidos | Unitaria | Pico a ± 1 muestra de su instante |
| B5 · Juez | Sesión igual a la referencia, desplazada 40 y 150 ms, ruido, quieto | Golden + unitaria | Igual → Perfect; 40 ms → Good/Perfect; 150 ms y quieto → Miss; igual que el golden Python |
| B5 · Juez | Misma entrada dos veces | Determinismo | Resultado idéntico bit a bit |
| B6 · Coreografía | Archivos válidos, incompletos y corruptos | Unitaria + esquema | Válidos cargan; inválidos se rechazan con mensaje claro |
| B7 · Sesiones | Grabar y reproducir | Ida y vuelta | Reproducción idéntica |
| B8 · Presentación | Posición de iconos dado un tiempo visual | Unitaria (GUT) | Posición exacta según la fórmula |
| B9 · Configuración | Guardar/cargar perfil; ausente o corrupto | Unitaria | Ida y vuelta exacta; valores por defecto si falla |
| B10 · Plataforma | Build limpio y carga de la GDExtension | CI | Compila en Windows desde cero; Godot headless carga la extensión |

## Pruebas entre bloques

- **Motor de punta a punta:** sesión grabada + coreografía → API del motor → resultados esperados (B7→B3→B4→B5 con B6).
- **Juego headless:** Godot sin ventana reproduce una canción con B7 como fuente y termina con la puntuación esperada.
- **Rendimiento:** 4 fuentes a 200 Hz cuestan < 1 ms por fotograma.
- **Contratos:** cada simulación de un agente pasa los mismos tests que la implementación real.

## Regla de regresión

Todo fallo encontrado en las pruebas reales (doc 07) se convierte en un test de bajo nivel con la sesión grabada que lo produjo.
