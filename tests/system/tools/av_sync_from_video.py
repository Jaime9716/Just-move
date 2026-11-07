#!/usr/bin/env python3
"""P3 · Sincronía audio–imagen medida desde un vídeo grabado con OTRO móvil.

Grabas la pantalla del PC (con destellos al ritmo, p. ej. la canción demo_metronomo) y el sonido
(clics) con la cámara de otro móvil, mejor en cámara lenta. Este script, con ffmpeg:
  1. extrae el brillo medio de cada fotograma (opcionalmente de una región, --roi) y su instante;
  2. extrae el audio y su envolvente;
  3. detecta destellos (subidas de brillo) y clics (subidas de envolvente);
  4. empareja cada destello con el clic más cercano y calcula el desfase
     desfase = t_destello − t_clic   (+ = la IMAGEN va TARDE respecto al sonido)
  al principio, al final, su deriva y su dispersión. Criterio (docs/07, P3): |desfase| ≤ 10 ms
  al principio y al final.

Cámara lenta: si el archivo se reproduce más lento que la realidad (p. ej. 240 fps guardados como
30 fps, el vídeo dura 8 veces más), pasa --slowdown 8. Si el móvil guarda los 240 fps con sus
instantes reales (el vídeo dura lo mismo que la escena), usa --slowdown 1 (por defecto).
--camera-offset-ms resta el desfase propio de la cámara si lo has medido (ver la guía de P3).

Ejemplos:
  python tests/system/tools/av_sync_from_video.py grabacion.mp4
  python tests/system/tools/av_sync_from_video.py grabacion.mp4 --roi 0.3,0.3,0.4,0.4 --json
  python tests/system/tools/av_sync_from_video.py slowmo.mp4 --slowdown 8 --verbose
"""
from __future__ import annotations

import argparse
import json
import math
import re
import shutil
import subprocess
import sys

import numpy as np

AUDIO_SR = 48000
_PTS_RE = re.compile(r"\bn:\s*(\d+)\s+pts:\s*(-?\d+)\s+pts_time:\s*(-?[0-9.eE+-]+)")


class AvSyncError(Exception):
    pass


def _ffmpeg() -> str:
    exe = shutil.which("ffmpeg")
    if not exe:
        raise AvSyncError("no encuentro ffmpeg en el PATH (Windows: winget install Gyan.FFmpeg)")
    return exe


def probe(path: str) -> dict:
    exe = shutil.which("ffprobe")
    if not exe:
        return {}
    p = subprocess.run([exe, "-v", "error", "-show_entries",
                        "stream=index,codec_type,codec_name,avg_frame_rate,r_frame_rate,sample_rate,width,height,duration",
                        "-of", "json", path], capture_output=True, text=True)
    if p.returncode != 0:
        raise AvSyncError(f"ffprobe no puede leer {path}: {p.stderr.strip()[-300:]}")
    return json.loads(p.stdout or "{}")


def _run(cmd: list[str]) -> tuple[bytes, str]:
    p = subprocess.run(cmd, capture_output=True)
    err = p.stderr.decode("utf-8", "replace")
    if p.returncode != 0:
        raise AvSyncError("ffmpeg falló: " + err.strip()[-500:])
    return p.stdout, err


def extract_brightness(path: str, roi=None, size: int = 16) -> tuple[np.ndarray, np.ndarray]:
    """(t_s, brillo 0..255) por fotograma, con el instante real de cada fotograma (pts)."""
    vf = []
    if roi:
        x, y, w, h = roi
        vf.append(f"crop=iw*{w}:ih*{h}:iw*{x}:ih*{y}")
    vf += [f"scale={size}:{size}:flags=area", "format=gray", "showinfo"]
    base = [_ffmpeg(), "-hide_banner", "-nostats", "-i", path, "-map", "0:v:0", "-an", "-vf", ",".join(vf)]
    try:
        raw, err = _run(base + ["-fps_mode", "passthrough", "-f", "rawvideo", "-"])
    except AvSyncError:
        raw, err = _run(base + ["-vsync", "0", "-f", "rawvideo", "-"])  # ffmpeg antiguo
    pts = [float(m.group(3)) for m in _PTS_RE.finditer(err)]
    fsz = size * size
    nfr = len(raw) // fsz
    if nfr == 0:
        raise AvSyncError("el vídeo no tiene fotogramas")
    frames = np.frombuffer(raw[: nfr * fsz], np.uint8).reshape(nfr, fsz)
    b = frames.mean(axis=1)
    if len(pts) != nfr:
        raise AvSyncError(f"ffmpeg dio {nfr} fotogramas pero {len(pts)} instantes; no puedo fecharlos")
    return np.asarray(pts, float), b


