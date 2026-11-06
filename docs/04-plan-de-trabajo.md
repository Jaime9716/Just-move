# 04 · Plan de trabajo en paralelo por agentes

> Copia local de la sección del documento vivo: https://claude.ai/artifact/NnR4oVqUA3CGrU3jsbaLLc

Los agentes pueden trabajar en paralelo porque primero se fijan los **contratos** (interfaces y formatos
de archivo) y unas sesiones grabadas reales. Desde ahí cada agente trabaja contra esos contratos y
simulaciones, sin esperar a los demás.

```
Fase 0            Fase 1 · paralelo                              Fase 2 · integración
                                                                  I1    I2    I3
┌──────────┐  A · Joy-Con SDL3     Adaptador, timestamps (F1–F5) ──●
│Contratos │  D · Sincronía Godot  SongClock, calibraciones      ──●
│    +     │  B · Prototipo Python Juez + resultados golden      ────────●
│ sesiones │  C · Motor y juez C++ Filtros, rasgos, juez         ────────●
│ grabadas │  F · Coreografías     Formato JSON y grabador       ────────●
│          │  E · UI de juego      Menús y pantalla (F3, F14)    ──────────────●
│          │  G · Build y CI       CMake, GDExtension, CI, tests ──────────────●
│          │  T1 · Tests motor     Bajo nivel (doc 06)           ──●─────●─────●
│          │  T2 · Tests juego     Bajo nivel GUT, headless      ──●─────●─────●
└──────────┘  T3 · Pruebas reales  Alto nivel P1–P11 (doc 07)    ──●─────●─────●
```

- **I1** · Muestras de Joy-Con en tiempo de canción (cierra H1 y H2)
- **I2** · El juez C++ puntúa igual que el golden de Python con coreografías reales (cierra H0 y H3)
- **I3** · Primera canción jugable en Windows: MVP (cierra H4)

## Fase 0 · Contratos (un agente arquitecto + tú)

1. `contracts/motion_sample.h`: estructura de muestra (`t_ns`, extremidad, acelerómetro xyz, giroscopio xyz, id de fuente).
2. `contracts/source.h`: interfaz de adaptador (`start`, `stop`, `poll`).
3. `contracts/engine_api.h`: API C del motor hacia Godot (cargar coreografía, enviar muestras, pedir resultados por paso).
4. `contracts/session.schema.json`: formato de sesión grabada (cabecera + muestras en JSONL).
5. `contracts/choreo.schema.json`: formato de coreografía (F9).
6. `data/sessions/`: **tú** grabas 3–5 sesiones reales con los Joy-Con y un script mínimo.
   Los agentes no tienen los mandos; estas grabaciones son su "hardware".

## Agentes de la fase 1

| Agente | Responsabilidad | Carpeta propia | Entrega | Necesita de otros |
|---|---|---|---|---|
| A · Joy-Con SDL3 | Adaptador Joy-Con, marcas de tiempo, grabar y reproducir (F1–F5) | `engine/input/` | Librería + herramienta de grabación | Solo contratos |
| B · Prototipo Python | Algoritmo del juez sobre sesiones grabadas (H0) | `proto/` | Especificación del algoritmo + resultados golden | Contratos y sesiones |
| C · Motor y juez C++ | Filtros, rasgos y juez (F7, F8, F10) | `engine/signal/`, `engine/judge/` | Librería con tests | Filtros y rasgos: nada; juez: especificación de B |
| D · Sincronía Godot | SongClock, calibraciones y guardado (F6, F12, F13, F15) | `game/core/` | Autoload + escenas de calibración | Nada (muestras simuladas) |
| E · UI de juego | Menús, asignación de extremidades, pantalla de juego (F3, F14) | `game/ui/` | Escenas Godot | Motor simulado con resultados falsos |
| F · Coreografías | Validador del formato JSON y grabador (F9, F11) | `tools/` | Herramientas + coreografía de ejemplo | Contratos; para grabar de verdad, el adaptador de A |
| G · Build y CI | Bloque B10: CMake, enlace GDExtension, CI en Windows (N8, N9) | `cmake/`, `ci/`, `game/bin/` | Build reproducible + CI en verde | Esqueletos de A y C |
| T1 · Tests motor | Pruebas de bajo nivel de B1, B3–B7 (doc 06) | `engine/tests/`, `proto/tests/` | Suites GoogleTest y pytest en CI | Contratos, sesiones, golden de B |
| T2 · Tests juego | Pruebas de bajo nivel de B2, B8, B9 y juego headless (doc 06) | `game/tests/` | Suites GUT en CI | Contratos; simulaciones de D y E |
| T3 · Pruebas reales | Plan de alto nivel P1–P11 (doc 07), guías y análisis de tu feedback | `tests/system/`, `docs/feedback/` | Guía por prueba + informe | Builds de cada integración y tu ejecución |

## Reglas para trabajar en paralelo

- Cada agente solo escribe en su carpeta; `contracts/` solo lo cambia el arquitecto, con aviso a todos.
- Cada agente entrega con una **simulación** de lo que produce, para que nadie quede bloqueado.
- Una rama de git por agente y pull request a `main`; G revisa que el CI pase antes de fusionar.
- "Hecho" = tests en verde con las sesiones grabadas de `data/sessions/`.
- Las pruebas con los Joy-Con reales las haces tú en cada integración (I1, I2, I3), y me das tu feedback antes de cerrar el hito (ver `docs/feedback/`).

## Después del MVP

- **H5 (Linux):** G con A.
- **H6 (nuevas entradas):** un agente nuevo por adaptador, sin tocar el juez.
- **H7 (multijugador, F16):** un agente para E y C: varias instancias del juez, una por jugador.

## Estructura de repositorio propuesta

```
contracts/      interfaces y esquemas (solo arquitecto)
engine/         input/  signal/  judge/   (C++)
proto/          prototipo Python
game/           proyecto Godot (core/, ui/, bin/)
tools/          grabador y validador de coreografías
data/           sessions/  choreo/
cmake/  ci/
docs/
```
