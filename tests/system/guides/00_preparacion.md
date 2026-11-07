# 00 · Preparación común (una vez)

1. **Juego y herramientas**: sigue `BUILD.md` (o descarga el artefacto de GitHub Actions
   `juego-jd_godot-windows-x86_64`). Necesitas `jd_record.exe` (en `engine\target\release\` o en la
   carpeta del artefacto). En estas guías la llamamos **`<TOOLS>`**.
2. **App de mando** instalada en el móvil (`BUILD.md` §4).
3. **Python 3.11+ con numpy** (`python -m pip install numpy pytest`) y **ffmpeg** en el PATH
   (`winget install Gyan.FFmpeg`; comprueba `ffmpeg -version` y `ffprobe -version`).
4. **Red** (`BUILD.md` §5): PC y móvil en la misma Wi-Fi (no la de invitados), perfil **Privada**,
   regla de Firewall UDP 47800, sin VPN en el móvil. Apunta si es 2,4 o 5 GHz.
5. **Carpetas de resultados** (desde la raíz del repo, en PowerShell):
   ```powershell
   mkdir data\sessions\real, tests\system\results -Force
   copy tests\system\results_template.csv tests\system\results.csv
   ```
6. **Carpeta de datos del juego** (`user://`):
   `%APPDATA%\Godot\app_userdata\Juego de Baile\` — ahí están `profile.cfg` (calibración),
   `fps_log.csv` (rendimiento) y `songs\` (canciones importadas). Ábrela con Win+R y pega la ruta.
7. **Móvil**: brillo automático desactivado no hace falta; sí **desactiva el ahorro de batería** salvo en
   P8 y deja la pantalla encendida (la app la mantiene encendida sola).

Todos los comandos `python tests\system\tools\...` se ejecutan **desde la raíz del repositorio**.
Al terminar cada prueba: fila en `tests\system\results.csv` y, si hay algo que contar,
`docs\feedback\AAAA-MM-DD-<momento>.md`.
