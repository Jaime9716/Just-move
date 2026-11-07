# Juego de Baile

Juego de baile para PC (Windows x86-64 primero, Linux después) en el que **el mando es un móvil
Android**: el móvil, en la mano o atado al pie, envía su acelerómetro y giroscopio por Wi-Fi y el
motor puntúa el movimiento sincronizado con la música. Canciones y vídeo del bailarín se importan de
YouTube con yt-dlp (**solo uso personal**).

| Pieza | Tecnología | Carpeta |
|---|---|---|
| Motor (señal, rasgos, juez, servidor del móvil, sesiones) | Rust, API en C | `engine/` |
| Integración con Godot (GDExtension) | Rust + godot-rust (gdext) | `engine/jd_godot/` |
| Juego (reloj de canción, calibración, UI, vídeo) | Godot 4.4 / GDScript | `game/` |
| App de mando | Android, Kotlin sin AndroidX | `android/` |
| Prototipo del juez | Python + numpy | `proto/` |
| Coreografías, importador de YouTube | Python + ffmpeg + yt-dlp | `tools/` |
| Pruebas en el mundo real | guías + scripts | `tests/system/` |

## Empezar

- Compilar y jugar: **[BUILD.md](BUILD.md)**.
- Grabar las primeras sesiones con el móvil (Fase 0): `tests/system/guides/fase0_grabar_sesiones.md`.
- Reglas para agentes y propiedad de carpetas: [AGENTS.md](AGENTS.md). Contratos entre bloques: `contracts/`.
- Diseño: `docs/` (los documentos 01–07 se escribieron con Joy-Con en mente; donde difieran, mandan
  `contracts/` y `AGENTS.md`). Informes de cada agente: `docs/agents/`. Tu feedback: `docs/feedback/`.

## Estado

- **Hecho y probado en la nube:** motor y juez v1.1 (156 tests Rust, coincidencia exacta con el prototipo
  Python), servidor del móvil con sincronización de reloj, sesiones, coreografías, importador de YouTube,
  lógica de la app Android (40 tests JVM, compatible byte a byte con el servidor).
- **Escrito pero sin compilar/ejecutar** (no hay red ni Godot/Android SDK en el entorno de desarrollo):
  GDExtension, escenas de Godot y tests de Godot, build de la APK. Los compilará la CI de GitHub
  (`.github/workflows/ci.yml`) en cuanto el repo esté en GitHub.
- **Siguiente:** subir a GitHub → CI verde → Fase 0 (grabar sesiones reales) → H0 con tu feedback.
