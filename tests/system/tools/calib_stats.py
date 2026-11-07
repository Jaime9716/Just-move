#!/usr/bin/env python3
"""P4 · Repetibilidad de la calibración.

Acepta los desfases de N calibraciones seguidas, de tres formas (se pueden mezclar):
  - números en ms en la línea de órdenes:          calib_stats.py 32 35 29 31 34
  - un .txt/.csv con un valor por línea (ms) o una columna offset_ms / input_offset_ms / visual_offset_ms
  - copias de user://profile.cfg (JSON del juego; input_offset y visual_offset en segundos)

Criterio por defecto (docs/07, P4): el rango (máx − mín) < 10 ms.

Ejemplos:
  python tests/system/tools/calib_stats.py 32 35 29 31 34
  python tests/system/tools/calib_stats.py tests/system/results/p4_calib.csv --json
  python tests/system/tools/calib_stats.py perfil1.cfg perfil2.cfg perfil3.cfg --key visual_offset
"""
from __future__ import annotations

import argparse
import csv
import io
import json
import sys
from pathlib import Path

import numpy as np

COLS = ("offset_ms", "input_offset_ms", "visual_offset_ms", "ms", "desfase_ms", "offset")


def _num(s: str) -> float:
    return float(s.strip().replace(",", ".").replace("ms", "").replace("+", ""))


def values_from_file(path: Path, key: str) -> list[float]:
    text = path.read_text(encoding="utf-8-sig")
    st = text.lstrip()
    if st.startswith("{"):
        d = json.loads(text)
        if key not in d:
            raise ValueError(f"{path}: no tiene '{key}'")
        return [float(d[key]) * 1e3]
    lines = [ln for ln in text.splitlines() if ln.strip() and not ln.lstrip().startswith("#")]
    if not lines:
        return []
    first = lines[0]
    delim = ";" if first.count(";") > first.count(",") else ","
    try:
        _num(first.split(delim)[0])
        has_header = False
    except ValueError:
        has_header = True
    if has_header:
        rows = list(csv.DictReader(io.StringIO("\n".join(lines)), delimiter=delim))
        names = {n.strip().lower(): n for n in (rows[0].keys() if rows else [])}
        col = next((names[c] for c in (key + "_ms",) + COLS if c in names), None)
        if col is None:
            raise ValueError(f"{path}: no encuentro columna de desfase ({', '.join(COLS)})")
        return [_num(r[col]) for r in rows if (r.get(col) or "").strip()]
    return [_num(ln.split(delim)[0]) for ln in lines]


def stats(vals, max_range_ms):
    v = np.asarray(vals, float)
    r = {"n": int(len(v)), "values_ms": [round(float(x), 2) for x in v]}
    if len(v) == 0:
        r["pass"] = False
        return r
    r.update(mean_ms=round(float(v.mean()), 2), median_ms=round(float(np.median(v)), 2),
             std_ms=round(float(v.std(ddof=1)) if len(v) > 1 else 0.0, 2),
             min_ms=round(float(v.min()), 2), max_ms=round(float(v.max()), 2),
             range_ms=round(float(v.max() - v.min()), 2),
             max_dev_from_median_ms=round(float(np.max(np.abs(v - np.median(v)))), 2))
    if len(v) >= 3:
        # tendencia: ¿cambia sistemáticamente al repetir (cansancio, aprendizaje)?
        r["trend_ms_per_try"] = round(float(np.polyfit(np.arange(len(v)), v, 1)[0]), 2)
    r["criterion"] = f"rango < {max_range_ms} ms"
    r["pass"] = bool(len(v) >= 2 and r["range_ms"] < max_range_ms)
    if len(v) < 5:
        r["warning"] = f"solo {len(v)} calibraciones: el plan pide 5"
    return r


def main(argv=None):
    ap = argparse.ArgumentParser(description="P4: repetibilidad de calibración (rango, media, desviación)")
    ap.add_argument("inputs", nargs="+", help="valores en ms, archivos .txt/.csv o copias de profile.cfg")
    ap.add_argument("--key", default="input_offset", choices=["input_offset", "visual_offset", "audio_offset"],
                    help="campo a leer de profile.cfg (input_offset)")
    ap.add_argument("--max-range-ms", type=float, default=10.0, help="rango máximo aceptado (10 ms)")
    ap.add_argument("--json", action="store_true", help="salida JSON")
    a = ap.parse_args(argv)
    vals = []
    try:
        for x in a.inputs:
            p = Path(x)
            if p.is_file():
                vals.extend(values_from_file(p, a.key))
            else:
                vals.append(_num(x))
    except (ValueError, OSError, json.JSONDecodeError) as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2
    r = stats(vals, a.max_range_ms)
    if a.json:
        print(json.dumps(r, ensure_ascii=False, indent=2))
    else:
        print(f"Calibraciones: {r['n']} · valores (ms): {r['values_ms']}")
        if r["n"]:
            print(f"media {r['mean_ms']:+.1f} ms · mediana {r['median_ms']:+.1f} · desv. típica {r['std_ms']:.1f}"
                  f" · rango {r['range_ms']:.1f} ms (de {r['min_ms']:+.1f} a {r['max_ms']:+.1f})")
            if "trend_ms_per_try" in r:
                print(f"tendencia {r['trend_ms_per_try']:+.1f} ms por intento")
            if "warning" in r:
                print(f"AVISO: {r['warning']}")
            print(f"Criterio: {r['criterion']} => {'APROBADO' if r['pass'] else 'SUSPENDIDO'}")
    return 0 if r["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
