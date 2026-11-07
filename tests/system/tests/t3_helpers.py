"""Utilidades comunes de los tests de tests/system/tools/ (datos sintéticos)."""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tests" / "system" / "tools"
sys.path.insert(0, str(TOOLS))


def run_tool(name: str, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(TOOLS / name), *map(str, args)], capture_output=True, text=True)


def write_session(path: Path, *, rate=200.0, seconds=10.0, devices=((1, "right_hand"),), gaps=(), backstep_at=None,
                  jitter_ms=0.3, seed=0, time_base="mono", drop_every=0, bad_accel_at=None):
    """Sesión JSONL sintética. gaps = [(t_s, dur_ms), ...] (se quitan muestras). Devuelve la ruta."""
    rng = np.random.default_rng(seed)
    head = {"format": "jd-session", "version": 1, "time_base": time_base, "song_id": None,
            "recorded_at": "2026-11-07T10:00:00Z", "note": "sintética T3",
            "devices": [{"source_id": s, "name": f"Sim {s}", "kind": "phone", "limb": l} for s, l in devices]}
    lines = [json.dumps(head)]
    n = int(seconds * rate)
    base = np.arange(n) / rate
    rows = []
    for src, limb in devices:
        t = base + rng.normal(0, jitter_ms / 1e3, n) * 0.3
        t = np.maximum.accumulate(t + np.arange(n) * 1e-9)
        keep = np.ones(n, bool)
        for g0, gd in gaps:
            keep &= ~((t > g0) & (t < g0 + gd / 1e3))
        if drop_every:
            keep[::drop_every] = False
        for i in np.nonzero(keep)[0]:
            ph = 2 * np.pi * 1.0 * t[i]
            a = [0.5 * np.sin(ph), 9.81 + (3 * np.sin(ph) if t[i] > seconds / 2 else 0.0), 0.2]
            if bad_accel_at is not None and abs(t[i] - bad_accel_at) < 0.5 / rate:
                a[0] = 900.0
            rows.append((float(t[i]), {"t": round(float(t[i]), 6), "src": src, "limb": limb,
                                       "a": [round(x, 4) for x in a], "g": [0.01, 0.02 * np.cos(ph), 0.0]}))
    rows.sort(key=lambda r: r[0])
    if backstep_at is not None:
        i = next(k for k, r in enumerate(rows) if r[0] > backstep_at)
        rows[i][1]["t"] = rows[i - 2][1]["t"]   # repite un instante anterior
    lines += [json.dumps(r[1]) for r in rows]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def make_av_video(path: Path, *, offset_s: float, seconds=20.0, period=0.523, fps=240, slowdown=1.0,
                  drift_s_per_s=0.0, flash_s=0.1, codec="h264"):
    """Vídeo sintético con ffmpeg: fotogramas que destellan y clics de audio.

    El destello k empieza en  1 + k·P + offset (+ deriva·t); el clic k en 1 + k·P (tiempo real).
    Con slowdown > 1 todo el contenido se estira (como una cámara lenta guardada a menos fps).
    """
    if not shutil.which("ffmpeg"):
        pytest.skip("ffmpeg no disponible")
    s = slowdown
    P, D, F, T0 = period * s, offset_s * s, flash_s * s, 1.0 * s
    click = 0.004 * s
    dur = seconds * s
    # tiempo real de la escena: T/s; el retraso del destello crece con la deriva
    lum = (f"if(gte(T,{T0}+{D})*lt(mod(T-{T0}-{D}-{drift_s_per_s}*(T-{T0}),{P}),{F}),230,20)")
    aud = f"if(gte(t,{T0})*lt(mod(t-{T0},{P}),{click}),0.8*sin(2*PI*{3000 / s}*t),0)"
    vcodec = ["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"] if codec == "h264" else \
             ["-c:v", "ffv1", "-c:a", "pcm_s16le"]
    cmd = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
           "-f", "lavfi", "-i", f"color=c=black:s=64x64:r={fps}:d={dur},format=gray,geq=lum='{lum}'",
           "-f", "lavfi", "-i", f"aevalsrc='{aud}':s=48000:d={dur}",
           *vcodec, "-shortest", str(path)]
    subprocess.run(cmd, check=True, capture_output=True)
    return path
