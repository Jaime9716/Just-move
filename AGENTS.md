# AGENTS.md — reglas para los agentes que programan este repositorio

Juego de baile. **El juego corre en PC (Windows x86-64 primero, Linux después) y el mando es un
móvil Android** que envía acelerómetro y giroscopio por Wi-Fi (como Just Dance Now). Sin Joy-Con.
Motor en **Rust** con API en C; juego en **Godot 4.4** (GDScript); app de mando en **Kotlin** (Android).

Lee antes de empezar: `docs/` (arquitectura por bloques, sincronización, requisitos, planes de prueba)
y **todo** `contracts/`. Donde los docs antiguos hablen de Joy-Con/SDL3/C++, mandan los contratos.

## Reglas comunes

1. **Solo escribes en tus carpetas/archivos** (tabla abajo). Puedes LEER todo el repo.
2. `contracts/`, `engine/jd_core/src/{types,json,clock}.rs`, `engine/jd_core/src/lib.rs`,
   `engine/jd_capi/src/lib.rs` y los `Cargo.toml` del workspace son del arquitecto (solo lectura).
   Si un contrato no te basta, NO lo cambies: escribe la propuesta en `docs/agents/<letra>-report.md`
   ("Propuestas de contrato") y usa una solución local.
3. **No hagas `git commit`** ni toques `.git`. El arquitecto integra.
4. Rust: edición 2021, **sin dependencias externas** en `jd_core`, `jd_capi`, `jd_tools` (no hay red).
   Compila y prueba con un target dir propio para no pisar a otros:
   `cd engine && CARGO_TARGET_DIR=../build/<letra> cargo test -p jd_core` (o el paquete que toque).
   Tests unitarios en tu módulo (`#[cfg(test)]`) y de integración en `engine/jd_core/tests/<letra>_*.rs`.
   Sin `unsafe` salvo en `jd_capi`. Sin warnings de `cargo build`.
5. Python 3.11+, solo stdlib + numpy (pytest disponible).
6. Identificadores en inglés; comentarios y mensajes al usuario en español.
7. Al terminar escribe `docs/agents/<letra>-report.md`: qué hiciste, cómo probarlo, qué queda,
   qué no pudiste verificar y por qué, propuestas de contrato.

## Limitaciones del entorno de desarrollo (nube, Linux)

- **Sin red**: no hay crates.io, GitHub, PyPI ni Maven. Sí hay: Rust 1.97 (cargo), python3 + numpy +
  pytest, ffmpeg, Java + Gradle (sin Android SDK), g++/clang.
- **No se puede compilar ni ejecutar aquí**: la GDExtension (`jd_godot`, necesita la crate `godot`),
  Godot, la app Android (necesita Android SDK) ni yt-dlp. Ese código se escribe con mucho cuidado y se
  compila en GitHub Actions (cuando el usuario dé acceso) o en el PC del usuario.
- No hay móvil real: usa el simulador de móvil (`jd_tools`, bin `phone_sim`, agente A) o datos
  sintéticos (`data/sessions/synthetic/`, agente B).

## Propiedad

| Agente | Bloques | Escribe en |
|---|---|---|
| A · Fuente móvil y sesiones | B1, B7 | `engine/jd_core/src/{session,replay,recorder}.rs`, `engine/jd_core/src/phone.rs` o `phone/`, `engine/jd_capi/src/sources.rs`, `engine/jd_tools/src/bin/{jd_record,phone_sim}.rs`, `engine/jd_core/tests/a_*.rs` |
| B · Prototipo Python | prototipo B3–B5 | `proto/` (menos `proto/tests/`), `data/sessions/synthetic/` |
| C · Motor y juez | B3, B4, B5 | `engine/jd_core/src/{signal,features,judge,engine}.rs`, `engine/jd_capi/src/engine.rs`, `engine/jd_tools/src/bin/jd_judge.rs`, `engine/jd_core/tests/c_*.rs` |
| D · Sincronía Godot | B2, B9 | `game/core/` |
| E · UI de juego | B8 | `game/ui/`, `game/scenes/`, `game/assets/` |
| F · Coreografías + YouTube | B6 | `engine/jd_core/src/choreo.rs`, `engine/jd_core/tests/f_*.rs`, `tools/`, `data/songs/` |
| G · Build, GDExtension, CI | B10 | `engine/jd_godot/`, `game/project.godot`, `game/jd_engine.gdextension`, `game/bin/`, `.github/`, `scripts/`, `BUILD.md` |
| P · App Android de mando | B1 (móvil) | `android/` |
| T1 · Tests motor | — | `engine/jd_core/tests/t1_*.rs`, `engine/jd_capi/tests/`, `proto/tests/` |
| T2 · Tests juego | — | `game/tests/` |
| T3 · Pruebas reales | — | `tests/system/`, `docs/feedback/` |

Todos: `docs/agents/<letra>-report.md`.

## Decisiones ya tomadas

- Reloj único: `jd_core::clock::now_ns()` = `jd_now_ns()`. La música es el reloj maestro (docs/02).
- `song_time = t_ns * 1e-9 + mono_to_song_s`; SongClock calcula `mono_to_song_s` (incluye offsets).
- Mando: móvil Android por UDP con sincronización de reloj tipo NTP (contracts/phone_protocol.md).
- Canciones y vídeos: importados de YouTube con yt-dlp (herramienta de F, **solo uso personal**) a un
  paquete de canción (contracts/song_package.md): `audio.ogg` + `video.ogv` (Theora sin audio).
- El vídeo del bailarín es **esclavo del SongClock**: mudo y con corrección de deriva (agente E).
- Puntuación: algoritmo del juez v1 (contracts/judge_algorithm_v1.md), igual en Python (B) y Rust (C).
