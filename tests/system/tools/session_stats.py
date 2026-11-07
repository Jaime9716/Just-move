#!/usr/bin/env python3
"""P2 · Estadísticas de una o varias sesiones grabadas (.jsonl, contracts/session_format.md).

Por dispositivo (src): número de muestras, duración, tasa (Hz), huecos > 50 ms, pérdidas estimadas,
t monotónico, rangos de acelerómetro y giroscopio, |a| en reposo, y APROBADO/SUSPENDIDO.

Ejemplos:
  python tests/system/tools/session_stats.py data/sessions/real/p2_reposo.jsonl
  python tests/system/tools/session_stats.py data/sessions/real/*.jsonl --json
  python tests/system/tools/session_stats.py s.jsonl --rate 200 --max-gaps 2

Código de salida: 0 = todo aprobado, 1 = algún suspenso, 2 = error de lectura.
"""
from __future__ import annotations

import argparse
import glob
import json
import math
import sys
from pathlib import Path

import numpy as np

A_LIMIT = 16 * 9.81 * 1.05   # m/s²: por encima, valor imposible para un móvil (±16 g)
G_LIMIT = 40.0               # rad/s: por encima, valor imposible (≈ 2300 °/s)


class SessionError(Exception):
    pass


def load_session(path: str | Path):
    """Devuelve (cabecera, {src: dict(t, a, g, limb)}). Respeta el contrato (cabecera = 1ª línea útil)."""
    header = None
    per: dict[int, dict] = {}
    with open(path, encoding="utf-8") as f:
        for lineno, line in enumerate(f, 1):
            s = line.strip()
            if not s or s.startswith("#"):
                continue
            try:
                obj = json.loads(s)
            except json.JSONDecodeError as e:
                raise SessionError(f"{path}:{lineno}: JSON inválido ({e.msg})") from None
            if header is None:
                if obj.get("format") != "jd-session" or obj.get("version") != 1:
                    raise SessionError(f"{path}:{lineno}: cabecera sin format=jd-session / version=1")
                header = obj
                continue
            if "t" not in obj:
                raise SessionError(f"{path}:{lineno}: muestra sin 't'")
            src = int(obj.get("src", 0))
            d = per.setdefault(src, {"t": [], "a": [], "g": [], "limb": obj.get("limb")})
            d["t"].append(float(obj["t"]))
            a = obj.get("a")
            g = obj.get("g")
            d["a"].append(a if a is not None else [math.nan] * 3)
            d["g"].append(g if g is not None else [math.nan] * 3)
    if header is None:
        raise SessionError(f"{path}: archivo vacío o sin cabecera")
    for d in per.values():
        d["t"] = np.asarray(d["t"], float)
        d["a"] = np.asarray(d["a"], float).reshape(-1, 3)
        d["g"] = np.asarray(d["g"], float).reshape(-1, 3)
    return header, per


