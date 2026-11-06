#!/usr/bin/env python3
"""Grabador de coreografías (F11): de una sesión bailada con el móvil a referencia IMU + pasos.

    python tools/choreo_record.py <sesion.jsonl> --song data/songs/<id> --offset S \
        [--merge] [--grid 1] [--min-peak 8] [--max-dev 0.35] [--limbs right_hand,left_foot] [--dry-run]

1. Lee la sesión grabada con `jd_record` (contracts/session_format.md, time_base "mono" o "song").
2. Pasa a tiempo de canción: ``t_song = t + offset`` (offset = tiempo de canción en el instante
   t = 0 de la grabación; negativo si empezaste a grabar antes de que sonara la música).
3. Guarda ``ref_imu.jsonl`` (time_base "song") en el paquete y pone ``references.imu``.
4. Detecta picos de aceleración lineal por extremidad (umbrales del juez v1: ≥ PEAK_MIN, ≥ 1.5 ×
   mediana del último segundo, separación ≥ 0.15 s), los cuantiza a la rejilla de beats
   (``--grid`` en beats: 1 = negras, 0.5 = corcheas) y propone pasos ``accent`` con icon
   ``edit_me`` y ``"auto": true``. Picos a más de ``--max-dev`` (fracción de la rejilla) de la
   rejilla se descartan.
5. Sin ``--merge`` los pasos propuestos reemplazan a los existentes; con ``--merge`` se conservan
   los existentes y solo se añaden extremidades nuevas. Siempre se guarda ``choreo.json.bak``.

Nota: la aceleración lineal se aproxima restando una estimación de gravedad de paso bajo (no la
orientación completa del juez v1); basta para proponer pasos que luego revisas a mano.
"""
from __future__ import annotations

import argparse
import json
import math
import shutil
import sys
from pathlib import Path

import numpy as np

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))

import choreo_validate  # noqa: E402

LIMB_ORDER = ("left_hand", "right_hand", "left_foot", "right_foot")
G = 9.80665
LPF_CUTOFF_HZ = 20.0
GRAVITY_TAU_S = 0.4
PEAK_MIN = 8.0
PEAK_REL = 1.5
PEAK_MIN_SEP_S = 0.150


class RecordError(RuntimeError):
    pass


# ------------------------------------------------------------------ sesiones

def load_session(path: Path) -> tuple[dict, list[dict]]:
    """(cabecera, muestras) de un .jsonl de sesión. Errores con 'archivo:línea:'."""
    header = None
    samples = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as e:
        raise RecordError(f"no se puede abrir {path}: {e}") from None
    for n, line in enumerate(lines, 1):
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        try:
            obj = json.loads(s)
        except json.JSONDecodeError as e:
            raise RecordError(f"{path}:{n}: JSON no válido ({e.msg})") from None
        if header is None:
            if obj.get("format") != "jd-session":
                raise RecordError(f"{path}:{n}: la primera línea debe ser la cabecera jd-session")
            if obj.get("version") != 1:
                raise RecordError(f"{path}:{n}: versión de sesión no soportada {obj.get('version')}")
            if obj.get("time_base") not in ("song", "mono"):
                raise RecordError(f"{path}:{n}: time_base debe ser \"song\" o \"mono\"")
            header = obj
            continue
        if not isinstance(obj.get("t"), (int, float)) or isinstance(obj.get("t"), bool):
            raise RecordError(f"{path}:{n}: falta t numérico")
        a = obj.get("a")
        if a is not None and (not isinstance(a, list) or len(a) != 3):
            raise RecordError(f"{path}:{n}: 'a' debe ser [x,y,z]")
        samples.append(obj)
    if header is None:
        raise RecordError(f"{path}: sesión vacía (sin cabecera)")
    return header, samples


def sample_limb(s: dict, devices: dict) -> str:
    limb = s.get("limb")
    if limb in LIMB_ORDER:
        return limb
    return devices.get(s.get("src"), "none")


