# Informe del agente G · Build, GDExtension y CI (B10)

## Qué hice

| Archivo | Contenido |
|---|---|
| `engine/jd_godot/Cargo.toml` | cdylib `jd_godot` fuera del workspace (workspace propio vacío), `godot = "~0.3.0"` con feature `api-4-4`, path-dep a `jd_core`. |
| `engine/jd_godot/src/lib.rs` | Clases `JDSource` y `JDEngine` (RefCounted, `init`) con **exactamente** los métodos de `contracts/gdextension_api.md`. Solo conversión de tipos Godot ↔ Rust. `res://`/`user://` → ruta del sistema con `ProjectSettings.globalize_path`. |
| `engine/jd_godot/src/bridge.rs` | Toda la lógica sin Godot: `SourceCore` (phone server / replay / recorder sobre `Arc<Mutex<dyn MotionSource>>`, backlog de `poll_samples(max)`), `EngineCore` (`pump` = drena la fuente, ordena por `t_ns` y llama a `Engine::push_samples`, sin pasar por GDScript), conversiones de enteros (limb, puerto 0→47800, código fuera de 1000..9999→aleatorio), config con valores por defecto si llegan ≤0/NaN. |
| `engine/jd_godot/bridge_test/` | Crate sin red que incluye `bridge.rs` con `#[path]`: **6 tests** (conversiones, replay+backlog, grabador que escribe cabecera+400 líneas, pump que puntúa `data/sessions/synthetic/perfect.jsonl` con ≥24/27 Perfect, servidor de móviles). |
| `game/jd_engine.gdextension` | `entry_symbol = "gdext_rust_init"`, `compatibility_minimum = "4.4"`, windows debug/release x86_64 → `res://bin/jd_godot.dll`, linux → `res://bin/libjd_godot.so`. |
| `game/project.godot` | Godot 4.4, "Juego de Baile", `res://scenes/main_menu.tscn`, autoload `SongClock` (D y E no piden más), 1280×720, stretch `canvas_items`/`expand`, renderer `gl_compatibility`. |
| `game/bin/.gitkeep` | |
| `.github/workflows/ci.yml` | Jobs: `engine` (ubuntu+windows: build `-D warnings`, test, bridge_test, clippy informativo), `jd_godot` (ubuntu+windows: release + artefactos `jd_godot-<os>` y `juego-jd_godot-<os>` con game/ listo + jd_record/phone_sim/jd_judge), `python` (numpy+pytest+ffmpeg; pytest por carpeta en proto/dev_tests, proto/tests, tools/tests, tests/system/tests si existen; código 5 = sin tests no falla), `godot` (Godot 4.4.1 headless con caché, copia la .so, `--import`, prueba de humo `scripts/ci/check_extension.gd`, `res://tests/run_tests.gd` si existe), `android` (si existe `android/gradlew`: Java 17, setup-android, setup-gradle, `assembleDebug testDebugUnitTest`, artefacto APK + informe de tests). Acciones con versión fija (`@v4`, `@v5`, `@v3`, `@v2`) y caché (rust-cache, pip, Godot, Gradle). |
| `scripts/build_linux.sh`, `scripts/build_windows.ps1` | Compilan motor + GDExtension, copian la biblioteca a `game/bin/` (respetan `CARGO_TARGET_DIR`) y pasan los tests (`--skip-tests` / `-SkipTests`). El `.ps1` va en UTF-8 con BOM y CRLF (Windows PowerShell 5.1 lee mal UTF-8 sin BOM). |
| `scripts/run_tests.sh` | cargo test workspace + bridge_test + pytest + Godot (si `GODOT` o `godot` en PATH y existe la .so). |
| `scripts/ci/check_extension.gd` | Prueba de humo de la extensión en Godot: clases registradas, now_ns, load_song con error, update vacío, create_replay null, phone server con código, replay+pump+poll_samples, grabador. Usa `ClassDB.class_call_static` (4.4) para los estáticos. |
| `BUILD.md` | Guía paso a paso (Rust+Build Tools MSVC, Python+numpy, Godot 4.4.1, ffmpeg, yt-dlp con aviso de uso personal, compilar, abrir, APK, Firewall UDP 47800 red privada, jd_record, importador, problemas frecuentes). |