def device_stats(t, a, g, *, rate, gap_s, rest_window_s=1.0):
    n = len(t)
    out: dict = {"n": int(n)}
    if n < 2:
        out.update(duration_s=0.0, rate_hz=0.0)
        return out
    dt = np.diff(t)
    dur = float(t[-1] - t[0])
    out["duration_s"] = round(dur, 3)
    out["rate_hz"] = round((n - 1) / dur, 2) if dur > 0 else 0.0
    out["non_monotonic"] = int(np.sum(dt <= 0))
    out["dt_median_ms"] = round(float(np.median(dt)) * 1e3, 3)
    out["dt_p99_ms"] = round(float(np.percentile(dt, 99)) * 1e3, 3)
    out["dt_max_ms"] = round(float(dt.max()) * 1e3, 1)
    big = np.nonzero(dt > gap_s)[0]
    out["gaps"] = int(len(big))
    out["gap_list"] = [{"t": round(float(t[i]), 3), "ms": round(float(dt[i]) * 1e3, 1)}
                       for i in big[np.argsort(-dt[big])][:10]]
    nominal = 1.0 / rate
    lost = np.clip(np.round(dt / nominal) - 1, 0, None)
    lost = lost[dt > 1.5 * nominal]
    out["lost_est"] = int(lost.sum())
    out["loss_pct"] = round(100.0 * lost.sum() / (lost.sum() + n), 3)
    has_a = ~np.isnan(a).any(axis=1)
    has_g = ~np.isnan(g).any(axis=1)
    out["with_accel_pct"] = round(100.0 * has_a.mean(), 2)
    out["with_gyro_pct"] = round(100.0 * has_g.mean(), 2)
    if has_a.any():
        aa = a[has_a]
        mag = np.linalg.norm(aa, axis=1)
        out["accel_min"] = [round(float(x), 2) for x in aa.min(axis=0)]
        out["accel_max"] = [round(float(x), 2) for x in aa.max(axis=0)]
        out["accel_mag_max"] = round(float(mag.max()), 2)
        out["accel_out_of_range"] = int(np.sum(np.abs(aa) > A_LIMIT))
        # |a| en reposo: ventana de 1 s con menor desviación típica (debería ser ≈ 9.81)
        ta = t[has_a]
        k = max(int(rest_window_s * out["rate_hz"]), 5)
        if len(mag) >= k:
            cs = np.cumsum(np.insert(mag, 0, 0.0))
            cs2 = np.cumsum(np.insert(mag * mag, 0, 0.0))
            m = (cs[k:] - cs[:-k]) / k
            var = np.maximum((cs2[k:] - cs2[:-k]) / k - m * m, 0.0)
            i = int(np.argmin(var))
            out["rest_t"] = round(float(ta[i]), 2)
            out["rest_mag"] = round(float(m[i]), 3)
            out["rest_std"] = round(float(math.sqrt(var[i])), 3)
        out["motion_std"] = round(float(np.std(mag)), 3)
    if has_g.any():
        gg = g[has_g]
        out["gyro_min"] = [round(float(x), 2) for x in gg.min(axis=0)]
        out["gyro_max"] = [round(float(x), 2) for x in gg.max(axis=0)]
        out["gyro_out_of_range"] = int(np.sum(np.abs(gg) > G_LIMIT))
    nan_or_inf = int(np.sum(~np.isfinite(a[has_a]))) + int(np.sum(~np.isfinite(g[has_g])))
    out["non_finite"] = nan_or_inf
    return out


def judge(st: dict, *, rate, rate_tol, max_gaps, max_loss_pct):
    """Lista de (criterio, ok, detalle)."""
    checks = []
    if st["n"] < 2:
        return [("muestras", False, f"{st['n']} muestras")]
    lo, hi = rate * (1 - rate_tol), rate * (1 + rate_tol)
    checks.append(("tasa", lo <= st["rate_hz"] <= hi, f"{st['rate_hz']} Hz (esperado {lo:.0f}–{hi:.0f})"))
    checks.append(("huecos", st["gaps"] <= max_gaps, f"{st['gaps']} huecos > umbral (máx. {max_gaps}); peor {st['dt_max_ms']} ms"))
    checks.append(("pérdidas", st["loss_pct"] <= max_loss_pct, f"{st['loss_pct']} % (máx. {max_loss_pct} %)"))
    checks.append(("t monotónico", st["non_monotonic"] == 0, f"{st['non_monotonic']} retrocesos/repeticiones"))
    oor = st.get("accel_out_of_range", 0) + st.get("gyro_out_of_range", 0) + st.get("non_finite", 0)
    checks.append(("rangos", oor == 0, f"{oor} valores imposibles o no finitos"))
    return checks


def analyze(path, args):
    header, per = load_session(path)
    devices = {int(d.get("source_id", 0)): d for d in header.get("devices", [])}
    res = {"file": str(path), "time_base": header.get("time_base", "song"), "song_id": header.get("song_id"),
           "note": header.get("note", ""), "recorded_at": header.get("recorded_at"), "devices": []}
    all_ok = True
    for src in sorted(per):
        d = per[src]
        st = device_stats(d["t"], d["a"], d["g"], rate=args.rate, gap_s=args.gap_ms / 1e3)
        hdr = devices.get(src, {})
        st.update(src=src, name=hdr.get("name", "?"), limb=d["limb"] or hdr.get("limb", "?"))
        checks = judge(st, rate=args.rate, rate_tol=args.rate_tol, max_gaps=args.max_gaps,
                       max_loss_pct=args.max_loss_pct)
        st["checks"] = [{"name": c, "ok": bool(ok), "detail": det} for c, ok, det in checks]
        st["pass"] = all(ok for _, ok, _ in checks)
        if "rest_mag" in st and abs(st["rest_mag"] - 9.81) > 0.6:
            st.setdefault("warnings", []).append(
                f"|a| en el tramo más quieto = {st['rest_mag']} m/s² (≈ 9.81 esperado): ¿acelerómetro sin gravedad o mal escalado?")
        all_ok &= st["pass"]
        res["devices"].append(st)
    for src, hdr in devices.items():
        if src not in per:
            res.setdefault("warnings", []).append(f"dispositivo {src} ({hdr.get('name')}) en la cabecera sin muestras")
    if not per:
        all_ok = False
    res["pass"] = bool(all_ok)
    return res


