"""Pruebas del importador de YouTube con yt-dlp simulado (sin red)."""
import json
import os
import subprocess
import sys
import types
from pathlib import Path

import pytest

from conftest import TOOLS, needs_ffmpeg

SCRIPT = TOOLS / "youtube_import" / "yt_import.py"
FAKE = Path(__file__).resolve().parent / "fake_yt_dlp.py"
URL = "https://www.youtube.com/watch?v=abc123XYZ"


def run_import(args, env_extra=None, path_dirs=()):
    env = {k: v for k, v in os.environ.items() if k not in ("JD_YTDLP", "JD_FFMPEG", "JD_FFPROBE")}
    env["PATH"] = os.pathsep.join([*map(str, path_dirs), env.get("PATH", "")])
    env.update(env_extra or {})
    return subprocess.run([sys.executable, str(SCRIPT), *args], env=env, capture_output=True, text=True)


def ffprobe(path):
    r = subprocess.run(["ffprobe", "-v", "error", "-print_format", "json", "-show_streams", "-show_format",
                        str(path)], capture_output=True, text=True, check=True)
    return json.loads(r.stdout)


@pytest.fixture
def fake_bin(tmp_path):
    b = tmp_path / "bin"
    b.mkdir()
    exe = b / "yt-dlp"
    exe.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{FAKE}" "$@"\n')
    exe.chmod(0o755)
    return b


def test_without_personal_use_prints_warning_and_exits(tmp_path):
    r = run_import([URL, "--id", "x", "--out", str(tmp_path)])
    assert r.returncode == 2
    assert "SOLO USO PERSONAL" in r.stderr and "--personal-use" in r.stderr
    assert not (tmp_path / "x").exists()


def test_missing_ytdlp_is_clear(tmp_path):
    # PATH sin yt-dlp (y el módulo yt_dlp no está instalado en el entorno de pruebas).
    env = {"PATH": str(Path(sys.executable).parent) + os.pathsep + "/usr/bin:/bin"}
    r = run_import([URL, "--id", "x", "--out", str(tmp_path), "--personal-use"], env_extra=env)
    if "yt_dlp" in sys.modules or subprocess.run([sys.executable, "-c", "import yt_dlp"]).returncode == 0:
        pytest.skip("yt_dlp instalado en este entorno")
    assert r.returncode == 1
    assert "No se encuentra yt-dlp" in r.stderr and "pip install" in r.stderr


def test_missing_ffmpeg_is_clear(tmp_path, fake_bin):
    r = run_import([URL, "--id", "x", "--out", str(tmp_path), "--personal-use"],
                   env_extra={"JD_FFMPEG": "/no/existe/ffmpeg"}, path_dirs=[fake_bin])
    assert r.returncode == 1
    assert "No se encuentra ffmpeg" in r.stderr


def test_bad_id_and_trim(tmp_path, fake_bin):
    r = run_import([URL, "--id", "Mal ID", "--out", str(tmp_path), "--personal-use"], path_dirs=[fake_bin])
    assert r.returncode == 1 and "--id" in r.stderr
    r = run_import([URL, "--id", "ok", "--out", str(tmp_path), "--personal-use", "--start", "5", "--end", "2"],
                   path_dirs=[fake_bin])
    assert r.returncode == 1 and "--end" in r.stderr


@needs_ffmpeg
def test_ytdlp_failure_is_reported(tmp_path, fake_bin, source_media):
    r = run_import([URL, "--id", "x", "--out", str(tmp_path), "--personal-use"],
                   env_extra={"FAKE_YT_SRC": str(source_media["dir"]), "FAKE_YT_FAIL": "1"}, path_dirs=[fake_bin])
    assert r.returncode == 1
    assert "yt-dlp falló" in r.stderr and "Video unavailable" in r.stderr


