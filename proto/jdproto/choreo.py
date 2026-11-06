"""Carga y validación de choreo.json (contracts/song_package.md)."""

import json
import math
import os

from .config import LIMBS


class ChoreoError(ValueError):
    pass


def _num(obj, key, path, required=True, default=None):
    if key not in obj or obj[key] is None:
        if required:
            raise ChoreoError(f"{path}.{key}: falta")
        return default
    v = obj[key]
    if isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v):
        raise ChoreoError(f"{path}.{key}: debe ser un número")
    return float(v)


def validate_choreo(c):
    """Valida un dict de coreografía; lanza ChoreoError con ruta JSON. Devuelve el mismo dict."""
    if not isinstance(c, dict):
        raise ChoreoError("$: debe ser un objeto")
    if c.get("format") != "jd-choreo":
        raise ChoreoError('format: debe ser "jd-choreo"')
    if c.get("version") != 1:
        raise ChoreoError(f"version: no soportada ({c.get('version')!r})")
    song = c.get("song")
    if not isinstance(song, dict):
        raise ChoreoError("song: falta o no es un objeto")
    for key in ("bpm", "offset_s", "duration_s"):
        _num(song, key, "song")
    if not isinstance(song.get("audio"), str) or not song["audio"]:
        raise ChoreoError("song.audio: falta")
    steps = c.get("steps")
    if not isinstance(steps, list):
        raise ChoreoError("steps: debe ser una lista")
    prev_t = -math.inf
    for i, st in enumerate(steps):
        p = f"steps[{i}]"
        if not isinstance(st, dict):
            raise ChoreoError(f"{p}: debe ser un objeto")
        t = _num(st, "t", p)
        dur = _num(st, "dur", p)
        if dur < 0:
            raise ChoreoError(f"{p}.dur: debe ser >= 0")
        if t < prev_t:
            raise ChoreoError(f"{p}.t: debe ser >= steps[{i - 1}].t")
        prev_t = t
        limbs = st.get("limbs")
        if not isinstance(limbs, list) or not limbs:
            raise ChoreoError(f"{p}.limbs: debe ser una lista no vacía")
        for j, limb in enumerate(limbs):
            if limb not in LIMBS:
                raise ChoreoError(f"{p}.limbs[{j}]: extremidad desconocida {limb!r}")
        if st.get("kind") not in ("accent", "move"):
            raise ChoreoError(f'{p}.kind: debe ser "accent" o "move"')
        st.setdefault("icon", "")
    refs = c.get("references", {})
    if refs is None:
        refs = {}
    if not isinstance(refs, dict):
        raise ChoreoError("references: debe ser un objeto")
    c["references"] = refs
    return c


def load_choreo(path):
    """Carga choreo.json (ruta al archivo o a la carpeta de la canción) -> dict como choreo.json."""
    if os.path.isdir(path):
        path = os.path.join(path, "choreo.json")
    with open(path, "r", encoding="utf-8") as f:
        try:
            c = json.load(f)
        except json.JSONDecodeError as e:
            raise ChoreoError(f"{path}: JSON inválido en línea {e.lineno} ({e.msg})") from None
    return validate_choreo(c)