def write_ref_session(path: Path, header: dict, samples: list[dict], song_id: str, offset: float,
                      duration: float, source_name: str) -> int:
    out_header = {
        "format": "jd-session", "version": 1, "time_base": "song", "song_id": song_id,
        "recorded_at": header.get("recorded_at"),
        "note": (f"referencia de {source_name} (time_base {header.get('time_base')}, offset {offset:+.3f} s)"
                 + (f"; {header['note']}" if header.get("note") else "")),
        "devices": header.get("devices", []),
    }
    n = 0
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(out_header, ensure_ascii=False) + "\n")
        for s in samples:
            t = s["t"] + offset
            if t < 0 or t > duration:
                continue
            o = {"t": round(t, 6)}
            o.update({k: v for k, v in s.items() if k != "t"})
            f.write(json.dumps(o, ensure_ascii=False, separators=(",", ":")) + "\n")
            n += 1
    tmp.replace(path)
    return n


# ------------------------------------------------------------------ picos

def linear_accel_magnitude(t: np.ndarray, a: np.ndarray) -> np.ndarray:
    """|a_filtrada − gravedad_estimada| por muestra (aprox. a |lin_accel| del juez)."""
    out = np.zeros(t.size)
    y = a[0].copy()
    g = a[0].copy()
    if np.linalg.norm(g) < 1e-6:
        g = np.array([0.0, G, 0.0])
    for i in range(t.size):
        dt = 0.005 if i == 0 else float(np.clip(t[i] - t[i - 1], 0.0005, 0.05))
        k = dt / (dt + 1.0 / (2 * math.pi * LPF_CUTOFF_HZ))
        y += k * (a[i] - y)
        kg = dt / (dt + GRAVITY_TAU_S)
        g += kg * (y - g)
        gn = np.linalg.norm(g)
        g_unit = g / gn if gn > 1e-6 else np.array([0.0, 1.0, 0.0])
        out[i] = np.linalg.norm(y - G * g_unit)
    return out


def detect_peaks(t: np.ndarray, mag: np.ndarray, min_peak: float = PEAK_MIN) -> list[tuple[float, float]]:
    """[(t, valor)] con las reglas de picos del juez v1 (mediana del último 1 s sobre |lin|)."""
    cand = []
    for i in range(1, mag.size - 1):
        v = mag[i]
        if v < min_peak or v < mag[i - 1] or v < mag[i + 1] or v == mag[i - 1]:
            continue
        lo = np.searchsorted(t, t[i] - 1.0)
        med = float(np.median(mag[lo:i + 1]))
        if v < PEAK_REL * med:
            continue
        cand.append((float(t[i]), float(v)))
    out: list[tuple[float, float]] = []
    for tp, v in cand:
        if out and tp - out[-1][0] < PEAK_MIN_SEP_S:
            if v > out[-1][1]:
                out[-1] = (tp, v)
            continue
        out.append((tp, v))
    return out


def peaks_by_limb(samples: list[dict], devices: dict, offset: float, min_peak: float) -> dict:
    per: dict[str, list] = {}
    for s in samples:
        if s.get("a") is None:
            continue
        per.setdefault(sample_limb(s, devices), []).append((s["t"] + offset, s["a"]))
    res = {}
    for limb, lst in per.items():
        if limb not in LIMB_ORDER or len(lst) < 3:
            continue
        lst.sort(key=lambda x: x[0])
        t = np.array([x[0] for x in lst], float)
        a = np.array([x[1] for x in lst], float)
        res[limb] = detect_peaks(t, linear_accel_magnitude(t, a), min_peak)
    return res


# ------------------------------------------------------------------ pasos

def propose_steps(peaks: dict, bpm: float, offset_s: float, duration: float, grid_beats: float,
                  max_dev: float) -> tuple[list[dict], dict]:
    beat = 60.0 / bpm
    g = beat * grid_beats
    dur = round(beat / 2, 3)
    slots: dict[float, set] = {}
    stats = {}
    for limb, lst in peaks.items():
        kept = 0
        for tp, _v in lst:
            q = offset_s + round((tp - offset_s) / g) * g
            if abs(tp - q) > max_dev * g or q < 0 or q + dur > duration:
                continue
            slots.setdefault(round(q, 3), set()).add(limb)
            kept += 1
        stats[limb] = (len(lst), kept)
    steps = [{"t": q, "dur": dur, "limbs": [l for l in LIMB_ORDER if l in ls], "icon": "edit_me",
              "kind": "accent", "auto": True} for q, ls in sorted(slots.items())]
    return steps, stats


