#!/usr/bin/env python3
"""Importa una canción de YouTube a un paquete de canción (contracts/song_package.md). Agente F · B6.

    python tools/youtube_import/yt_import.py <url> --id <song_id> --personal-use \
        [--out data/songs] [--start S] [--end S] [--no-video] [--force] [--overwrite-choreo]

AVISO: descargar de YouTube con yt-dlp incumple sus condiciones de servicio. Esta herramienta es
SOLO PARA USO PERSONAL: los paquetes creados no se pueden distribuir ni subir al repositorio, y un
juego distribuido no puede incluir esta función. yt-dlp NO se incluye en el repo; instálalo tú.

Crea data/songs/<song_id>/ con:
  audio.ogg   Vorbis 44.1 kHz estéreo
  video.ogv   Theora sin audio, ≤ 720p, 30 fps (salvo --no-video)
  meta.json   origen (youtube, url, título, canal, fecha, personal_use_only: true)
  choreo.json esqueleto: BPM y offset estimados, pasos accent cada 2 beats con icon "edit_me" y
              "auto": true para que los edites (o usa tools/choreo_record.py)
  .gitignore  "*" para que el paquete no se suba por error
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

TOOLS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(TOOLS))

import choreo_validate  # noqa: E402
import jd_media  # noqa: E402

REPO = TOOLS.parent
DEFAULT_OUT = REPO / "data" / "songs"

WARNING = """\
==========================================================================================
 AVISO · SOLO USO PERSONAL
 Descargar audio/vídeo de YouTube con yt-dlp incumple las condiciones de servicio de YouTube
 y puede infringir derechos de autor. Úsalo solo con fines personales y privados:
   - no distribuyas ni compartas los paquetes creados (audio.ogg, video.ogv),
   - no los subas al repositorio (el importador añade un .gitignore en cada paquete),
   - un juego distribuido no puede incluir esta herramienta ni esos archivos.
 Si aceptas, vuelve a ejecutar con --personal-use.
