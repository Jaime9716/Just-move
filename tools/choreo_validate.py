#!/usr/bin/env python3
"""Validador de choreo.json (contracts/song_package.md). Agente F · B6.

Mismas reglas y rutas JSON que `engine/jd_core/src/choreo.rs`:

    python tools/choreo_validate.py data/songs/*/choreo.json

Sale con 0 si todos son válidos; 1 si alguno falla (los errores se imprimen uno por línea,
precedidos por el archivo: ``data/songs/x/choreo.json: steps[3].t: debe ser >= steps[2].t``).

Como módulo: ``validate(obj) -> list[str]`` y ``load(path) -> dict`` (lanza ``ChoreoError``).
"""
from __future__ import annotations

import json
import math
import sys
from pathlib import Path

FORMAT = "jd-choreo"
VERSION = 1
MAX_BPM = 400.0
DURATION_EPS = 1e-6
MAX_ERRORS = 20
LIMBS = ("left_hand", "right_hand", "left_foot", "right_foot")
KINDS = ("accent", "move")


class ChoreoError(ValueError):
    """Coreografía inválida; ``errors`` es la lista de mensajes con ruta JSON."""

    def __init__(self, errors: list[str]):
        super().__init__("\n".join(errors))
        self.errors = errors


def _fnum(x) -> str:
    if isinstance(x, float):
        if math.isnan(x):
            return "NaN"
        if x == int(x) and abs(x) < 1e15:
            return str(int(x))
        return "%.9g" % x
    return str(x)


def _short(v) -> str:
    if v is None:
        return "null"
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        return _fnum(float(v))
    if isinstance(v, str):
        return '"%s"' % v[:40]
    if isinstance(v, list):
        return "una lista"
    if isinstance(v, dict):
        return "un objeto"
    return repr(v)


def _is_num(v) -> bool:
    return isinstance(v, (int, float)) and not isinstance(v, bool)


class _Errors:
    def __init__(self):
        self.list: list[str] = []
        self.extra = 0

    def push(self, path: str, msg: str):
        if len(self.list) < MAX_ERRORS:
            self.list.append(f"{path}: {msg}")
        else:
            self.extra += 1

    def result(self) -> list[str]:
        out = list(self.list)
        if self.extra:
            out.append(f"... y {self.extra} errores más")
        return out


def _req_str(e, obj, parent, key):
    path = f"{parent}.{key}"
    if key not in obj:
        e.push(path, "falta")
        return ""
    v = obj[key]
    if isinstance(v, str):
        return v
    e.push(path, f"debe ser texto (es {_short(v)})")
    return ""


def _opt_str(e, obj, parent, key, nullable):
    path = f"{parent}.{key}"
    if key not in obj:
        return ""
    v = obj[key]
    if isinstance(v, str):
        return v
    if v is None and nullable:
        return ""
    want = "texto o null" if nullable else "texto"
    e.push(path, f"debe ser {want} (es {_short(v)})")
    return ""


def _req_num(e, obj, parent, key):
    path = f"{parent}.{key}"
    if key not in obj:
        e.push(path, "falta")
        return math.nan
    v = obj[key]
    if _is_num(v):
        return float(v)
    e.push(path, f"debe ser un número (es {_short(v)})")
    return math.nan


def _check_rel_path(e, path, p, optional):
    if p == "":
        if not optional:
            e.push(path, "no puede estar vacío")
        return
    if p.startswith("/") or "\\" in p or ":" in p:
        e.push(path, f'debe ser una ruta relativa dentro del paquete (es "{p}")')
    elif ".." in p.split("/"):
        e.push(path, f'no puede salir del paquete con ".." (es "{p}")')


