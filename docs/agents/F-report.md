# Informe del agente F · Coreografías (B6) e importador de YouTube

## Qué hice

| Archivo | Contenido |
|---|---|
| `engine/jd_core/src/choreo.rs` | `load_choreo(path)`, `validate_choreo(&Choreo)` (firmas sin cambiar) y extra `parse_choreo(text)`. Usa `crate::json`. Informa **todos** los errores (máx. 20), uno por línea, con ruta JSON (`steps[3].t: debe ser >= steps[2].t (4.2 < 4.5)`); `load_choreo` antepone el archivo a cada línea; JSON corrupto → `archivo:línea:col: …` (de `json::parse_file`). |
| `engine/jd_core/tests/f_choreo.rs` | 13 pruebas: válidos, incompletos, corruptos, tipos, desordenados, limbs desconocidas/repetidas/vacías, kind inválido, rangos, rutas, `data/songs/*/choreo.json` válidos y **casos compartidos con Python**. |
| `tools/choreo_validate.py` | Mismas reglas y mensajes que Rust. CLI (`archivo` o carpeta de paquete) y módulo (`validate`, `load`, `save`, `dumps`). |
| `tools/tests/choreo_cases/cases.json` | 39 casos que ejecutan **los dos** validadores (paridad Rust ↔ Python). |
| `tools/jd_media.py` | ffmpeg/ffprobe (con errores claros), decodificación PCM, estimación de tempo, esqueleto de choreo. |
| `tools/youtube_import/yt_import.py` + `README.md` | Importador (detalles abajo), con aviso de uso personal. |
| `tools/choreo_record.py` | Grabador: sesión de `jd_record` + offset → `ref_imu.jsonl` (time_base song) + pasos accent propuestos. |
| `tools/make_demo_song.py` | Genera `data/songs/demo_metronomo/` (30 s, 120 BPM, offset 1.0 s, 29 pasos con accent/move y las 4 extremidades). |
| `data/songs/demo_metronomo/` | `choreo.json` y `meta.json` (versionables, deterministas); `audio.ogg`/`video.ogv` generados (ignorados por git). |
| `tools/tests/` | 69 pruebas pytest (validador, tempo, importador con yt-dlp simulado, grabador, demo). |

