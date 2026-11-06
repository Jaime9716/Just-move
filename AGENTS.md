# AGENTS.md — reglas para los agentes que programan este repositorio

Juego de baile con Joy-Con. Prioridad: **Windows x86-64 + Joy-Con**; Linux después.
Lee antes de empezar: `docs/` (arquitectura por bloques, sincronización, requisitos, planes de prueba)
y **todo** `contracts/`.

## Reglas comunes

1. **Solo escribes en tus carpetas** (tabla abajo). Puedes LEER todo el repo.
2. `contracts/` y `engine/common/` son de solo lectura. Si un contrato no te basta, NO lo cambies:
   escribe la propuesta en `docs/agents/<tu-letra>-report.md` (sección "Propuestas de contrato")
   y trabaja con una solución local mientras tanto.
3. **No hagas `git commit`** ni toques `.git` (otros agentes trabajan a la vez). El arquitecto integra.
4. Compila en tu propio directorio de build para no pisar a otros:
   `cmake -S . -B build/<tu-letra> -G Ninja && cmake --build build/<tu-letra> && ctest --test-dir build/<tu-letra> --output-on-failure`
5. C++17, sin dependencias externas en el núcleo (solo STL + `engine/common`). Namespace `jd`.
   Biblioteca estática por área: `jd_<area>` definida en `engine/<area>/CMakeLists.txt`
   (el CMake raíz la incluye sola si existe). Tests de desarrollador en `engine/<area>/tests/`,
   con `engine/common/mini_test.h`, registrados con `add_test`.
6. Python 3.11+, solo stdlib + numpy (pytest disponible para tests).
7. Código y comentarios: identificadores en inglés, comentarios y mensajes al usuario en español.
8. Al terminar escribe `docs/agents/<tu-letra>-report.md`: qué hiciste, cómo probarlo, qué queda,
   qué no pudiste verificar y por qué, propuestas de contrato.

## Limitaciones del entorno de desarrollo (nube, Linux)

- **Sin red**: no hay GitHub ni PyPI. No hay SDL3, godot-cpp, GoogleTest, GUT, Godot ni yt-dlp instalados.
  Sí hay: g++ 13, clang, cmake 3.28, ninja, python3 + numpy + pytest, ffmpeg.
- El código que dependa de SDL3 / godot-cpp / Godot / yt-dlp se escribe con cuidado pero **no se puede
  compilar ni ejecutar aquí**: aíslalo detrás de opciones (`JD_WITH_SDL3`, `JD_WITH_GODOT`) o de
  interfaces con implementaciones falsas, y prueba todo lo demás. El usuario compila en su PC Windows
  (Visual Studio + CMake) siguiendo `BUILD_WINDOWS.md` (agente G).
- Godot objetivo: **Godot 4.4** (GDScript tipado). SDL3: 3.2.x.
- No hay Joy-Con aquí: usa sesiones sintéticas (`data/sessions/synthetic/`, las genera el agente B con
  `proto/synth.py`) o genera tus propios datos de prueba dentro de tu carpeta.

## Propiedad de carpetas

| Agente | Bloques | Carpetas propias |
|---|---|---|
| A · Joy-Con SDL3 | B1, B7 | `engine/input/`, `engine/session/`, `engine/api/sources.cpp` |
| B · Prototipo Python | prototipo B3–B5 | `proto/`, `data/sessions/synthetic/` |
| C · Motor y juez C++ | B3, B4, B5 | `engine/signal/`, `engine/features/`, `engine/judge/`, `engine/api/engine.cpp` |
| D · Sincronía Godot | B2, B9 | `game/core/` |
| E · UI de juego | B8 | `game/ui/`, `game/scenes/`, `game/assets/` |
| F · Coreografías + YouTube | B6 | `engine/choreo/`, `tools/`, `data/songs/` |
| G · Build y CI | B10 | `CMakeLists.txt` (raíz), `cmake/`, `game/gdextension/`, `game/project.godot`, `scripts/`, `BUILD_WINDOWS.md` |
| T1 · Tests motor | — | `engine/tests/`, `proto/tests/` |
| T2 · Tests juego | — | `game/tests/` |
| T3 · Pruebas reales | — | `tests/system/`, `docs/feedback/` |

Todos: `docs/agents/<letra>-report.md`.

## Decisiones ya tomadas

- Reloj único: `jd_now_ns()` (contracts/clock.h). La música es el reloj maestro (docs/02).
- `song_time = t_ns * 1e-9 + mono_to_song_s`; SongClock calcula `mono_to_song_s` (incluye offsets).
- Canciones y vídeos: se importan de YouTube con yt-dlp (herramienta de F, **solo uso personal**),
  a un paquete de canción (contracts/song_package.md): `audio.ogg` + `video.ogv` (Theora sin audio).
- El vídeo del bailarín es **esclavo del SongClock**: se reproduce mudo y se corrige la deriva (agente E).
- Puntuación IMU: comparación con referencia grabada + pico en el beat (docs/01, docs/03).
