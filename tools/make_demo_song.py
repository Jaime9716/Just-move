#!/usr/bin/env python3
"""Genera el paquete de prueba data/songs/demo_metronomo/ (agente F · B6).

    python tools/make_demo_song.py [--out data/songs] [--seconds 30] [--bpm 120]

- audio.ogg  : clics de metrónomo (Vorbis 44.1 kHz estéreo); el 1.º de cada compás más agudo.
- video.ogv  : vídeo de prueba (Theora sin audio, 640x360, 30 fps): número del beat en el compás
               en grande (7 segmentos), destello en cada beat, compás y barra de progreso.
- choreo.json: coreografía válida (determinista, se versiona).
- meta.json  : {"source": "generated", ...}.

audio.ogg y video.ogv no se versionan (.gitignore); se regeneran con este script.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

import numpy as np

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))

import choreo_validate  # noqa: E402
import jd_media  # noqa: E402

SONG_ID = "demo_metronomo"
OFFSET_S = 1.0
SR = 44100
W, H, FPS = 640, 360, 30


# ------------------------------------------------------------------ audio

def click_audio(seconds: float, bpm: float, offset: float, sr: int = SR) -> np.ndarray:
    x = np.zeros(int(round(seconds * sr)))
    n = int(0.03 * sr)
    tt = np.arange(n) / sr
    env = np.exp(-tt / 0.006)
    beat = 60.0 / bpm
    k = 0
    while offset + k * beat < seconds - 0.05:
        f, amp = (1760.0, 0.9) if k % 4 == 0 else (880.0, 0.6)
        i = int(round((offset + k * beat) * sr))
        m = min(n, x.size - i)
        x[i:i + m] += amp * np.sin(2 * np.pi * f * tt[:m]) * env[:m]
        k += 1
    return x


def write_wav_stereo(path: Path, x: np.ndarray, sr: int = SR):
    pcm = (np.clip(x, -1, 1) * 32767).astype("<i2")
    st = np.repeat(pcm[:, None], 2, axis=1)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(sr)
        w.writeframes(st.tobytes())


# ------------------------------------------------------------------ vídeo

# Segmentos: a (arriba), b (arriba dcha), c (abajo dcha), d (abajo), e (abajo izq), f (arriba izq), g (centro)
DIGITS = {0: "abcdef", 1: "bc", 2: "abged", 3: "abgcd", 4: "fgbc", 5: "afgcd", 6: "afgedc",
          7: "abc", 8: "abcdefg", 9: "abcdfg"}


def draw_digit(img: np.ndarray, d: int, x0: int, y0: int, w: int, h: int, th: int, color):
    half = h // 2
    seg = {
        "a": (x0, y0, w, th), "d": (x0, y0 + h - th, w, th), "g": (x0, y0 + half - th // 2, w, th),
        "f": (x0, y0, th, half), "b": (x0 + w - th, y0, th, half),
        "e": (x0, y0 + half, th, h - half), "c": (x0 + w - th, y0 + half, th, h - half),
    }
    for s in DIGITS[d]:
        x, y, ww, hh = seg[s]
        img[y:y + hh, x:x + ww] = color


def draw_number(img, n: int, cx: int, y0: int, w: int, h: int, th: int, color, gap: int = 0):
    s = str(n)
    gap = gap or th
    total = len(s) * w + (len(s) - 1) * gap
    x = cx - total // 2
    for ch in s:
        draw_digit(img, int(ch), x, y0, w, h, th, color)
        x += w + gap


def video_frames(seconds: float, bpm: float, offset: float):
    beat = 60.0 / bpm
    n_frames = int(round(seconds * FPS))
    base = np.array([24, 26, 38], np.uint8)
    for i in range(n_frames):
        t = i / FPS
        img = np.empty((H, W, 3), np.uint8)
        img[:] = base
        if t >= offset:
            b = int(np.floor((t - offset) / beat + 1e-9))
            since = t - (offset + b * beat)
            flash = max(0.0, 1.0 - since / 0.25)
            down = b % 4 == 0
            fc = np.array([255, 170, 40] if down else [60, 160, 255], float)
            img[:] = (base * (1 - flash) + fc * flash * 0.6).astype(np.uint8)
            draw_number(img, b % 4 + 1, W // 2, 70, 90, 170, 18, (255, 255, 255))
            draw_number(img, b // 4 + 1, 60, 20, 24, 44, 6, (200, 200, 200))  # compás
            # cuatro puntos del compás
            for k in range(4):
                c = (255, 255, 255) if k <= b % 4 else (80, 80, 90)
                x = W // 2 - 90 + k * 60
                img[270:290, x:x + 20] = c
        else:
            # cuenta atrás antes del primer beat: barra que se llena
            img[170:190, 120:120 + int((W - 240) * t / offset)] = (120, 120, 140)
        img[H - 8:H, : int(W * t / seconds)] = (90, 200, 120)  # progreso
        yield img


def encode_video(ffmpeg: str, dst: Path, seconds: float, bpm: float, offset: float):
    cmd = [ffmpeg, "-y", "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}",
           "-r", str(FPS), "-i", "-", *jd_media.video_codec_args(ffmpeg), str(dst)]
    p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        for f in video_frames(seconds, bpm, offset):
            p.stdin.write(f.tobytes())
        p.stdin.close()
    except BrokenPipeError:
        pass
    err = p.stderr.read().decode("utf-8", "replace")
    if p.wait() != 0:
        raise RuntimeError(f"ffmpeg falló al codificar el vídeo:\n{err}")


# ------------------------------------------------------------------ coreografía

def demo_steps(bpm: float, offset: float, seconds: float) -> list[dict]:
    beat = 60.0 / bpm
    steps = []

    def at(b):
        return round(offset + b * beat, 3)

    def add(b, limbs, icon, kind="accent", dur_beats=0.5):
        steps.append({"t": at(b), "dur": round(dur_beats * beat, 3), "limbs": limbs, "icon": icon, "kind": kind})

    n_bars = int((seconds - offset - 1.0) / (4 * beat))
    for bar in range(1, n_bars):
        b0 = bar * 4
        if bar <= 4:  # golpes: derecha en 1, izquierda en 3
            add(b0, ["right_hand"], "punch_right")
            add(b0 + 2, ["left_hand"], "punch_left")
        elif bar <= 6:  # cada beat alternando
            for k in range(4):
                add(b0 + k, ["right_hand"] if k % 2 == 0 else ["left_hand"],
                    "punch_right" if k % 2 == 0 else "punch_left")
        elif bar <= 8:  # movimientos continuos de dos beats con las dos manos
            add(b0, ["left_hand", "right_hand"], "wave", "move", 2.0)
            add(b0 + 2, ["left_hand", "right_hand"], "wave", "move", 2.0)
        elif bar <= 10:  # pies
            add(b0, ["left_foot"], "stomp_left")
            add(b0 + 2, ["right_foot"], "stomp_right")
        elif bar < n_bars - 1:  # palmada + brazo
            add(b0, ["left_hand", "right_hand"], "clap")
            add(b0 + 1, ["right_hand"], "arm_circle", "move", 2.0)
        else:  # pose final con todo
            add(b0, ["left_hand", "right_hand", "left_foot", "right_foot"], "pose")
    return steps


def build_choreo(bpm: float, seconds: float, with_video: bool = True) -> dict:
    c = jd_media.make_choreo("Demo metrónomo", "Juego de Baile", bpm, OFFSET_S, seconds,
                             demo_steps(bpm, OFFSET_S, seconds), video="video.ogv" if with_video else None)
    return c


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Genera data/songs/demo_metronomo/")
    ap.add_argument("--out", type=Path, default=TOOLS.parent / "data" / "songs")
    ap.add_argument("--seconds", type=float, default=30.0)
    ap.add_argument("--bpm", type=float, default=120.0)
    ap.add_argument("--choreo-only", action="store_true", help="solo choreo.json y meta.json (sin ffmpeg)")
    a = ap.parse_args(argv)

    pkg = a.out / SONG_ID
    pkg.mkdir(parents=True, exist_ok=True)
    choreo = build_choreo(a.bpm, a.seconds)
    choreo_validate.save(pkg / "choreo.json", choreo)
    meta = {"source": "generated", "generator": "tools/make_demo_song.py", "personal_use_only": False,
            "title": "Demo metrónomo", "bpm": a.bpm, "offset_s": OFFSET_S, "duration_s": a.seconds,
            "note": "Clics sintéticos y vídeo con contador de beat; libre de derechos."}
    (pkg / "meta.json").write_text(json.dumps(meta, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if a.choreo_only:
        print(f"{pkg}: choreo.json y meta.json ({len(choreo['steps'])} pasos)")
        return 0
    try:
        ffmpeg, _ = jd_media.require_ffmpeg()
    except jd_media.ToolMissing as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory(prefix="jd_demo_") as td:
        wav = Path(td) / "clicks.wav"
        write_wav_stereo(wav, click_audio(a.seconds, a.bpm, OFFSET_S))
        jd_media.run([ffmpeg, "-y", "-v", "error", "-i", str(wav), *jd_media.audio_codec_args(ffmpeg),
                      str(pkg / "audio.ogg")], "codificar audio.ogg")
    encode_video(ffmpeg, pkg / "video.ogv", a.seconds, a.bpm, OFFSET_S)
    print(f"{pkg}: audio.ogg, video.ogv, choreo.json ({len(choreo['steps'])} pasos), meta.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
