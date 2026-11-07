# Cómo compilar y jugar al Juego de Baile (paso a paso)

Guía para alguien que nunca ha compilado nada. El juego corre en **PC con Windows 10/11 de 64 bits**
(Linux también funciona) y el mando es un **móvil Android** conectado a la misma Wi-Fi.

Piezas:

| Pieza | Qué es | Dónde |
|---|---|---|
| Motor | Rust: lee el móvil, filtra la señal y puntúa | `engine/` |
| GDExtension | La biblioteca `jd_godot.dll` que conecta el motor con Godot | `engine/jd_godot/` → `game/bin/` |
| Juego | Proyecto Godot 4.4 (GDScript) | `game/` |
| App de mando | APK Android | `android/` (la compila GitHub Actions) |
| Herramientas | Python: prototipo, importador de canciones | `proto/`, `tools/` |

> **Atajo sin compilar nada**: si el repo está en GitHub, abre la pestaña **Actions**, entra en la
> última ejecución verde de "CI" y descarga los artefactos: `juego-jd_godot-windows-x86_64` (juego con
> la DLL ya puesta + herramientas `jd_record.exe`, `phone_sim.exe`, `jd_judge.exe`) y
> `mando-android-debug-apk` (la app del móvil). Entonces salta al paso 4.

---

## 1. Instalar las herramientas (una sola vez)

### 1.1 Rust (Windows)
1. Instala las **Build Tools de Visual Studio 2022** (gratis):
   https://visualstudio.microsoft.com/es/visual-cpp-build-tools/ → en el instalador marca
   **"Desarrollo para el escritorio con C++"** (incluye el compilador MSVC y el SDK de Windows). Son
   ~6 GB. Sin esto, Rust falla con `link.exe not found`.
2. Descarga y ejecuta **rustup-init.exe** de https://rustup.rs y acepta las opciones por defecto
   (toolchain `stable-x86_64-pc-windows-msvc`).
3. **Cierra y vuelve a abrir** la terminal (PowerShell) y comprueba:
   ```powershell
   cargo --version
   rustc --version
   ```

En Linux: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` y `sudo apt install build-essential`.

### 1.2 Python + numpy (para herramientas y tests; opcional para jugar)
1. Instala Python 3.11 o 3.12 de https://www.python.org/downloads/ y **marca "Add python.exe to PATH"**.
2. En una terminal nueva:
   ```powershell
   python -m pip install numpy pytest
   ```

### 1.3 Godot 4.4
1. Descarga **Godot 4.4.1 (estándar, NO la versión .NET)** de https://godotengine.org/download/archive/
   (`Godot_v4.4.1-stable_win64.exe.zip`).
2. Descomprime en, por ejemplo, `C:\Godot\`. No se instala: se ejecuta el `.exe`.
   El `..._console.exe` es el mismo programa con consola; úsalo para ver errores y para los tests:
   `$env:GODOT = "C:\Godot\Godot_v4.4.1-stable_win64_console.exe"`.

### 1.4 ffmpeg (solo para importar canciones)
- Windows: `winget install Gyan.FFmpeg` (o descarga de https://www.gyan.dev/ffmpeg/builds/ y añade la
  carpeta `bin` al PATH). Comprueba con `ffmpeg -version` en una terminal nueva.
- Linux: `sudo apt install ffmpeg`.

### 1.5 yt-dlp (solo para importar canciones de YouTube)
```powershell
python -m pip install yt-dlp
```
> **Aviso legal**: el importador es **solo para uso personal** con vídeos que tengas derecho a usar.
> No redistribuyas las canciones ni los vídeos descargados (el paquete de canción incluye un
> `.gitignore` para que no acaben en el repositorio). Respeta los términos de YouTube y los derechos de autor.

---

## 2. Compilar

Abre PowerShell **en la carpeta del repositorio** (en el Explorador: clic derecho → "Abrir en Terminal") y ejecuta:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build_windows.ps1
```