def _structure(v, e: _Errors) -> dict:
    """Comprobaciones de tipos/campos. Devuelve una vista normalizada."""
    c = {"title": "", "artist": "", "audio": "", "video": "", "bpm": math.nan,
         "offset_s": math.nan, "duration_s": math.nan, "steps": [], "ref_imu": ""}
    if not isinstance(v, dict):
        e.push("$", "la raíz debe ser un objeto")
        return c
    if "format" not in v:
        e.push("format", 'falta (debe ser "jd-choreo")')
    elif v["format"] != FORMAT or not isinstance(v["format"], str):
        e.push("format", f'debe ser "{FORMAT}" (es {_short(v["format"])})')
    if "version" not in v:
        e.push("version", "falta (debe ser 1)")
    elif not _is_num(v["version"]):
        e.push("version", f"debe ser un número (es {_short(v['version'])})")
    elif float(v["version"]) != VERSION:
        e.push("version", f"versión no soportada {_fnum(float(v['version']))} (se admite 1)")

    if "song" not in v:
        e.push("song", "falta")
    elif not isinstance(v["song"], dict):
        e.push("song", f"debe ser un objeto (es {_short(v['song'])})")
    else:
        s = v["song"]
        c["title"] = _req_str(e, s, "song", "title")
        c["artist"] = _opt_str(e, s, "song", "artist", False)
        c["audio"] = _req_str(e, s, "song", "audio")
        c["video"] = _opt_str(e, s, "song", "video", True)
        c["bpm"] = _req_num(e, s, "song", "bpm")
        c["offset_s"] = _req_num(e, s, "song", "offset_s")
        c["duration_s"] = _req_num(e, s, "song", "duration_s")

    if "steps" not in v:
        e.push("steps", "falta")
    elif not isinstance(v["steps"], list):
        e.push("steps", f"debe ser una lista (es {_short(v['steps'])})")
    else:
        for i, sv in enumerate(v["steps"]):
            p = f"steps[{i}]"
            if not isinstance(sv, dict):
                e.push(p, f"debe ser un objeto (es {_short(sv)})")
                continue
            st = {"t": _req_num(e, sv, p, "t"), "dur": _req_num(e, sv, p, "dur"),
                  "icon": _opt_str(e, sv, p, "icon", False), "limbs": [], "kind": "accent"}
            if "limbs" not in sv:
                e.push(f"{p}.limbs", "falta")
            elif not isinstance(sv["limbs"], list):
                e.push(f"{p}.limbs", f"debe ser una lista (es {_short(sv['limbs'])})")
            else:
                for j, lv in enumerate(sv["limbs"]):
                    if not isinstance(lv, str):
                        e.push(f"{p}.limbs[{j}]", f"debe ser texto (es {_short(lv)})")
                    elif lv not in LIMBS:
                        e.push(f"{p}.limbs[{j}]", f'extremidad desconocida "{lv}" '
                               "(válidas: left_hand, right_hand, left_foot, right_foot)")
                    else:
                        st["limbs"].append(lv)
                if not sv["limbs"]:
                    e.push(f"{p}.limbs", "no puede estar vacío")
            if "kind" not in sv:
                e.push(f"{p}.kind", 'falta ("accent" o "move")')
            elif sv["kind"] not in KINDS or not isinstance(sv["kind"], str):
                e.push(f"{p}.kind", f'debe ser "accent" o "move" (es {_short(sv["kind"])})')
            else:
                st["kind"] = sv["kind"]
            c["steps"].append(st)

    if "references" in v:
        r = v["references"]
        if not isinstance(r, dict):
            e.push("references", f"debe ser un objeto (es {_short(r)})")
        else:
            c["ref_imu"] = _opt_str(e, r, "references", "imu", True)
    return c