=========================================================================================="""

AUDIO_FORMAT = "bestaudio/best"
VIDEO_FORMAT = "bestvideo[height<=720][fps<=60]/bestvideo[height<=720]/best[height<=720]/best"
SONG_ID_RE = re.compile(r"^[a-z0-9][a-z0-9_-]{0,63}$")


class ImportErrorClear(RuntimeError):
    """Error con mensaje para el usuario (sin traza)."""


# ------------------------------------------------------------------ yt-dlp

class YtDlpBackend:
    name = "?"

    def info(self, url: str) -> dict: ...
    def download(self, url: str, fmt: str, out_tmpl: str) -> None: ...


class YtDlpModule(YtDlpBackend):
    def __init__(self, mod):
        self.mod = mod
        self.name = "módulo yt_dlp"

    def _ydl(self, extra: dict):
        opts = {"quiet": True, "no_warnings": True, "noplaylist": True, "noprogress": True}
        opts.update(extra)
        return self.mod.YoutubeDL(opts)

    def info(self, url):
        with self._ydl({"skip_download": True}) as y:
            info = y.extract_info(url, download=False)
            return y.sanitize_info(info) if hasattr(y, "sanitize_info") else info

    def download(self, url, fmt, out_tmpl):
        with self._ydl({"format": fmt, "outtmpl": out_tmpl}) as y:
            y.extract_info(url, download=True)


class YtDlpExe(YtDlpBackend):
    def __init__(self, exe: str):
        self.exe = exe
        self.name = f"ejecutable {exe}"

    def _run(self, args, what):
        cmd = [self.exe, "--no-playlist", "--no-progress", *args]
        if self.exe.endswith(".py"):
            cmd = [sys.executable, *cmd]
        r = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if r.returncode != 0:
            msg = r.stderr.decode("utf-8", "replace").strip().splitlines()[-5:]
            raise ImportErrorClear(f"yt-dlp falló al {what} (código {r.returncode}):\n  " + "\n  ".join(msg)
                                   + "\n  (prueba a actualizarlo: `yt-dlp -U` o `pip install -U yt-dlp`)")
        return r

    def info(self, url):
        r = self._run(["--dump-single-json", "--skip-download", url], "leer la información")
        try:
            return json.loads(r.stdout.decode("utf-8", "replace"))
        except json.JSONDecodeError:
            raise ImportErrorClear("yt-dlp no devolvió JSON válido con --dump-single-json") from None

    def download(self, url, fmt, out_tmpl):
        self._run(["-f", fmt, "-o", out_tmpl, url], f"descargar ({fmt})")


def find_ytdlp() -> YtDlpBackend:
    """JD_YTDLP (ruta a ejecutable) > módulo yt_dlp > yt-dlp en el PATH."""
    env = os.environ.get("JD_YTDLP")
    if env:
        if os.path.isfile(env):
            return YtDlpExe(env)
        raise ImportErrorClear(f"JD_YTDLP apunta a '{env}', que no existe.")
    try:
        import yt_dlp  # type: ignore
        return YtDlpModule(yt_dlp)
    except ImportError:
        pass
    exe = shutil.which("yt-dlp") or shutil.which("yt-dlp.exe")
    if exe:
        return YtDlpExe(exe)
    raise ImportErrorClear(
        "No se encuentra yt-dlp (ni el módulo de Python ni el ejecutable).\n"
        "  Instálalo tú (no se incluye en el repo):  python -m pip install -U yt-dlp\n"
        "  o descarga el ejecutable de https://github.com/yt-dlp/yt-dlp/releases y ponlo en el PATH\n"
        "  (o indica su ruta con la variable JD_YTDLP).")


def _downloaded(tmp: Path, stem: str) -> Path:
    files = sorted(p for p in tmp.glob(stem + ".*") if not p.name.endswith((".part", ".ytdl", ".json")))
    if not files:
        raise ImportErrorClear(f"yt-dlp terminó pero no dejó ningún archivo '{stem}.*'")
    return files[0]


# ------------------------------------------------------------------ ffmpeg

def _trim_args(start: float | None, end: float | None) -> list[str]:
    a = []
    if start:
        a += ["-ss", f"{start:.3f}"]
    if end is not None:
        a += ["-t", f"{end - (start or 0.0):.3f}"]
    return a


def convert_audio(ffmpeg, src: Path, dst: Path, start, end):
    jd_media.run([ffmpeg, "-y", "-v", "error", *_trim_args(start, end), "-i", str(src),
                  "-map", "0:a:0", "-vn", "-sn", "-dn", "-map_metadata", "-1",
                  *jd_media.audio_codec_args(ffmpeg), str(dst)], "convertir el audio a Ogg Vorbis")


def convert_video(ffmpeg, src: Path, dst: Path, start, end):
    jd_media.run([ffmpeg, "-y", "-v", "error", *_trim_args(start, end), "-i", str(src),
                  "-map", "0:v:0", "-sn", "-dn", "-map_metadata", "-1",
                  *jd_media.video_codec_args(ffmpeg), str(dst)], "convertir el vídeo a Ogg Theora")


def check_video(ffprobe, path: Path):
    info = jd_media.probe(ffprobe, path)
    streams = info.get("streams", [])
    if any(s.get("codec_type") == "audio" for s in streams):
        raise ImportErrorClear(f"{path}: el vídeo tiene pista de audio (debería ser mudo)")
    vs = [s for s in streams if s.get("codec_type") == "video"]
    if not vs or vs[0].get("codec_name") != "theora" or int(vs[0].get("height", 0)) > 720:
        raise ImportErrorClear(f"{path}: el vídeo no es Theora ≤ 720p")


# ------------------------------------------------------------------ principal

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description="Importa audio y vídeo de YouTube a un paquete de canción (SOLO USO PERSONAL).",
        epilog="Requiere yt-dlp y ffmpeg instalados por el usuario.")
    p.add_argument("url", help="URL del vídeo de YouTube")
    p.add_argument("--id", required=True, dest="song_id", help="identificador del paquete (a-z, 0-9, _ y -)")
    p.add_argument("--personal-use", action="store_true",
                   help="confirmas que es solo para uso personal (obligatorio)")
    p.add_argument("--out", type=Path, default=DEFAULT_OUT, help=f"carpeta de canciones (por defecto {DEFAULT_OUT})")
    p.add_argument("--start", type=float, default=None, help="recortar: inicio en segundos")
    p.add_argument("--end", type=float, default=None, help="recortar: fin en segundos")
    p.add_argument("--no-video", action="store_true", help="no descargar el vídeo del bailarín")
    p.add_argument("--force", action="store_true", help="re-importar sobre un paquete existente")
    p.add_argument("--overwrite-choreo", action="store_true",
                   help="con --force, reemplaza también choreo.json (si no, se conserva)")
    return p


def import_song(args, log=print) -> Path:
    if not SONG_ID_RE.match(args.song_id):
        raise ImportErrorClear(f"--id '{args.song_id}' no válido: usa minúsculas, números, '_' o '-' (máx. 64)")
    if args.start is not None and args.start < 0:
        raise ImportErrorClear("--start debe ser >= 0")
    if args.end is not None and args.end <= (args.start or 0.0):
        raise ImportErrorClear("--end debe ser mayor que --start")

    try:
        ffmpeg, ffprobe = jd_media.require_ffmpeg()
    except jd_media.ToolMissing as e:
        raise ImportErrorClear(str(e)) from None
    yt = find_ytdlp()

    pkg = Path(args.out) / args.song_id
    choreo_path = pkg / "choreo.json"
    if pkg.exists() and any(pkg.iterdir()) and not args.force:
        raise ImportErrorClear(f"{pkg} ya existe. Usa --force para re-importar (choreo.json se conserva).")
    keep_choreo = choreo_path.exists() and not args.overwrite_choreo

    log(f"[1/5] Información del vídeo ({yt.name})…")
    info = yt.info(args.url)
    title = info.get("track") or info.get("title") or args.song_id
    channel = info.get("channel") or info.get("uploader") or ""
    artist = info.get("artist") or info.get("creator") or channel

    with tempfile.TemporaryDirectory(prefix="jd_yt_") as td:
        tmp = Path(td)
        log("[2/5] Descargando audio…")
        yt.download(args.url, AUDIO_FORMAT, str(tmp / "audio_src.%(ext)s"))
        asrc = _downloaded(tmp, "audio_src")
        vsrc = None
        if not args.no_video:
            log("[3/5] Descargando vídeo (≤ 720p)…")
            yt.download(args.url, VIDEO_FORMAT, str(tmp / "video_src.%(ext)s"))
            vsrc = _downloaded(tmp, "video_src")

        pkg.mkdir(parents=True, exist_ok=True)
        (pkg / ".gitignore").write_text("# Paquete importado de YouTube: SOLO USO PERSONAL, no se versiona.\n*\n",
                                        encoding="utf-8")
        log("[4/5] Convirtiendo con ffmpeg…")
        convert_audio(ffmpeg, asrc, pkg / "audio.ogg", args.start, args.end)
        if vsrc is not None:
            convert_video(ffmpeg, vsrc, pkg / "video.ogv", args.start, args.end)
            check_video(ffprobe, pkg / "video.ogv")
        elif (pkg / "video.ogv").exists() and not keep_choreo:
            (pkg / "video.ogv").unlink()

    log("[5/5] Estimando BPM y offset…")
    duration = jd_media.media_duration(ffprobe, pkg / "audio.ogg")
    pcm = jd_media.decode_mono(ffmpeg, pkg / "audio.ogg")
    bpm, offset = jd_media.estimate_tempo(pcm)
    if offset >= duration:
        offset = 0.0

    meta = {
        "source": "youtube",
        "url": args.url,
        "youtube_id": info.get("id"),
        "title": info.get("title"),
        "channel": channel,
        "uploader": info.get("uploader"),
        "source_duration_s": info.get("duration"),
        "trim": {"start_s": args.start, "end_s": args.end},
        "imported_at": jd_media.utc_now_iso(),
        "personal_use_only": True,
        "tool": "tools/youtube_import/yt_import.py",
        "tempo_estimate": {"bpm": bpm, "offset_s": offset, "method": "onset-strength+autocorrelation"},
    }
    (pkg / "meta.json").write_text(json.dumps(meta, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    if keep_choreo:
        log(f"  choreo.json existente conservado (estimación: {bpm} BPM, offset {offset} s)")
    else:
        steps = jd_media.skeleton_steps(bpm, offset, duration)
        choreo = jd_media.make_choreo(title, artist, bpm, offset, duration, steps,
                                      video=None if args.no_video else "video.ogv")
        choreo["notes"] = ("Esqueleto automático: BPM/offset estimados y pasos 'edit_me' cada 2 beats. "
                           "Revísalos, cambia limbs/icon/kind y borra 'auto'.")
        choreo_validate.save(choreo_path, choreo)
    log(f"Listo: {pkg}  ({duration:.1f} s, ~{bpm} BPM, primer beat {offset} s)")
    log("Recuerda: SOLO USO PERSONAL; no distribuyas este paquete.")
    return pkg


def main(argv=None) -> int:
    args = build_parser().parse_args(argv)
    if not args.personal_use:
        print(WARNING, file=sys.stderr)
        return 2
    print(WARNING.splitlines()[1].strip() + " (aceptado con --personal-use)", file=sys.stderr)
    try:
        import_song(args)
    except (ImportErrorClear, choreo_validate.ChoreoError, RuntimeError) as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