## Cómo probarlo
```bash
cd engine && CARGO_TARGET_DIR=../build/G cargo test                      # workspace: todo OK
CARGO_TARGET_DIR=build/G cargo test --manifest-path engine/jd_godot/bridge_test/Cargo.toml   # 6 OK
CARGO_TARGET_DIR=build/G scripts/run_tests.sh                            # todo OK (Godot omitido)
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"  # YAML válido
```
Resultado aquí: workspace OK, bridge_test 6/6, pytest proto/dev_tests 64 y tools/tests 69, YAML válido,
clippy sin avisos en `bridge.rs` (los 10 avisos de clippy son de `jd_core`, no míos).

## No verificado (y por qué)
- **`src/lib.rs` no se ha compilado nunca**: no hay red para bajar `godot` 0.3. Escrito contra la API de
  gdext 0.3 (`#[derive(GodotClass)] #[class(base=RefCounted, init)]`, `#[godot_api]`, `#[func]` sin
  `self` = estático, `Gd::from_object`, `Option<Gd<T>>` como null, `Array<Dictionary>: FromIterator`,
  `Dictionary::set`, `ProjectSettings::singleton().globalize_path(&GString)`, `#[gdextension] unsafe impl
  ExtensionLibrary`). Primera compilación real: job `jd_godot` de la CI. Si falla, casi seguro será en
  esas firmas, no en la lógica (que está probada en bridge_test). No subir a gdext 0.4 sin revisar
  (renombra `Dictionary`/`Array` sin tipo, cambia `AsArg`).
- **Parámetros por defecto del contrato** (`port = 47800`, `pairing_code = 0`, `speed = 1.0`,
  `max = 512`) **no se declaran** en Godot: no tengo certeza de que gdext 0.3 soporte `#[opt(default)]`.
  GDScript debe pasar todos los argumentos (EngineFactory de D ya lo hace). Valores 0/no válidos se
  traducen al defecto.
- Godot headless, `--import`, la carga real de la .so y `check_extension.gd`: solo en CI.
- Workflow de GitHub Actions: solo validado como YAML y revisado a mano.
- La APK: depende del proyecto de P (`android/`); el job se salta si no hay `gradlew`.
- `build_windows.ps1`: revisado a mano, no ejecutado (no hay PowerShell).

## Qué queda
- Tras la primera CI verde, guardar en el repo `engine/jd_godot/Cargo.lock` (va en el artefacto como
  `jd_godot.Cargo.lock`) para fijar las dependencias transitivas.
- Si Godot cambia `project.godot` al abrirlo (añade `config/icon`, etc.), es normal; mantener las claves.

## Notas para otros agentes / arquitecto
- `.gitignore` (no es mío) ignora `*.import`; en Godot 4 conviene **versionar** los `.import`
  (guardan la configuración de importación). Propongo quitar esa línea y añadir `game/bin/*.dll`,
  `game/bin/*.so`, `game/bin/*.pdb` y `game/_ci_check_extension.gd`.
- `poll_samples` y `pump` sobre la misma fuente se pueden mezclar sin perder muestras (el backlog de
  `poll_samples` lo vacía `pump` primero), aunque el contrato recomienda no hacerlo.
- Con un grabador, ambos `JDSource` (interior y grabador) siguen siendo válidos; leer solo del grabador.
  `endpoint()`/`pairing_code()` del grabador devuelven los del servidor de móviles interior.

## Propuestas de contrato
- Documentar en `gdextension_api.md` que los argumentos con valor por defecto son obligatorios en
  la implementación nativa (o permitirlo cuando se confirme `#[opt(default)]` en gdext).
