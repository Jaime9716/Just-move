# P9 · Rendimiento (fps)

**Criterio:** fps medio ≥ 58,8; ≥ 99 % de fotogramas ≥ 54 fps; ningún fotograma > 50 ms (tras 2 s). Tiempo: 10 min.

1. Menú del juego → marca la casilla **log de fps** (o lanza Godot con `-- --fps-log`).
2. Juega una **canción de YouTube completa con vídeo** (720p). Apunta la resolución de la ventana y si
   está a pantalla completa, y la frecuencia del monitor (60/120/144 Hz).
3. Al acabar, **copia** el log antes de jugar otra vez (se sobrescribe en cada partida):
   ```powershell
   copy "$env:APPDATA\Godot\app_userdata\Juego de Baile\fps_log.csv" tests\system\results\p9_fps_<cancion>.csv
   python tests\system\tools\fps_log_stats.py tests\system\results\p9_fps_<cancion>.csv
   ```
   Con un monitor de 144 Hz y vsync, usa `--target 144` (o el script te avisará).
4. Repite con la demo (vídeo ligero) para comparar, y una vez con el modo "plan B" de vídeo si lo ves en la
   línea de depuración.
5. Si hay tirones, ¿coinciden con algo (un paso, el principio del vídeo, conexión del móvil)? El script
   lista los peores con su segundo.
