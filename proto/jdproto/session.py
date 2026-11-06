"""Carga de sesiones grabadas (contracts/session_format.md)."""

import json

from .config import LIMBS


class SessionError(ValueError):
    pass


def _vec3(value, where):
    if value is None:
        return None
    if not isinstance(value, (list, tuple)) or len(value) != 3:
        raise SessionError(f"{where}: se esperaba un vector de 3 números")
    try:
        return [float(value[0]), float(value[1]), float(value[2])]
    except (TypeError, ValueError):
        raise SessionError(f"{where}: se esperaba un vector de 3 números") from None


def parse_session_lines(lines, name="<sesión>"):
    header = None
    samples = []
    for lineno, raw in enumerate(lines, start=1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError as e:
            raise SessionError(f"{name}:{lineno}: JSON inválido ({e.msg})") from None
        if header is None:
            if obj.get("format") != "jd-session":
                raise SessionError(f"{name}:{lineno}: la cabecera debe tener format=\"jd-session\"")
            if obj.get("version") != 1:
                raise SessionError(f"{name}:{lineno}: versión de sesión no soportada: {obj.get('version')!r}")
            header = obj
            header.setdefault("time_base", "song")
            header.setdefault("devices", [])
            continue
        if "t" not in obj:
            raise SessionError(f"{name}:{lineno}: muestra sin 't'")
        src = int(obj.get("src", 0))
        limb = obj.get("limb")
        if limb is None:
            for dev in header["devices"]:
                if int(dev.get("source_id", -1)) == src:
                    limb = dev.get("limb")
                    break
        if limb not in LIMBS:
            limb = "none"
        samples.append({
            "t": float(obj["t"]),
            "src": src,
            "limb": limb,
            "a": _vec3(obj.get("a"), f"{name}:{lineno}.a"),
            "g": _vec3(obj.get("g"), f"{name}:{lineno}.g"),
        })
    if header is None:
        raise SessionError(f"{name}: archivo vacío (falta la cabecera)")
    return {"header": header, "samples": samples}


def load_session(path):
    """-> dict(header=..., samples=[dict(t, src, limb, a, g)]). `a`/`g` = None si faltan."""
    with open(path, "r", encoding="utf-8") as f:
        return parse_session_lines(f, name=str(path))


def to_song_time(samples, mono_to_song_s):
    """Copia de las muestras con t desplazado (para sesiones con time_base="mono")."""
    return [dict(s, t=s["t"] + mono_to_song_s) for s in samples]


def write_session(path, header, samples, t_decimals=6, v_decimals=5):
    """Escribe una sesión .jsonl (cabecera + una muestra por línea)."""
    with open(path, "w", encoding="utf-8") as f:
        f.write(json.dumps(header, ensure_ascii=False) + "\n")
        for s in samples:
            obj = {"t": round(s["t"], t_decimals), "src": s["src"], "limb": s["limb"]}
            if s.get("a") is not None:
                obj["a"] = [round(float(v), v_decimals) for v in s["a"]]
            if s.get("g") is not None:
                obj["g"] = [round(float(v), v_decimals) for v in s["g"]]
            f.write(json.dumps(obj, separators=(",", ":")) + "\n")
