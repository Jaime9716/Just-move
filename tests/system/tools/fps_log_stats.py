#!/usr/bin/env python3
"""P9 · Rendimiento a partir del log de fps del juego (user://fps_log.csv, cabecera "t_s,fps").

El juego escribe una línea por fotograma: t_s (s desde que empieza la partida) y fps = 1/delta.
Se activa con la casilla "log de fps" del menú, con `-- --fps-log` o con JD_FPS_LOG=1, y se
SOBRESCRIBE en cada partida: cópialo al terminar la canción.
Windows: %APPDATA%\\Godot\\app_userdata\\Juego de Baile\\fps_log.csv
Linux:   ~/.local/share/godot/app_userdata/Juego de Baile/fps_log.csv

Criterios por defecto ("60 fps estables"), tras descartar los 2 primeros segundos:
  - fps medio ≥ 98 % del objetivo (≥ 58,8 con objetivo 60)
  - ≥ 99 % de los fotogramas a ≥ 90 % del objetivo (≥ 54 fps, fotograma ≤ 18,5 ms)
  - ningún fotograma de más de 50 ms (tirón visible)

Ejemplos:
  python tests/system/tools/fps_log_stats.py fps_log.csv
  python tests/system/tools/fps_log_stats.py fps_log.csv --target 60 --skip-s 2 --json
"""
from __future__ import annotations

import argparse
import json
import sys

import numpy as np


def load_fps_log(path):
    t, fps = [], []
    with open(path, encoding="utf-8-sig") as f:
        for i, line in enumerate(f, 1):
            s = line.strip()
            if not s or s.startswith("#"):
                continue
            if i == 1 and not s[0].isdigit() and not s[0] in "-.":
                cols = [c.strip().lower() for c in s.split(",")]
                if cols[:2] != ["t_s", "fps"]:
                    raise ValueError(f"{path}: cabecera inesperada {s!r} (esperada t_s,fps)")
                continue
            parts = s.split(",")
            if len(parts) < 2:
                raise ValueError(f"{path}:{i}: línea sin dos columnas")
            t.append(float(parts[0]))
            fps.append(float(parts[1]))
    return np.asarray(t, float), np.asarray(fps, float)


def stats(t, fps, *, target=60.0, skip_s=2.0, stall_ms=50.0, min_ok_frac=0.99, mean_frac=0.98, ok_frac=0.9):
    r = {"frames_total": int(len(t))}
    m = t >= (t[0] + skip_s if len(t) else 0)
    t, fps = t[m], fps[m]
    fps = fps[fps > 0]
    r["frames"] = int(len(fps))
    if len(fps) < 10:
        r["pass"] = False
        r["error"] = "muy pocos fotogramas en el log"
        return r
    ft = 1000.0 / fps
    dur = float(t[-1] - t[0])
    ok_thr = ok_frac * target
    r.update(
        duration_s=round(dur, 2),
        fps_mean=round(len(ft) / (ft.sum() / 1000.0), 2),         # fotogramas / tiempo real
        fps_median=round(float(np.median(fps)), 2),
        fps_min=round(float(fps.min()), 2),
        fps_p1=round(float(np.percentile(fps, 1)), 2),
        frame_ms_p99=round(float(np.percentile(ft, 99)), 2),
        frame_ms_max=round(float(ft.max()), 1),
        frames_below_pct=round(100.0 * float(np.mean(fps < ok_thr)), 3),
        stalls=int(np.sum(ft > stall_ms)),
        stall_list=[{"t_s": round(float(tt), 2), "ms": round(float(x), 1)}
                    for tt, x in sorted(zip(t[ft > stall_ms], ft[ft > stall_ms]), key=lambda p: -p[1])[:10]],
        target=target,
    )
    # peor segundo (fps medio por ventanas de 1 s)
    if dur >= 1:
        sec = np.floor(t - t[0]).astype(int)
        per = np.bincount(sec, weights=ft) / 1000.0
        cnt = np.bincount(sec)
        with np.errstate(divide="ignore", invalid="ignore"):
            fs = np.where(per > 0, cnt / per, np.nan)
        full = fs[:-1] if len(fs) > 1 else fs
        i = int(np.nanargmin(full))
        r["worst_second"] = {"t_s": float(i), "fps": round(float(full[i]), 1)}
    checks = [
        ("fps medio", r["fps_mean"] >= mean_frac * target, f"{r['fps_mean']} (≥ {mean_frac * target:.1f})"),
        ("fotogramas lentos", r["frames_below_pct"] <= 100 * (1 - min_ok_frac),
         f"{r['frames_below_pct']} % por debajo de {ok_thr:.0f} fps (≤ {100 * (1 - min_ok_frac):.0f} %)"),
        ("tirones", r["stalls"] == 0, f"{r['stalls']} fotogramas > {stall_ms:.0f} ms (peor {r['frame_ms_max']} ms)"),
    ]
    r["checks"] = [{"name": n, "ok": bool(ok), "detail": d} for n, ok, d in checks]
    r["pass"] = all(ok for _, ok, _ in checks)
    if r["fps_median"] > target * 1.3:
        r["warning"] = (f"mediana {r['fps_median']} fps: ¿vsync desactivado o monitor de alta frecuencia? "
                        f"Usa --target con la frecuencia del monitor")
    return r


def main(argv=None):
    ap = argparse.ArgumentParser(description="P9: resumen del log de fps (t_s,fps) del juego")
    ap.add_argument("csv", help="fps_log.csv copiado de user://")
    ap.add_argument("--target", type=float, default=60.0, help="fps objetivo (60)")
    ap.add_argument("--skip-s", type=float, default=2.0, help="segundos iniciales a descartar (2)")
    ap.add_argument("--stall-ms", type=float, default=50.0, help="fotograma más largo que esto = tirón (50)")
    ap.add_argument("--json", action="store_true", help="salida JSON")
    a = ap.parse_args(argv)
    try:
        t, fps = load_fps_log(a.csv)
    except (OSError, ValueError) as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2
    r = stats(t, fps, target=a.target, skip_s=a.skip_s, stall_ms=a.stall_ms)
    r["file"] = a.csv
    if a.json:
        print(json.dumps(r, ensure_ascii=False, indent=2))
    else:
        print(f"== {a.csv}")
        if "error" in r:
            print(f"   ERROR: {r['error']}")
        else:
            print(f"   {r['frames']} fotogramas en {r['duration_s']} s (descartados {a.skip_s:g} s iniciales)")
            print(f"   fps medio {r['fps_mean']} · mediana {r['fps_median']} · 1 % peor {r['fps_p1']} · mínimo {r['fps_min']}")
            print(f"   fotograma p99 {r['frame_ms_p99']} ms · máximo {r['frame_ms_max']} ms")
            if "worst_second" in r:
                print(f"   peor segundo: t={r['worst_second']['t_s']:.0f} s a {r['worst_second']['fps']} fps")
            for s in r["stall_list"][:5]:
                print(f"   tirón de {s['ms']} ms en t={s['t_s']} s")
            for c in r["checks"]:
                print(f"   [{'OK ' if c['ok'] else 'MAL'}] {c['name']}: {c['detail']}")
            if "warning" in r:
                print(f"   AVISO: {r['warning']}")
        print(f"   RESULTADO: {'APROBADO' if r['pass'] else 'SUSPENDIDO'}")
    return 0 if r["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