def print_human(r):
    print(f"== {r['file']}")
    print(f"   base de tiempo: {r['time_base']} · canción: {r['song_id']} · nota: {r['note']!r}")
    for w in r.get("warnings", []):
        print(f"   AVISO: {w}")
    if not r["devices"]:
        print("   SUSPENDIDO: no hay muestras")
    for d in r["devices"]:
        print(f"   -- src {d['src']} · {d['name']} · {d['limb']}: {d['n']} muestras en {d.get('duration_s', 0)} s")
        if d["n"] >= 2:
            print(f"      Δt mediana {d['dt_median_ms']} ms · p99 {d['dt_p99_ms']} ms · máx {d['dt_max_ms']} ms"
                  f" · pérdidas estimadas {d['lost_est']} ({d['loss_pct']} %)")
            print(f"      con acel {d['with_accel_pct']} % · con giro {d['with_gyro_pct']} %")
            if "accel_min" in d:
                print(f"      acel min {d['accel_min']} máx {d['accel_max']} · |a| máx {d['accel_mag_max']}"
                      f" · reposo |a|={d.get('rest_mag')}±{d.get('rest_std')} (t={d.get('rest_t')} s)")
            if "gyro_min" in d:
                print(f"      giro min {d['gyro_min']} máx {d['gyro_max']}")
            for g in d["gap_list"][:5]:
                print(f"      hueco de {g['ms']} ms en t={g['t']} s")
        for c in d.get("checks", []):
            print(f"      [{'OK ' if c['ok'] else 'MAL'}] {c['name']}: {c['detail']}")
        for w in d.get("warnings", []):
            print(f"      AVISO: {w}")
        print(f"      => {'APROBADO' if d['pass'] else 'SUSPENDIDO'}")
    print(f"   RESULTADO: {'APROBADO' if r['pass'] else 'SUSPENDIDO'}")


def main(argv=None):
    ap = argparse.ArgumentParser(description="P2: tasa, huecos, pérdidas, monotonía y rangos de sesiones .jsonl")
    ap.add_argument("files", nargs="+", help="sesiones .jsonl (se aceptan comodines, también en Windows)")
    ap.add_argument("--rate", type=float, default=200.0, help="tasa esperada en Hz (200)")
    ap.add_argument("--rate-tol", type=float, default=0.05, help="tolerancia relativa de la tasa (0.05 = ±5 %%)")
    ap.add_argument("--gap-ms", type=float, default=50.0, help="umbral de hueco en ms (50)")
    ap.add_argument("--max-gaps", type=int, default=0, help="huecos > umbral permitidos por dispositivo (0)")
    ap.add_argument("--max-loss-pct", type=float, default=1.0, help="pérdidas estimadas permitidas en %% (1)")
    ap.add_argument("--json", action="store_true", help="salida JSON")
    args = ap.parse_args(argv)

    files = []
    for f in args.files:
        files.extend(sorted(glob.glob(f)) or [f])
    results, err = [], False
    for f in files:
        try:
            results.append(analyze(f, args))
        except (OSError, SessionError) as e:
            err = True
            results.append({"file": f, "error": str(e), "pass": False})
    if args.json:
        print(json.dumps(results if len(results) > 1 else results[0], ensure_ascii=False, indent=2))
    else:
        for r in results:
            if "error" in r:
                print(f"== {r['file']}\n   ERROR: {r['error']}")
            else:
                print_human(r)
    if err:
        return 2
    return 0 if all(r["pass"] for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())
