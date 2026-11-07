# P3 · Sincronía audio–imagen (con la cámara de otro móvil)

**Criterio:** (a) destellos de la interfaz vs clics: |desfase| ≤ 10 ms al principio y al final;
(b) vídeo del bailarín vs música: ≤ 40 ms (nunca > 70 ms) y sin deriva creciente. Tiempo: 30 min.

Necesitas **otro móvil** (o cámara) que grabe vídeo **con sonido**, mejor en **cámara lenta** (120/240 fps).
Desfase = t_destello − t_clic: **positivo = la imagen va tarde**.

## Preparar
1. Audio por **altavoces con cable** o los del monitor (no Bluetooth; si solo tienes Bluetooth, apúntalo).
2. En el móvil-cámara: modo cámara lenta. Haz una prueba de 5 s y mira si el vídeo **tiene sonido**
   (muchos móviles lo quitan en cámara lenta; si es así usa vídeo normal a 60 fps y graba más tiempo).
   Comprueba qué guarda: `ffprobe -hide_banner tu_video.mp4` → si dice p. ej. `240 fps` y la duración es
   la real, usa `--slowdown 1`; si dice `30 fps` y el vídeo dura 8 veces más que la escena, `--slowdown 8`.
3. Pon el móvil-cámara fijo (apoyado o con trípode) a 30–50 cm, enfocando la zona que destella y
   **cerca del altavoz** (el sonido tarda ~3 ms por metro).
4. Visual offset del perfil: apunta el valor actual (en "Calibrar pantalla").

## (a) Interfaz: escena "Calibrar pantalla"
1. Menú → **Calibrar pantalla**. Destella un cuadro a cada clic. No toques el deslizador.
2. Graba **3 minutos** seguidos (para ver deriva). Copia el vídeo al PC en `tests\system\results\p3a.mp4`.
3. Analiza (con `--roi` encuadras solo el cuadro que destella: x,y,ancho,alto en fracciones del fotograma):
   ```powershell
   python tests\system\tools\av_sync_from_video.py tests\system\results\p3a.mp4 --slowdown 1
   python tests\system\tools\av_sync_from_video.py tests\system\results\p3a.mp4 --roi 0.3,0.3,0.4,0.4 --verbose
   ```
   Mira `principio`, `final`, `deriva` y el veredicto. Si dice "no veo destellos", ajusta `--roi`.

## (b) Vídeo del bailarín: canción demo larga
1. Genera una demo de 3 min en tu carpeta de canciones (gana a la de `data\songs`):
   ```powershell
   python tools\make_demo_song.py --seconds 180 --out "$env:APPDATA\Godot\app_userdata\Juego de Baile\songs"
   ```
   (Para volver a la de 30 s, borra esa carpeta `demo_metronomo`.)
2. Juega "Demo metrónomo" (no hace falta bailar). Graba con el móvil-cámara **la zona del vídeo del
   bailarín** (el fondo destella en cada beat) desde el principio hasta el final.
3. `python tests\system\tools\av_sync_from_video.py tests\system\results\p3b.mp4 --roi <zona del vídeo>`
   Aquí el criterio es 40 ms: añade `--tol-ms 40`. Apunta también lo que diga la línea de depuración del
   juego (arriba a la derecha: "seek"/"plan B" y deriva).

## Sesgo de la propia cámara (opcional)
La cámara también graba su audio con unos ms de desfase propio (y el sonido tarda ~3 ms por metro).
Por eso cuenta sobre todo la **diferencia entre principio y final** y comparar antes/después de un
cambio. Si conoces ese sesgo (p. ej. midiéndolo con otro método), réstalo con `--camera-offset-ms`.

**Apunta** principio, final y deriva de (a) y (b) en `results.csv` y guarda `--json` en `tests\system\results\`.
Los vídeos no se suben al repo (pesan); guárdalos por si hay que repetir el análisis.