def merge_steps(existing: list[dict], proposed: list[dict], tol: float) -> tuple[list[dict], int]:
    out = list(existing)
    added = 0
    for p in proposed:
        covered = {l for e in existing if abs(e["t"] - p["t"]) < tol for l in e.get("limbs", [])}
        limbs = [l for l in p["limbs"] if l not in covered]
        if limbs:
            out.append(dict(p, limbs=limbs))
            added += 1
    out.sort(key=lambda s: s["t"])  # estable: los existentes van antes a igual t
    return out, added


# ------------------------------------------------------------------ principal

def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Sesión bailada → ref_imu.jsonl + pasos propuestos.")
    ap.add_argument("session", type=Path, help="sesión .jsonl grabada con jd_record")
    ap.add_argument("--song", type=Path, required=True, help="carpeta del paquete (con choreo.json)")
    ap.add_argument("--offset", type=float, default=None,
                    help="tiempo de canción en t=0 de la sesión (obligatorio si time_base es mono)")
    ap.add_argument("--merge", action="store_true", help="conservar los pasos existentes")
    ap.add_argument("--grid", type=float, default=1.0, help="rejilla en beats (1, 0.5, 2…)")
    ap.add_argument("--min-peak", type=float, default=PEAK_MIN, help="pico mínimo en m/s²")
    ap.add_argument("--max-dev", type=float, default=0.35, help="desvío máx. del pico a la rejilla (fracción)")
    ap.add_argument("--limbs", default="", help="solo estas extremidades (separadas por comas)")
    ap.add_argument("--dry-run", action="store_true", help="no escribe nada; solo informa")
    a = ap.parse_args(argv)

    try:
        choreo_path = a.song / "choreo.json"
        choreo = choreo_validate.load(choreo_path)
        header, samples = load_session(a.session)
        if a.offset is None:
            if header["time_base"] == "mono":
                raise RecordError("la sesión está en time_base \"mono\": indica --offset "
                                  "(tiempo de canción cuando empezó la grabación)")
            a.offset = 0.0
        if a.grid <= 0:
            raise RecordError("--grid debe ser > 0")
        devices = {d.get("source_id"): d.get("limb") for d in header.get("devices", [])}
        song = choreo["song"]
        bpm, off, dur = float(song["bpm"]), float(song["offset_s"]), float(song["duration_s"])

        peaks = peaks_by_limb(samples, devices, a.offset, a.min_peak)
        if a.limbs:
            want = {x.strip() for x in a.limbs.split(",") if x.strip()}
            bad = want - set(LIMB_ORDER)
            if bad:
                raise RecordError(f"--limbs: extremidad desconocida {sorted(bad)}")
            peaks = {k: v for k, v in peaks.items() if k in want}
        proposed, stats = propose_steps(peaks, bpm, off, dur, a.grid, a.max_dev)
        if a.merge:
            steps, added = merge_steps(choreo["steps"], proposed, tol=60.0 / bpm * a.grid / 2)
        else:
            steps, added = proposed, len(proposed)

        for limb in LIMB_ORDER:
            if limb in stats:
                print(f"  {limb:10s}: {stats[limb][0]} picos, {stats[limb][1]} en la rejilla")
        if not stats:
            print("  (no hay muestras de acelerómetro con extremidad asignada)")
        print(f"Pasos: {len(choreo['steps'])} antes → {len(steps)} ({added} nuevos"
              f"{', conservando existentes' if a.merge else ', reemplazando'})")
        if a.dry_run:
            return 0

        new = dict(choreo)
        new["steps"] = steps
        new["references"] = dict(choreo.get("references") or {}, imu="ref_imu.jsonl")
        errs = choreo_validate.validate(new)
        if errs:
            raise choreo_validate.ChoreoError(errs)
        n = write_ref_session(a.song / "ref_imu.jsonl", header, samples, a.song.name, a.offset, dur, a.session.name)
        shutil.copy(choreo_path, choreo_path.with_suffix(".json.bak"))
        choreo_validate.save(choreo_path, new)
        print(f"Escrito {a.song / 'ref_imu.jsonl'} ({n} muestras) y {choreo_path} (copia en choreo.json.bak)")
    except (RecordError, choreo_validate.ChoreoError) as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
