"""B4 · Rasgos por extremidad: tramas y picos (contracts/judge_algorithm_v1.md §B4).

Precisiones (ver proto/ALGORITHM.md):
  * La trama k tiene centro t_c = k·FRAME_HOP_S (k entero, puede ser negativo) e incluye las muestras
    con |t − t_c| ≤ FRAME_WIN_S/2 + 1e-9. Tramas sin muestras no existen.
  * Mínimo local: m[i] > m[i−1] y m[i] ≥ m[i+1] (la primera y la última muestra nunca son pico).
  * PEAK_REL: umbral = peak_rel × mediana(energy de las tramas con t_c ∈ (t_pico − 1 s, t_pico]);
    sin tramas → solo PEAK_MIN. Mediana estándar (media de los dos centrales si el número es par).
  * Separación: candidatos en orden de tiempo; si uno está a < PEAK_MIN_SEP_S del último aceptado,
    lo sustituye solo si es estrictamente mayor.
  * is_peak va en la trama k = floor(t_pico/hop + 0.5); peak_t_song = t_pico (0.0 en tramas sin pico).
"""

import math

import numpy as np

from .config import resolve_config

EPS_T = 1e-9


def compute_frames(filtered, config=None):
    """filtered: lista de dict(t, lin, q) de UNA extremidad (orden de tiempo).

    -> (frames, peaks): frames = {k: dict(k, t, energy, peak, is_peak, peak_t_song, dir, quat)},
    peaks = lista de (t_pico, valor) aceptados."""
    cfg = resolve_config(config)
    hop, half = cfg["frame_hop_s"], cfg["frame_win_s"] / 2.0
    frames = {}
    if not filtered:
        return frames, []
    t = np.array([f["t"] for f in filtered], dtype=np.float64)
    lin = np.array([f["lin"] for f in filtered], dtype=np.float64)
    mag = np.sqrt(np.sum(lin * lin, axis=1))
    mag2 = mag * mag

    k_lo = math.ceil((t[0] - half) / hop) - 1
    k_hi = math.floor((t[-1] + half) / hop) + 1
    for k in range(k_lo, k_hi + 1):
        c = k * hop
        i0 = int(np.searchsorted(t, c - half - EPS_T, side="left"))
        i1 = int(np.searchsorted(t, c + half + EPS_T, side="right"))
        if i1 <= i0:
            continue
        # filtro exacto (searchsorted ya lo garantiza con t ordenado)
        n = i1 - i0
        energy = math.sqrt(float(np.sum(mag2[i0:i1])) / n)
        peak = float(np.max(mag[i0:i1]))
        m = lin[i0:i1].sum(axis=0) / n
        mn = math.sqrt(float(m[0] * m[0] + m[1] * m[1] + m[2] * m[2]))
        if mn < cfg["dir_min_norm"]:
            d = (0.0, 0.0, 0.0)
        else:
            d = (float(m[0] / mn), float(m[1] / mn), float(m[2] / mn))
        frames[k] = {"k": k, "t": c, "energy": energy, "peak": peak, "is_peak": False,
                     "peak_t_song": 0.0, "dir": d, "quat": tuple(filtered[i1 - 1]["q"])}

    peaks = detect_peaks(t, mag, frames, cfg)
    for pt, _ in peaks:
        k = math.floor(pt / hop + 0.5)
        fr = frames.get(k)
        if fr is not None:
            fr["is_peak"] = True
            fr["peak_t_song"] = pt
    return frames, peaks


def detect_peaks(t, mag, frames, cfg):
    hop = cfg["frame_hop_s"]
    rel_win = cfg["peak_rel_window_s"]
    keys = sorted(frames)
    centers = np.array([frames[k]["t"] for k in keys], dtype=np.float64)
    energies = np.array([frames[k]["energy"] for k in keys], dtype=np.float64)
    accepted = []
    n = len(mag)
    for i in range(1, n - 1):
        v = mag[i]
        if not (v > mag[i - 1] and v >= mag[i + 1]):
            continue
        if v < cfg["peak_min"]:
            continue
        ti = t[i]
        j0 = int(np.searchsorted(centers, ti - rel_win, side="right"))
        j1 = int(np.searchsorted(centers, ti, side="right"))
        if j1 > j0:
            if v < cfg["peak_rel"] * float(np.median(energies[j0:j1])):
                continue
        if accepted and ti - accepted[-1][0] < cfg["peak_min_sep_s"]:
            if v > accepted[-1][1]:
                accepted[-1] = (float(ti), float(v))
            continue
        accepted.append((float(ti), float(v)))
    return accepted


def compute_features(filtered_by_limb, config=None):
    """{limb: filtradas} -> {limb: {"frames": {k: trama}, "peaks": [(t, v)]}}."""
    cfg = resolve_config(config)
    out = {}
    for limb, filt in filtered_by_limb.items():
        frames, peaks = compute_frames(filt, cfg)
        out[limb] = {"frames": frames, "peaks": peaks}
    return out