@needs_ffmpeg
def test_full_import_with_fake_ytdlp(tmp_path, fake_bin, source_media):
    log = tmp_path / "calls.jsonl"
    out = tmp_path / "songs"
    r = run_import([URL, "--id", "prueba_yt", "--out", str(out), "--personal-use"],
                   env_extra={"FAKE_YT_SRC": str(source_media["dir"]), "FAKE_YT_LOG": str(log)},
                   path_dirs=[fake_bin])
    assert r.returncode == 0, r.stderr + r.stdout
    pkg = out / "prueba_yt"

    # audio.ogg: Vorbis 44.1 kHz estéreo
    a = ffprobe(pkg / "audio.ogg")
    (s,) = a["streams"]
    assert s["codec_name"] == "vorbis" and int(s["sample_rate"]) == 44100 and s["channels"] == 2

    # video.ogv: Theora, sin audio, ≤ 720p, 30 fps
    v = ffprobe(pkg / "video.ogv")
    assert [st["codec_type"] for st in v["streams"]] == ["video"]
    vs = v["streams"][0]
    assert vs["codec_name"] == "theora"
    assert int(vs["height"]) == 720 and int(vs["width"]) == 960
    # En Ogg/Theora avg_frame_rate sale 0/0: se usa r_frame_rate y se cuentan fotogramas.
    num, den = map(int, vs["r_frame_rate"].split("/"))
    assert abs(num / den - 30.0) < 0.01
    r = subprocess.run(["ffprobe", "-v", "error", "-count_frames", "-select_streams", "v:0", "-show_entries",
                        "stream=nb_read_frames", "-of", "csv=p=0", str(pkg / "video.ogv")],
                       capture_output=True, text=True, check=True)
    assert abs(int(r.stdout.strip()) - 360) <= 2  # 12 s × 30 fps

    meta = json.loads((pkg / "meta.json").read_text(encoding="utf-8"))
    assert meta["source"] == "youtube" and meta["url"] == URL
    assert meta["title"] == "Canción de prueba" and meta["channel"] == "Canal Falso"
    assert meta["personal_use_only"] is True and meta["imported_at"].endswith("Z")

    import choreo_validate
    c = choreo_validate.load(pkg / "choreo.json")
    assert abs(c["song"]["bpm"] - 120.0) <= 2.0
    assert abs(c["song"]["offset_s"] - 1.0) <= 0.03
    assert c["song"]["video"] == "video.ogv"
    assert abs(c["song"]["duration_s"] - 12.0) < 0.1
    steps = c["steps"]
    assert len(steps) >= 4 and all(s["kind"] == "accent" and s["auto"] and s["icon"] == "edit_me" for s in steps)
    gaps = {round(b["t"] - a["t"], 2) for a, b in zip(steps, steps[1:])}
    assert all(abs(g - 1.0) <= 0.04 for g in gaps), gaps  # cada 2 beats a 120 BPM

    assert (pkg / ".gitignore").read_text().strip().endswith("*")
    calls = [json.loads(l) for l in log.read_text().splitlines()]
    assert any("--dump-single-json" in c for c in calls)
    assert sum("-f" in c for c in calls) == 2

    # Re-importar sin --force falla; con --force conserva choreo.json editado.
    r = run_import([URL, "--id", "prueba_yt", "--out", str(out), "--personal-use"],
                   env_extra={"FAKE_YT_SRC": str(source_media["dir"])}, path_dirs=[fake_bin])
    assert r.returncode == 1 and "--force" in r.stderr
    c["song"]["title"] = "Editada"
    choreo_validate.save(pkg / "choreo.json", c)
    r = run_import([URL, "--id", "prueba_yt", "--out", str(out), "--personal-use", "--force", "--no-video"],
                   env_extra={"FAKE_YT_SRC": str(source_media["dir"])}, path_dirs=[fake_bin])
    assert r.returncode == 0, r.stderr
    assert choreo_validate.load(pkg / "choreo.json")["song"]["title"] == "Editada"


@needs_ffmpeg
def test_trim_and_module_backend(tmp_path, source_media, monkeypatch):
    """Usa un módulo yt_dlp falso (API YoutubeDL) en proceso, con --start/--end y sin vídeo."""
    import shutil as _sh
    calls = []

    class FakeYDL:
        def __init__(self, opts):
            self.opts = opts
            calls.append(opts)

        def __enter__(self):
            return self

        def __exit__(self, *a):
            return False

        def extract_info(self, url, download=False):
            if download:
                src = source_media["audio"] if self.opts["format"].startswith("bestaudio") else source_media["video"]
                _sh.copy(src, self.opts["outtmpl"].replace("%(ext)s", src.suffix[1:]))
            return {"id": "m1", "title": "Mod", "uploader": "Uploader", "artist": "Artista Real", "duration": 12}

        def sanitize_info(self, info):
            return info

    monkeypatch.setitem(sys.modules, "yt_dlp", types.SimpleNamespace(YoutubeDL=FakeYDL))
    monkeypatch.delenv("JD_YTDLP", raising=False)
    import yt_import
    rc = yt_import.main([URL, "--id", "recorte", "--out", str(tmp_path), "--personal-use",
                         "--start", "2", "--end", "10", "--no-video"])
    assert rc == 0
    pkg = tmp_path / "recorte"
    assert not (pkg / "video.ogv").exists()
    import choreo_validate
    c = choreo_validate.load(pkg / "choreo.json")
    assert c["song"]["video"] is None
    assert c["song"]["artist"] == "Artista Real"
    assert abs(c["song"]["duration_s"] - 8.0) < 0.1
    assert abs(c["song"]["bpm"] - 120.0) <= 2.0
    # Clics en 1.0 + k·0.5 → tras recortar 2 s, primer beat en ~0.0 (o 0.5).
    assert min(abs(c["song"]["offset_s"] - 0.0), abs(c["song"]["offset_s"] - 0.5)) <= 0.03
    assert all(o["noplaylist"] for o in calls)
