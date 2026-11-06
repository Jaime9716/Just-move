"""Fixtures comunes de las pruebas de tools/ (agente F)."""
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest

TOOLS = Path(__file__).resolve().parent.parent
REPO = TOOLS.parent
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "youtube_import"))

needs_ffmpeg = pytest.mark.skipif(not (shutil.which("ffmpeg") and shutil.which("ffprobe")),
                                  reason="ffmpeg/ffprobe no instalados")


def click_track(bpm=120.0, offset=1.0, seconds=12.0, sr=44100, seed=0):
    """Clics (1.5 kHz, 20 ms) a `bpm` desde `offset`, con algo de ruido."""
    rng = np.random.default_rng(seed)
    x = np.zeros(int(seconds * sr))
    n = int(0.02 * sr)
    c = np.sin(2 * np.pi * 1500 * np.arange(n) / sr) * np.exp(-np.arange(n) / (0.004 * sr))
    t = offset
    while t < seconds - 0.05:
        i = int(round(t * sr))
        x[i:i + n] += 0.8 * c[: max(0, min(n, x.size - i))]
        t += 60.0 / bpm
    return x + 0.005 * rng.standard_normal(x.size), sr


def write_wav(path, x, sr):
    import wave
    pcm = (np.clip(x, -1, 1) * 32767).astype("<i2")
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sr)
        w.writeframes(pcm.tobytes())


@pytest.fixture(scope="session")
def source_media(tmp_path_factory):
    """Simula lo que bajaría yt-dlp: audio .webm-like (aquí .mka) y vídeo 1280x960@25 CON audio."""
    if not shutil.which("ffmpeg"):
        pytest.skip("ffmpeg no instalado")
    d = tmp_path_factory.mktemp("yt_src")
    x, sr = click_track(120.0, 1.0, 12.0)
    wav = d / "clicks.wav"
    write_wav(wav, x, sr)
    audio = d / "audio_src.mka"
    subprocess.run(["ffmpeg", "-y", "-v", "error", "-i", str(wav), "-c:a", "pcm_s16le", str(audio)], check=True)
    video = d / "video_src.mkv"
    subprocess.run(["ffmpeg", "-y", "-v", "error", "-f", "lavfi", "-i", "testsrc=size=1280x960:rate=25:duration=12",
                    "-i", str(wav), "-shortest", "-c:v", "mpeg4", "-q:v", "8", "-c:a", "pcm_s16le", str(video)],
                   check=True)
    return {"audio": audio, "video": video, "dir": d}