def _semantics(c: dict, e: _Errors):
    """Igual que validate_choreo de Rust."""
    if c["title"].strip() == "":
        e.push("song.title", "no puede estar vacío")
    _check_rel_path(e, "song.audio", c["audio"], False)
    _check_rel_path(e, "song.video", c["video"], True)
    _check_rel_path(e, "references.imu", c["ref_imu"], True)
    bpm, dur, off = c["bpm"], c["duration_s"], c["offset_s"]
    if not math.isfinite(bpm) or bpm <= 0 or bpm > MAX_BPM:
        e.push("song.bpm", f"debe estar en (0, {_fnum(MAX_BPM)}] (es {_fnum(bpm)})")
    dur_ok = math.isfinite(dur) and dur > 0
    if not dur_ok:
        e.push("song.duration_s", f"debe ser > 0 (es {_fnum(dur)})")
    if not math.isfinite(off) or off < 0:
        e.push("song.offset_s", f"debe ser >= 0 (es {_fnum(off)})")
    elif dur_ok and off >= dur:
        e.push("song.offset_s", f"debe ser < song.duration_s ({_fnum(dur)})")
    steps = c["steps"]
    for i, s in enumerate(steps):
        p = f"steps[{i}]"
        t, d = s["t"], s["dur"]
        t_ok = math.isfinite(t) and t >= 0
        if not t_ok:
            e.push(f"{p}.t", f"debe ser >= 0 (es {_fnum(t)})")
        d_ok = math.isfinite(d) and d >= 0
        if not d_ok:
            e.push(f"{p}.dur", f"debe ser >= 0 (es {_fnum(d)})")
        if t_ok and d_ok and dur_ok and t + d > dur + DURATION_EPS:
            e.push(f"{p}.t", f"t + dur ({_fnum(t + d)}) supera song.duration_s ({_fnum(dur)})")
        if i > 0:
            prev = steps[i - 1]["t"]
            if t_ok and math.isfinite(prev) and t < prev:
                e.push(f"{p}.t", f"debe ser >= steps[{i - 1}].t ({_fnum(t)} < {_fnum(prev)})")
        if not s["limbs"]:
            e.push(f"{p}.limbs", "no puede estar vacío")
        for j, l in enumerate(s["limbs"]):
            if l in s["limbs"][:j]:
                e.push(f"{p}.limbs[{j}]", f"extremidad repetida ({l})")


def validate(obj) -> list[str]:
    """Lista de errores (vacía si es válida). Primero estructura; si falla, no sigue."""
    e = _Errors()
    c = _structure(obj, e)
    if e.list:
        return e.result()
    _semantics(c, e)
    return e.result()


def _reject_constant(name):
    raise ValueError(f"constante no válida en JSON: {name}")


def parse_text(text: str):
    """json.loads estricto (sin NaN/Infinity). Errores 'línea:columna: mensaje'."""
    try:
        return json.loads(text, parse_constant=_reject_constant)
    except json.JSONDecodeError as ex:
        raise ChoreoError([f"{ex.lineno}:{ex.colno}: {ex.msg}"]) from None
    except ValueError as ex:
        raise ChoreoError([str(ex)]) from None


def load(path) -> dict:
    """Carga y valida; lanza ChoreoError con mensajes precedidos por la ruta del archivo."""
    path = Path(path)
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as ex:
        raise ChoreoError([f"no se puede abrir {path}: {ex}"]) from None
    try:
        obj = parse_text(text)
    except ChoreoError as ex:
        raise ChoreoError([f"{path}:{m}" for m in ex.errors]) from None
    errs = validate(obj)
    if errs:
        raise ChoreoError([f"{path}: {m}" for m in errs])
    return obj


def dumps(obj) -> str:
    """Serialización legible y estable para choreo.json (un paso por línea)."""
    steps = obj.get("steps", [])
    head = {k: v for k, v in obj.items() if k != "steps"}
    lines = ["{"]
    items = list(head.items())
    for k, v in items:
        if k == "references":
            continue
        lines.append(f"  {json.dumps(k)}: {json.dumps(v, ensure_ascii=False)},")
    lines.append('  "steps": [')
    for i, s in enumerate(steps):
        comma = "," if i + 1 < len(steps) else ""
        lines.append("    " + json.dumps(s, ensure_ascii=False) + comma)
    lines.append("  ],")
    lines.append(f'  "references": {json.dumps(head.get("references", {}), ensure_ascii=False)}')
    lines.append("}")
    return "\n".join(lines) + "\n"


def save(path, obj):
    """Valida y escribe (atómico). Lanza ChoreoError si no es válida."""
    errs = validate(obj)
    if errs:
        raise ChoreoError([f"{path}: {m}" for m in errs])
    path = Path(path)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(dumps(obj), encoding="utf-8")
    tmp.replace(path)


def main(argv=None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__.strip())
        return 0 if argv else 2
    bad = 0
    for a in argv:
        p = Path(a)
        if p.is_dir():
            p = p / "choreo.json"
        try:
            c = load(p)
            print(f"{p}: OK ({len(c['steps'])} pasos, {_fnum(float(c['song']['bpm']))} BPM)")
        except ChoreoError as ex:
            bad += 1
            for m in ex.errors:
                print(m, file=sys.stderr)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
