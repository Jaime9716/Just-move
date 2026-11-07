# P11 · Linux

**Criterio:** P1–P5 cumplen con los mismos criterios que en Windows; P3 y P4 difieren ≤ 5 ms de Windows.
Tiempo: 2 h.

1. Compila: `scripts/build_linux.sh` (y `scripts/run_tests.sh`). Herramientas en `engine/target/release/`.
2. Red: misma Wi-Fi; si usas `ufw`: `sudo ufw allow 47800/udp`. Con firewalld:
   `sudo firewall-cmd --add-port=47800/udp`.
3. Repite las guías P1–P5 cambiando las rutas:
   - `<TOOLS>/jd_record` en lugar de `jd_record.exe`;
   - `user://` = `~/.local/share/godot/app_userdata/Juego de Baile/`;
   - `python3 tests/system/tools/...` (mismos scripts y opciones).
4. Audio: apunta si usas PulseAudio o PipeWire y la salida (la latencia de audio en Linux cambia la calibración).
5. En `results.csv` pon `entorno` = `Linux <distro>, <móvil>, <Wi-Fi>` para poder comparar fila a fila.
