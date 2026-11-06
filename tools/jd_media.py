"""Utilidades de medios compartidas por las herramientas de F (ffmpeg, tempo, esqueleto de choreo).

- ``require_ffmpeg()``: rutas de ffmpeg/ffprobe o ``ToolMissing`` con mensaje claro.
- ``decode_mono(path, sr)``: PCM mono float32 vía ffmpeg.
- ``estimate_tempo(pcm, sr)``: (bpm, offset_s) con onset strength + autocorrelación (70–180 BPM).
- ``skeleton_steps(bpm, offset, duration)``: pasos accent cada 2 beats marcados para editar.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

import numpy as np

BPM_MIN = 70.0
BPM_MAX = 180.0
ANALYSIS_SR = 22050
N_FFT = 1024
HOP = 128
# Retardo medio entre la entrada de un golpe y el máximo del flujo espectral con estos parámetros
# (medido con clics sintéticos; ver tools/tests/test_tempo.py). Se suma al offset estimado.
ONSET_LAG_S = 0.013


class ToolMissing(RuntimeError):
    pass


def find_tool(name: str, env_var: str) -> str | None:
    p = os.environ.get(env_var)
    if p:
        return p if (os.path.isfile(p) and os.access(p, os.X_OK)) else None
    return shutil.which(name)


def require_ffmpeg() -> tuple[str, str]:
    ff = find_tool("ffmpeg", "JD_FFMPEG")
    fp = find_tool("ffprobe", "JD_FFPROBE")
    if not ff or not fp:
        missing = " y ".join(n for n, x in (("ffmpeg", ff), ("ffprobe", fp)) if not x)
        raise ToolMissing(
            f"No se encuentra {missing}. Instálalo y ponlo en el PATH "
            "(Windows: `winget install Gyan.FFmpeg`; Linux: `sudo apt install ffmpeg`), "
            "o indica la ruta con JD_FFMPEG / JD_FFPROBE.")
    return ff, fp


def run(cmd: list[str], what: str) -> subprocess.CompletedProcess:
    r = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if r.returncode != 0:
        tail = r.stderr.decode("utf-8", "replace").strip().splitlines()[-8:]
        raise RuntimeError(f"Falló {what} (código {r.returncode}):\n  " + "\n  ".join(tail))
    return r


def ffmpeg_has_encoder(ffmpeg: str, name: str) -> bool:
    r = subprocess.run([ffmpeg, "-hide_banner", "-encoders"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    return any(line.split()[1:2] == [name] for line in r.stdout.decode("utf-8", "replace").splitlines()
               if len(line.split()) > 1)


def audio_codec_args(ffmpeg: str) -> list[str]:
    """Vorbis 44.1 kHz estéreo (libvorbis; si falta, el codificador nativo experimental)."""
    if ffmpeg_has_encoder(ffmpeg, "libvorbis"):
        return ["-c:a", "libvorbis", "-q:a", "5", "-ar", "44100", "-ac", "2"]
    return ["-c:a", "vorbis", "-strict", "-2", "-ar", "44100", "-ac", "2"]


def video_codec_args(ffmpeg: str) -> list[str]:
    if not ffmpeg_has_encoder(ffmpeg, "libtheora"):
        raise ToolMissing("Tu ffmpeg no tiene el codificador libtheora (necesario para video.ogv). "
                          "Instala una compilación completa de ffmpeg.")
    # ≤ 720 de alto, ancho par, 30 fps constantes, yuv420p (lo que reproduce Godot).
    return ["-an", "-vf", "scale=-2:'min(720,ih)':flags=bicubic,fps=30,format=yuv420p",
            "-c:v", "libtheora", "-q:v", "7", "-g", "30"]


def probe(ffprobe: str, path) -> dict:
    r = run([ffprobe, "-v", "error", "-print_format", "json", "-show_format", "-show_streams", str(path)],
            f"ffprobe {path}")
    return json.loads(r.stdout.decode("utf-8", "replace"))


def media_duration(ffprobe: str, path) -> float:
    info = probe(ffprobe, path)
    d = info.get("format", {}).get("duration")
    if d is None:
        ds = [float(s["duration"]) for s in info.get("streams", []) if "duration" in s]
        d = max(ds) if ds else 0.0
    return float(d)


def decode_mono(ffmpeg: str, path, sr: int = ANALYSIS_SR) -> np.ndarray:
    r = run([ffmpeg, "-v", "error", "-i", str(path), "-vn", "-ac", "1", "-ar", str(sr), "-f", "f32le", "-"],
            f"decodificar audio de {path}")
    return np.frombuffer(r.stdout, dtype="<f4").astype(np.float64)


# ------------------------------------------------------------------ tempo

def onset_strength(pcm: np.ndarray, sr: int = ANALYSIS_SR, n_fft: int = N_FFT, hop: int = HOP):
    """Flujo espectral log (rectificado) por trama. Devuelve (env, fps). env[i] ~ t = i*hop/sr."""
    if pcm.size < n_fft:
        pcm = np.pad(pcm, (0, n_fft - pcm.size))
    x = np.pad(pcm, (n_fft // 2, n_fft // 2))
    n_frames = 1 + (x.size - n_fft) // hop
    idx = np.arange(n_fft)[None, :] + hop * np.arange(n_frames)[:, None]
    win = np.hanning(n_fft)
    spec = np.abs(np.fft.rfft(x[idx] * win, axis=1))
    logs = np.log1p(1000.0 * spec)
    flux = np.maximum(0.0, np.diff(logs, axis=0)).sum(axis=1)
    env = np.concatenate([[0.0], flux])
    # Quitar tendencia lenta (media móvil ~0.5 s) y rectificar.
    fps = sr / hop
    k = max(1, int(0.5 * fps))
    trend = np.convolve(env, np.ones(k) / k, mode="same")
    env = np.maximum(0.0, env - trend)
    m = env.max()
    return (env / m if m > 0 else env), fps


def _interp(env: np.ndarray, pos: np.ndarray) -> np.ndarray:
    return np.interp(pos, np.arange(env.size), env, left=0.0, right=0.0)


def estimate_tempo(pcm: np.ndarray, sr: int = ANALYSIS_SR, bpm_min: float = BPM_MIN,
                   bpm_max: float = BPM_MAX) -> tuple[float, float]:
    """(bpm, offset_s). offset_s = primer beat de la rejilla con golpe claro (≥ 0)."""
    env, fps = onset_strength(pcm, sr)
    if env.max() <= 0 or env.size < 4 * fps:
        return 120.0, 0.0
    e = env - env.mean()
    # Autocorrelación vía FFT.
    n = 1 << int(np.ceil(np.log2(2 * e.size)))
    f = np.fft.rfft(e, n)
    ac = np.fft.irfft(f * np.conj(f), n)[: e.size]
    ac /= ac[0] if ac[0] > 0 else 1.0

    lag_min = 60.0 * fps / bpm_max
    lag_max = 60.0 * fps / bpm_min
    lags = np.arange(int(np.floor(lag_min)), int(np.ceil(lag_max)) + 1)
    lags = lags[(lags >= 1) & (2 * lags < ac.size)]

    def score(lag: np.ndarray) -> np.ndarray:
        # Peine: el beat también correlaciona a 2 y 3 periodos; un poco de mitad de periodo
        # desempata contra el "tempo mitad". Prior log-normal suave alrededor de 120 BPM.
        s = _interp(ac, lag) + 0.5 * _interp(ac, 2 * lag) + 0.25 * _interp(ac, 3 * lag)
        bpm = 60.0 * fps / lag
        prior = np.exp(-0.5 * (np.log2(bpm / 120.0) / 1.0) ** 2)
        return s * (0.5 + 0.5 * prior)

    s = score(lags.astype(float))
    best = float(lags[int(np.argmax(s))])
    # Nivel métrico: si a medio periodo hay casi tanta correlación, el tempo real es el doble
    # (p. ej. clics a 170 BPM que el peine confunde con 85).
    while best / 2 >= lag_min and _interp(ac, np.array([best / 2]))[0] >= 0.7 * _interp(ac, np.array([best]))[0]:
        best /= 2
    # Refinado fino del periodo (subtrama).
    fine = np.linspace(best - 1.5, best + 1.5, 301)
    fine = fine[(fine >= lag_min * 0.98) & (fine <= lag_max * 1.02)]
    best = float(fine[int(np.argmax(score(fine)))])

    # Refinado conjunto periodo/fase con un tren de pulsos sobre toda la canción.
    period, phase = _fit_grid(env, best)
    bpm = 60.0 * fps / period
    offset = _first_beat(env, period, phase) / fps + ONSET_LAG_S
    return float(round(bpm, 2)), float(max(0.0, round(offset, 3)))


def _fit_grid(env: np.ndarray, period0: float) -> tuple[float, float]:
    best = (-1.0, period0, 0.0)
    for period in np.linspace(period0 * 0.99, period0 * 1.01, 81):
        nb = int((env.size - 1) / period)
        if nb < 2:
            continue
        phases = np.linspace(0.0, period, int(np.ceil(period * 4)), endpoint=False)
        pos = phases[:, None] + period * np.arange(nb)[None, :]
        val = _interp(env, pos).sum(axis=1) / nb
        i = int(np.argmax(val))
        if val[i] > best[0]:
            best = (float(val[i]), float(period), float(phases[i]))
    return best[1], best[2]


def _first_beat(env: np.ndarray, period: float, phase: float) -> float:
    nb = int((env.size - 1 - phase) / period) + 1
    pos = phase + period * np.arange(nb)
    # Máximo local en ±1 trama para no perder golpes por redondeo.
    vals = np.maximum.reduce([_interp(env, pos + d) for d in (-1.0, 0.0, 1.0)])
    thr = 0.5 * float(np.median(vals[vals > 0])) if np.any(vals > 0) else 0.0
    for p, v in zip(pos, vals):
        if v >= thr and v > 0:
            return float(p)
    return float(phase)


# ------------------------------------------------------------------ choreo

def skeleton_steps(bpm: float, offset_s: float, duration_s: float, every_beats: int = 2,
                   tail_s: float = 1.0) -> list[dict]:
    """Pasos accent cada `every_beats` beats, alternando manos, marcados para editar."""
    beat = 60.0 / bpm
    dur = round(beat / 2, 3)
    steps = []
    k = 0
    t = offset_s
    while t + dur <= duration_s - tail_s:
        limb = "right_hand" if k % 2 == 0 else "left_hand"
        steps.append({"t": round(t, 3), "dur": dur, "limbs": [limb], "icon": "edit_me",
                      "kind": "accent", "auto": True})
        k += 1
        t = offset_s + k * every_beats * beat
    return steps


def make_choreo(title: str, artist: str, bpm: float, offset_s: float, duration_s: float,
                steps: list[dict], video: str | None = "video.ogv", imu: str | None = None) -> dict:
    return {
        "format": "jd-choreo",
        "version": 1,
        "song": {"title": title, "artist": artist, "audio": "audio.ogg", "video": video,
                 "bpm": bpm, "offset_s": offset_s, "duration_s": round(duration_s, 3)},
        "steps": steps,
        "references": {"imu": imu} if imu else {},
    }


def utc_now_iso() -> str:
    import datetime as _dt
    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def tools_dir() -> Path:
    return Path(__file__).resolve().parent