def extract_audio(path: str) -> tuple[float, np.ndarray]:
    """(t0_s, muestras mono float a AUDIO_SR) alineadas con los pts del vídeo."""
    cmd = [_ffmpeg(), "-hide_banner", "-nostats", "-i", path, "-map", "0:a:0", "-vn",
           "-af", f"ashowinfo,aresample={AUDIO_SR}", "-ac", "1", "-f", "s16le", "-acodec", "pcm_s16le", "-"]
    try:
        raw, err = _run(cmd)
    except AvSyncError as e:
        if "matches no streams" in str(e) or "Stream map" in str(e):
            raise AvSyncError("el vídeo no tiene audio (muchos móviles quitan el sonido en cámara lenta: "
                              "prueba otro modo, p. ej. 60 fps normal)") from None
        raise
    m = _PTS_RE.search(err)
    t0 = float(m.group(3)) if m else 0.0
    x = np.frombuffer(raw[: len(raw) // 2 * 2], "<i2").astype(np.float32) / 32768.0
    if len(x) == 0:
        raise AvSyncError("el audio está vacío")
    return t0, x


def detect_flashes(t, b, min_sep_s, min_contrast=8.0):
    """Instantes (interpolados) de las subidas de brillo por el 50 % entre fondo y destello."""
    lo, hi = np.percentile(b, 5), np.percentile(b, 95)
    if hi - lo < min_contrast:
        raise AvSyncError(f"no veo destellos (contraste {hi - lo:.1f} de 255); usa --roi sobre la zona que destella")
    # Detección en dos pasos: (1) candidato = subida por encima del 20 % global (con histéresis);
    # (2) umbral LOCAL al 50 % entre el fondo justo antes y el máximo justo después, para que los
    # destellos menos brillantes (otro color) no se fechen un fotograma tarde.
    trig = lo + 0.2 * (hi - lo)
    rearm = lo + 0.1 * (hi - lo)
    dt = float(np.median(np.diff(t))) if len(t) > 1 else 1.0
    k_post = max(2, int(round(min_sep_s / 3 / dt)))
    out, armed, last = [], True, -math.inf
    for i in range(1, len(b)):
        if not armed and b[i] < rearm:
            armed = True
        if not (armed and b[i - 1] < trig <= b[i] and t[i] - last >= min_sep_s):
            continue
        armed = False
        j0 = i - 1
        while j0 > 0 and b[j0 - 1] < b[j0] and b[j0] > rearm:   # retrocede hasta el pie de la subida
            j0 -= 1
        base = b[j0]
        top = float(b[i: i + k_post].max())
        thr = base + 0.5 * (top - base)
        j = j0 + 1
        while j < len(b) and b[j] < thr:
            j += 1
        if j >= len(b):
            continue
        frac = (thr - b[j - 1]) / (b[j] - b[j - 1]) if b[j] != b[j - 1] else 1.0
        prev_t = max(t[j - 1], t[j] - dt)   # fotogramas repetidos omitidos: no interpolar sobre el hueco
        tc = prev_t + frac * (t[j] - prev_t)
        out.append(tc)
        last = tc
    return np.asarray(out), {"low": round(float(lo), 1), "high": round(float(hi), 1)}


def detect_clicks(t0, x, min_sep_s, sr=AUDIO_SR):
    """Instantes de inicio de cada clic (envolvente del audio con paso alto)."""
    y = np.abs(np.diff(x, prepend=x[:1]))           # paso alto simple: resalta los transitorios
    k = max(int(sr * 0.0005), 1)                      # media móvil centrada de 0,5 ms
    env = np.convolve(y, np.ones(k) / k, mode="same")
    noise = float(np.median(env))
    peak = float(np.percentile(env, 99.95))
    if peak < noise * 4 or peak < 1e-4:
        raise AvSyncError("no oigo clics en el audio (¿volumen bajo o música encima?)")
    thr = noise + 0.25 * (peak - noise)
    above = np.nonzero(env > thr)[0]
    out, last = [], -math.inf
    sep = int(min_sep_s * sr)
    for i in above:
        if i - last >= sep:
            out.append(t0 + i / sr)
            last = i
    return np.asarray(out), {"noise": noise, "peak": peak}


def pair_events(flashes, clicks, max_pair_s):
    pairs = []
    if len(clicks) == 0:
        return pairs
    for f in flashes:
        j = int(np.argmin(np.abs(clicks - f)))
        if abs(clicks[j] - f) <= max_pair_s:
            pairs.append((float(clicks[j]), float(f)))
    # cada clic solo una vez (el destello más cercano)
    best: dict[float, tuple[float, float]] = {}
    for c, f in pairs:
        if c not in best or abs(f - c) < abs(best[c][1] - c):
            best[c] = (c, f)
    return sorted(best.values())


def analyze(path, *, roi=None, slowdown=1.0, camera_offset_ms=0.0, min_sep_s=0.15, max_pair_ms=None,
            tol_ms=10.0, edge_frac=0.25):
    info = probe(path)
    t, b = extract_brightness(path, roi)
    a_t0, audio = extract_audio(path)
    sep_file = min_sep_s * slowdown
    flashes, finfo = detect_flashes(t, b, sep_file)
    clicks, _ = detect_clicks(a_t0, audio, sep_file)
    if len(flashes) < 3 or len(clicks) < 3:
        raise AvSyncError(f"muy pocos eventos: {len(flashes)} destellos y {len(clicks)} clics")
    if max_pair_ms is None:
        period = float(np.median(np.diff(clicks)))
        max_pair_s = 0.4 * period
    else:
        max_pair_s = max_pair_ms / 1e3 * slowdown
    pairs = pair_events(flashes, clicks, max_pair_s)
    if len(pairs) < 3:
        raise AvSyncError(f"solo {len(pairs)} destellos tienen un clic cerca; revisa --roi o --max-pair-ms")
    c = np.array([p[0] for p in pairs]) / slowdown
    off = (np.array([p[1] for p in pairs]) - np.array([p[0] for p in pairs])) / slowdown * 1e3 - camera_offset_ms
    med = np.median(off)
    mad = np.median(np.abs(off - med))
    keep = np.abs(off - med) <= max(5.0, 4 * 1.4826 * mad)
    rejected = int((~keep).sum())
    c, off = c[keep], off[keep]
    n = len(off)
    ne = max(3, int(math.ceil(edge_frac * n)))
    ne = min(ne, n)
    start, end = float(np.median(off[:ne])), float(np.median(off[-ne:]))
    drift = float(np.polyfit(c - c[0], off, 1)[0] * 60.0) if n >= 3 and c[-1] > c[0] else 0.0
    dt_frames = np.diff(t)
    frame_ms = float(np.median(dt_frames)) / slowdown * 1e3 if len(dt_frames) else math.nan
    streams = [{k: s.get(k) for k in ("codec_type", "codec_name", "avg_frame_rate", "sample_rate", "width", "height")}
               for s in info.get("streams", [])]
    res = {
        "file": str(path), "streams": streams, "slowdown": slowdown, "camera_offset_ms": camera_offset_ms,
        "frames": int(len(t)), "real_frame_ms": round(frame_ms, 3),
        "real_fps": round(1e3 / frame_ms, 1) if frame_ms > 0 else None,
        "flashes": int(len(flashes)), "clicks": int(len(clicks)), "pairs": int(n), "rejected_outliers": rejected,
        "brightness": finfo,
        "offset_start_ms": round(start, 2), "offset_end_ms": round(end, 2),
        "offset_median_ms": round(float(np.median(off)), 2), "offset_std_ms": round(float(np.std(off)), 2),
        "drift_ms_per_min": round(drift, 2), "span_s": round(float(c[-1] - c[0]), 2),
        "edge_pairs": ne, "tolerance_ms": tol_ms,
        "pairs_detail": [{"t_click_s": round(float(ci), 4), "offset_ms": round(float(oi), 2)} for ci, oi in zip(c, off)],
    }
    res["pass"] = bool(abs(start) <= tol_ms and abs(end) <= tol_ms)
    warns = []
    if frame_ms > 10:
        warns.append(f"cada fotograma real dura {frame_ms:.1f} ms: cada destello se mide con ±{frame_ms/2:.0f} ms; "
                     "la mediana de muchos ayuda, pero mejor cámara lenta (≥ 120 fps)")
    if res["span_s"] < 20:
        warns.append(f"solo {res['span_s']} s entre el primer y el último clic: para ver la deriva graba ≥ 1 min")
    if rejected:
        warns.append(f"{rejected} parejas descartadas por atípicas")
    res["warnings"] = warns
    return res


def _roi(s: str):
    try:
        v = [float(x) for x in s.split(",")]
        assert len(v) == 4 and all(0 <= x <= 1 for x in v) and v[2] > 0 and v[3] > 0
        return v
    except (ValueError, AssertionError):
        raise argparse.ArgumentTypeError("--roi x,y,ancho,alto en fracciones 0..1 (p. ej. 0.25,0.25,0.5,0.5)")


def main(argv=None):
    ap = argparse.ArgumentParser(description="P3: desfase audio–imagen desde un vídeo de la pantalla (destellos + clics)")
    ap.add_argument("video", help="vídeo grabado con otro móvil (mp4, mov, mkv…)")
    ap.add_argument("--roi", type=_roi, help="región x,y,ancho,alto en fracciones del fotograma (por defecto, todo)")
    ap.add_argument("--slowdown", type=float, default=1.0, help="factor de cámara lenta del archivo (1 = tiempo real)")
    ap.add_argument("--camera-offset-ms", type=float, default=0.0, help="desfase propio de la cámara a restar (ms)")
    ap.add_argument("--min-sep-ms", type=float, default=150.0, help="separación mínima entre eventos, tiempo real (150)")
    ap.add_argument("--max-pair-ms", type=float, help="distancia máx. destello–clic (por defecto 40 %% del periodo)")
    ap.add_argument("--tol-ms", type=float, default=10.0, help="tolerancia del criterio (10 ms)")
    ap.add_argument("--verbose", action="store_true", help="lista cada pareja destello–clic")
    ap.add_argument("--json", action="store_true", help="salida JSON")
    a = ap.parse_args(argv)
    try:
        r = analyze(a.video, roi=a.roi, slowdown=a.slowdown, camera_offset_ms=a.camera_offset_ms,
                    min_sep_s=a.min_sep_ms / 1e3, max_pair_ms=a.max_pair_ms, tol_ms=a.tol_ms)
    except (AvSyncError, OSError) as e:
        if a.json:
            print(json.dumps({"file": a.video, "error": str(e), "pass": False}, ensure_ascii=False))
        else:
            print(f"ERROR: {e}", file=sys.stderr)
        return 2
    if a.json:
        if not a.verbose:
            r.pop("pairs_detail")
        print(json.dumps(r, ensure_ascii=False, indent=2))
    else:
        print(f"== {r['file']}")
        for s in r["streams"]:
            print(f"   pista {s['codec_type']}: {s['codec_name']} {s.get('avg_frame_rate') or ''} {s.get('sample_rate') or ''}")
        print(f"   {r['frames']} fotogramas · {r['real_fps']} fps reales (cámara lenta ×{r['slowdown']:g})")
        print(f"   {r['flashes']} destellos · {r['clicks']} clics · {r['pairs']} parejas en {r['span_s']} s")
        print(f"   desfase (+ = imagen tarde): principio {r['offset_start_ms']:+.1f} ms · final {r['offset_end_ms']:+.1f} ms"
              f" · mediana {r['offset_median_ms']:+.1f} ± {r['offset_std_ms']:.1f} ms · deriva {r['drift_ms_per_min']:+.2f} ms/min")
        if a.verbose:
            for p in r["pairs_detail"]:
                print(f"      t={p['t_click_s']:8.3f} s  {p['offset_ms']:+7.1f} ms")
        for w in r["warnings"]:
            print(f"   AVISO: {w}")
        print(f"   Criterio |desfase| ≤ {r['tolerance_ms']:g} ms al principio y al final => "
              f"{'APROBADO' if r['pass'] else 'SUSPENDIDO'}")
    return 0 if r["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
