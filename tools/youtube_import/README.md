# Importador de YouTube (solo uso personal)

> **AVISO.** Descargar audio o vídeo de YouTube con yt-dlp **incumple las condiciones de servicio de
> YouTube** y puede infringir derechos de autor. Esta herramienta existe **solo para uso personal y
> privado** por decisión del dueño del proyecto:
>
> - no distribuyas ni compartas los paquetes que crea (`audio.ogg`, `video.ogv`, `meta.json`…);
> - no los subas al repositorio (el importador escribe un `.gitignore` con `*` dentro de cada paquete
>   y los binarios ya están ignorados en el `.gitignore` raíz);
> - **un juego distribuido no puede incluir esta herramienta ni esos archivos.**
>
> yt-dlp **no** está ni estará en este repositorio: lo instalas tú.

## Requisitos

- Python 3.11+ con numpy.
- **yt-dlp**: `python -m pip install -U yt-dlp` (módulo) o el ejecutable de
  <https://github.com/yt-dlp/yt-dlp/releases> en el PATH. También vale `JD_YTDLP=/ruta/a/yt-dlp`.
  YouTube cambia a menudo: si falla la descarga, actualízalo (`yt-dlp -U` / `pip install -U yt-dlp`).
- **ffmpeg + ffprobe** con `libvorbis` y `libtheora` (Windows: `winget install Gyan.FFmpeg`;
  Linux: `sudo apt install ffmpeg`). Rutas alternativas: `JD_FFMPEG`, `JD_FFPROBE`.

## Uso

```bash
python tools/youtube_import/yt_import.py "https://www.youtube.com/watch?v=XXXX" \
    --id mi_cancion --personal-use [--out data/songs] [--start 12.5 --end 95] [--no-video]
```

Sin `--personal-use` imprime el aviso y sale (código 2) sin descargar nada.

| Opción | Efecto |
|---|---|
| `--id` | carpeta del paquete: `a-z`, `0-9`, `_`, `-` |
| `--out` | carpeta de canciones (por defecto `data/songs/` del repo) |
| `--start`, `--end` | recorta audio y vídeo igual (segundos del original) |
| `--no-video` | solo audio (`song.video = null`) |
| `--force` | re-importa sobre un paquete existente; **conserva `choreo.json`** |
| `--overwrite-choreo` | con `--force`, regenera también el esqueleto de `choreo.json` |

## Qué hace

1. Lee la información del vídeo (título, canal) con yt-dlp.
2. Descarga el mejor audio (`bestaudio`) y el mejor vídeo de ≤ 720p, por separado.
3. ffmpeg convierte:
   - `audio.ogg`: Ogg Vorbis, 44.1 kHz, estéreo;
   - `video.ogv`: Ogg Theora **sin pista de audio**, ≤ 720 px de alto, 30 fps, yuv420p
     (lo que reproduce Godot; el vídeo es esclavo del SongClock).
4. Estima BPM (70–180) y offset del primer beat: PCM mono 22.05 kHz vía ffmpeg → flujo espectral
   (onset strength) → autocorrelación con peine de múltiplos y prior suave en 120 BPM → ajuste fino de
   periodo y fase sobre toda la canción. Puede equivocarse ×2 (p. ej. 85 vs 170) en música real:
   **revísalo**.
5. Escribe `meta.json` (`source: "youtube"`, `url`, `title`, `channel`, `imported_at`,
   `personal_use_only: true`, recorte y estimación de tempo) y `choreo.json` válido con pasos `accent`
   cada 2 beats alternando manos, con `"icon": "edit_me"` y `"auto": true` para que los edites.

## Después

- Edita `choreo.json` a mano y valida: `python tools/choreo_validate.py data/songs/mi_cancion`.
- O baila la canción con el móvil (`jd_record`) y genera referencia + pasos:
  `python tools/choreo_record.py sesion.jsonl --song data/songs/mi_cancion --offset <s> [--merge]`.

## Pruebas

`python -m pytest tools/tests -q` — usan un yt-dlp simulado (`tools/tests/fake_yt_dlp.py`, sin red)
y medios generados con ffmpeg (clics a 120 BPM; se comprueba 120 ± 2 BPM y que `video.ogv` es Theora
a 30 fps sin pista de audio con ffprobe).