Hace:
1. `cargo build --release` del motor (`engine/`) → herramientas en `engine\target\release\`
   (`jd_record.exe`, `phone_sim.exe`, `jd_judge.exe`).
2. `cargo build --release` de la GDExtension (`engine/jd_godot`; la primera vez descarga la crate
   `godot` de crates.io, hace falta internet; tarda unos minutos).
3. Copia `jd_godot.dll` a `game\bin\`.
4. Pasa los tests (Rust, Python si está, Godot si `$env:GODOT` está definido). Para saltarlos: `-SkipTests`.

En Linux: `scripts/build_linux.sh` (copia `libjd_godot.so` a `game/bin/`) y `scripts/run_tests.sh` para
los tests.

---

## 3. Abrir el juego en Godot

1. Abre Godot → **Importar** → elige `game/project.godot` → "Importar y editar".
2. La primera vez Godot importa los recursos (unos segundos) y carga `bin/jd_godot.dll`.
3. Pulsa **F5** (o el botón ▶) para jugar. La escena principal es el menú.
4. En el menú, la línea de estado debe decir que el motor es **real**. Si dice "motor SIMULADO", la
   DLL no se cargó (ver "Problemas frecuentes").

> Si recompilas la DLL, **cierra Godot antes**: Windows bloquea la DLL mientras está cargada.

---

## 4. Instalar la app de mando en el móvil

1. Descarga el artefacto `mando-android-debug-apk` de GitHub Actions (es un `.zip` con un `.apk`).
2. Pasa el `.apk` al móvil (cable USB, Drive, correo...) y ábrelo.
3. Android pedirá permitir **"Instalar aplicaciones desconocidas"** para la app con la que lo abriste
   (Archivos, Chrome...). Acéptalo. Es una versión de depuración, no viene de Google Play.
4. Con el depurador USB activado también vale: `adb install -r app-debug.apk`.

---

## 5. Red: misma Wi-Fi y Firewall de Windows

El móvil envía los datos por **UDP al puerto 47800** del PC.

1. PC y móvil en **la misma red Wi-Fi** (no la red de invitados: suele aislar dispositivos). Ni datos
   móviles ni VPN en el móvil.
2. La red del PC debe ser **privada**: Configuración → Red e Internet → Wi-Fi (o Ethernet) → tu red →
   Tipo de perfil de red: **Privada**.
3. Abre el puerto en el Firewall. La primera vez que el juego (o `jd_record.exe`) abre el servidor,
   Windows pregunta: marca **Redes privadas** y pulsa "Permitir". Si no salió el aviso o lo
   rechazaste, en PowerShell **como administrador**:
   ```powershell
   New-NetFirewallRule -DisplayName "Juego de Baile UDP 47800" -Direction Inbound -Protocol UDP -LocalPort 47800 -Action Allow -Profile Private
   ```
4. En el juego, "Conectar móvil" muestra **IP:puerto** y un **código de 4 dígitos**: escríbelos en la
   app del móvil. RTT de sincronización < 30 ms es bueno; más (en naranja) indica una Wi-Fi saturada.

---

## 6. Grabar tu primera sesión (jd_record)

Una sesión es un archivo JSONL con lo que mide el móvil (contracts/session_format.md). Sirve para
probar el juez y para reproducir partidas sin móvil.

```powershell
cd engine\target\release
.\jd_record.exe ..\..\..\data\sessions\mi_primera.jsonl --seconds 30 --note "prueba mano derecha"
```
1. Muestra IP:puerto y código: conéctate desde la app.
2. Mueve el móvil (sujeto en la mano o atado a la muñeca/tobillo). Graba 30 s o hasta Enter/Ctrl+C.
3. Opciones: `--port P`, `--code C`, `--song ID` (id de la canción), `--seconds N`.

Sin móvil puedes probar con el simulador en otra terminal: `.\phone_sim.exe <IP:puerto> --code <código> --seconds 30`.

---

## 7. Importar una canción de YouTube (uso personal)

```powershell
python tools\youtube_import\yt_import.py "https://www.youtube.com/watch?v=XXXX" --id mi_cancion --personal-use
```
Crea el paquete de canción (`audio.ogg`, `video.ogv` Theora sin audio, `choreo.json` con pasos
automáticos y `meta.json`). Opciones: `--start/--end` para recortar, `--no-video`. Después edita la
coreografía (o grábala con `tools/choreo_record.py`) y valida con `python tools\choreo_validate.py <carpeta>`.
El juego busca canciones en `user://songs/` y en `data/songs/`. Ver `tools/youtube_import/README.md`.

---

## 8. Ejecutar todos los tests

- Windows: `scripts\build_windows.ps1` (los pasa al final).
- Linux: `scripts/run_tests.sh` (define `GODOT=/ruta/a/godot` para incluir los de Godot).
- GitHub Actions (`.github/workflows/ci.yml`) lo ejecuta todo en cada push: motor en Windows y
  Ubuntu, GDExtension, Python, Godot headless y la APK.

---

## 9. Problemas frecuentes

| Síntoma | Causa y solución |
|---|---|
| `link.exe not found` / `linker 'link.exe' not found` | Faltan las Build Tools de Visual Studio con "Desarrollo para el escritorio con C++" (paso 1.1). Reinicia la terminal tras instalarlas. |
| `cargo` no se reconoce | Abre una terminal nueva tras instalar rustup. |
| `failed to download godot` / errores de red en cargo | La GDExtension necesita internet la primera vez (crates.io). Proxy de empresa: configura `HTTPS_PROXY`. |
| El menú dice "motor SIMULADO" | Falta `game/bin/jd_godot.dll`, o es de otra versión de Godot. Recompila, cierra y abre Godot. Mira la pestaña "Salida": `Can't open dynamic library` = falta la DLL o falta el runtime de VC++ (instala "Microsoft Visual C++ Redistributable x64"). |
| `No se pudo copiar jd_godot.dll` | Godot la tiene abierta: ciérralo y repite el script. |
| No se puede ejecutar el script `.ps1` | Usa `powershell -ExecutionPolicy Bypass -File ...` como arriba. |
| El móvil no encuentra el PC / no conecta | Misma Wi-Fi (no invitados), perfil de red **Privada**, regla de Firewall UDP 47800, sin VPN. Prueba con la IP que muestra el juego. Algunos routers tienen "aislamiento de AP": desactívalo. |
| `no se puede abrir el puerto UDP 47800` | Otro programa lo usa (¿otro juego abierto o `jd_record` corriendo?). Ciérralo. |
| RTT alto (> 30 ms) o tirones | Acerca el router, usa la banda de 5 GHz, cierra descargas. |
| El vídeo no se ve o no va sincronizado | El vídeo debe ser Theora `.ogv` sin audio (lo genera el importador). Con ffmpeg: `ffmpeg -i in.mp4 -an -c:v libtheora -q:v 7 -r 30 video.ogv`. |
| `yt-dlp` falla | Actualízalo: `python -m pip install -U yt-dlp` (YouTube cambia a menudo). |
| La APK no se instala | Permite "Instalar aplicaciones desconocidas"; si había otra versión firmada distinta, desinstálala antes. |
| Tests de Python: `ModuleNotFoundError: numpy` | `python -m pip install numpy pytest`. |