### Reglas de validación (Rust = Python)
`format == "jd-choreo"`, `version == 1`; `song.title` texto no vacío; `song.artist` texto opcional;
`song.audio` ruta relativa no vacía; `song.video` y `references.imu` ruta relativa, null o ausente;
rutas sin `/` inicial, `\`, `:` ni `..`; `0 < bpm ≤ 400`; `duration_s > 0`; `0 ≤ offset_s < duration_s`;
por paso: `t ≥ 0`, `dur ≥ 0`, `t + dur ≤ duration_s (+1e-6)`, `limbs` lista no vacía de nombres
conocidos sin repetir, `icon` texto opcional (→ ""), `kind` ∈ {accent, move}; `t` no decreciente
(pasos simultáneos permitidos). Campos desconocidos se ignoran. Primero se comprueba la estructura;
si falla no se hacen las comprobaciones semánticas (evita errores en cascada).

### Importador de YouTube
`python tools/youtube_import/yt_import.py <url> --id <song_id> --personal-use [--out] [--start --end] [--no-video] [--force] [--overwrite-choreo]`
- Sin `--personal-use` → aviso y código 2. yt-dlp: `JD_YTDLP` > módulo `yt_dlp` > ejecutable en PATH;
  si falta, mensaje con cómo instalarlo. ffmpeg/ffprobe: PATH o `JD_FFMPEG`/`JD_FFPROBE`.
- Descarga audio (`bestaudio`) y vídeo (≤ 720p) por separado; ffmpeg → `audio.ogg` (libvorbis q5,
  44.1 kHz estéreo) y `video.ogv` (libtheora, `-an`, `scale=-2:min(720,ih)`, 30 fps, yuv420p); comprueba
  con ffprobe que el vídeo es Theora sin audio.
- Tempo: PCM mono 22.05 kHz → flujo espectral log (n_fft 1024, hop 128) → autocorrelación con peine
  (1, 2, 3 periodos) y prior log-normal en 120 BPM, regla de nivel métrico (si a medio periodo hay ≥ 70 %
  de correlación, dobla el tempo), ajuste fino conjunto periodo/fase sobre toda la canción; offset =
  primer beat de la rejilla con golpe claro, corregido 13 ms de latencia del flujo (medido).
- `meta.json` (source, url, youtube_id, title, channel, uploader, trim, imported_at, personal_use_only,
  tempo_estimate) y `choreo.json` con pasos accent cada 2 beats (`icon: "edit_me"`, `auto: true`,
  `notes` arriba). Escribe `.gitignore` con `*` dentro del paquete. `--force` conserva `choreo.json`.

### Grabador (`choreo_record.py`)
`t_song = t_sesión + --offset` (offset = tiempo de canción en t=0 de la grabación; obligatorio si
time_base es "mono"). Escribe `ref_imu.jsonl` recortado a [0, duration_s] conservando a/g/src/limb.
Picos por extremidad con las reglas del juez v1 (≥ 8 m/s², ≥ 1.5 × mediana del último 1 s,
separación 0.15 s) sobre |a − gravedad| (gravedad por paso bajo, no la orientación completa de B3);
cuantiza a `--grid` beats, descarta los que se desvían más de `--max-dev` × rejilla; picos simultáneos
de varias extremidades → un paso con varias limbs. `--merge` conserva pasos existentes y solo añade
extremidades no cubiertas cerca (±½ rejilla). Siempre deja `choreo.json.bak`; `--dry-run` disponible.

## Cómo probarlo
```bash
cd engine && CARGO_TARGET_DIR=../build/F cargo test -p jd_core --test f_choreo   # y choreo:: unitarias
python3 -m pytest tools/tests -q                                               # 69 pruebas, ~20 s
python3 tools/make_demo_song.py                                                # regenera la demo
python3 tools/choreo_validate.py data/songs/*/
```
Resultados aquí: 13/13 Rust (+3 unitarias), 69/69 pytest, `cargo build -p jd_core` sin warnings.
Tempo en clics: 72–174 BPM con error < 0.05 BPM y offset < 20 ms; 120 BPM del importador → 120.0.

## Qué no pude verificar
- **yt-dlp real y YouTube** (no hay red ni yt-dlp): probado con un yt-dlp simulado (ejecutable y módulo
  `YoutubeDL`). Los argumentos usados (`--dump-single-json --skip-download`, `-f`, `-o`, `--no-playlist`,
  `--no-progress`) son estándar, pero conviene una prueba real en el PC del usuario.
- Estimación de tempo con **música real**: solo probada con clics sintéticos (también con contratiempos).
  Puede equivocarse ×2 o con tempo variable; el README avisa de revisarlo.
- Que Godot 4.4 reproduzca los `.ogv` generados (no hay Godot aquí). Son Theora yuv420p 30 fps estándar.
- Grabador con una sesión real de `jd_record` (usa el formato del contrato; probado con sesiones sintéticas).
- Rutas de Windows en el importador (usa `pathlib`; `.exe` de yt-dlp se busca en el PATH).

## Notas para otros agentes
- En Ogg/Theora ffprobe da `avg_frame_rate=0/0`; usad `r_frame_rate` o contad fotogramas (E, T2).
- `data/songs/demo_metronomo/`: primer beat en 1.0 s, primer paso en 3.0 s (compás 0 = cuenta atrás).
  Útil para pruebas de sincronía (D/E) y del motor (C/T1).
- Las coreografías generadas llevan campos extra (`auto`, `notes`): el cargador Rust y el validador
  Python los ignoran; el prototipo Python (B) debería ignorarlos también.

## Propuestas de contrato (`contracts/song_package.md`)
1. Documentar las reglas exactas de arriba (rangos de bpm/offset/duración, `t+dur ≤ duration_s`, limbs
   sin repetir, rutas relativas sin `..`, `artist`/`icon`/`video`/`references` opcionales, campos
   desconocidos ignorados) y que se informan todos los errores, uno por línea.
2. Reservar los campos opcionales `step.auto` (bool, paso propuesto automáticamente) y `notes` (texto).
3. `meta.json`: admitir `source: "generated"` y los campos extra (`channel`, `title`, `youtube_id`,
   `trim`, `tempo_estimate`).
4. Añadir `song.preview_s` (inicio de la vista previa en el menú) si E lo necesita.
