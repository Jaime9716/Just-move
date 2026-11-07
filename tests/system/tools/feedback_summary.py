#!/usr/bin/env python3
"""Resume los feedbacks de docs/feedback/*.md (copias de plantilla.md).

Por archivo: momento, fecha, entorno (móvil, Wi-Fi, audio…), canción, pruebas con su resultado,
diversión y fallos. En conjunto: último estado de cada prueba P1–P11, pruebas pendientes, media de
diversión, móviles y redes usados y lista de fallos abiertos.

Ejemplos:
  python tests/system/tools/feedback_summary.py
  python tests/system/tools/feedback_summary.py --dir docs/feedback --json
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TESTS = [f"P{i}" for i in range(1, 12)]
SKIP = {"readme.md", "plantilla.md"}
_COMMENT = re.compile(r"<!--.*?-->", re.S)
_PLACEHOLDER = re.compile(r"^<[^>]*>$")


def _clean(v: str) -> str:
    v = _COMMENT.sub("", v).strip()
    return "" if _PLACEHOLDER.match(v) else v


def _yes(v: str):
    s = v.strip().lower()
    if s in ("sí", "si", "yes", "s", "ok", "cumple", "✔", "✓"):
        return True
    if s in ("no", "n", "✗", "x", "no cumple"):
        return False
    return None


def parse_feedback(path: Path) -> dict:
    text = _COMMENT.sub("", path.read_text(encoding="utf-8-sig"))
    r = {"file": path.name, "moment": "", "date": "", "sections": {}, "tests": [], "fun": None, "failure": None}
    m = re.match(r"(\d{4}-\d{2}-\d{2})-(.+)\.md$", path.name)
    if m:
        r["date"], r["moment"] = m.group(1), m.group(2)
    section = ""
    in_table = False
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("# "):
            parts = [p.strip() for p in s[2:].split("·")]
            if len(parts) >= 2 and not _PLACEHOLDER.match(parts[1]):
                r["moment"] = parts[1] or r["moment"]
            if len(parts) >= 3 and re.match(r"\d{4}-\d{2}-\d{2}", parts[2]):
                r["date"] = parts[2][:10]
            continue
        if s.startswith("## "):
            section = s[3:].strip()
            in_table = False
            continue
        if s.startswith("|"):
            cells = [c.strip() for c in s.strip("|").split("|")]
            if not in_table:
                in_table = True       # cabecera
                continue
            if all(set(c) <= set("-: ") for c in cells):
                continue
            pid = cells[0].upper().replace(" ", "")
            if not re.match(r"^P\d{1,2}$", pid):
                continue
            res = cells[1] if len(cells) > 1 else ""
            ok = _yes(cells[2]) if len(cells) > 2 else None
            if not res and ok is None:
                continue
            r["tests"].append({"id": pid, "result": res, "pass": ok, "comment": cells[3] if len(cells) > 3 else ""})
            continue
        in_table = False
        mm = re.match(r"^[-*]\s+([^:]+?):\s*(.*)$", s)
        if mm and section:
            key, val = mm.group(1).strip(), _clean(mm.group(2))
            if val:
                r["sections"].setdefault(section, {})[key] = val
                if key.lower().startswith("diversión") or key.lower().startswith("diversion") \
                        or "divertido" in key.lower():
                    n = re.search(r"\d+(?:[.,]\d+)?", val)
                    if n:
                        r["fun"] = float(n.group(0).replace(",", "."))
    fallos = r["sections"].get("Fallos", {})
    if any(k.lower().startswith("qué pasó") or k.lower().startswith("que paso") for k in fallos):
        r["failure"] = fallos
    return r


def summarize(items: list[dict]) -> dict:
    items = sorted(items, key=lambda x: (x["date"], x["file"]))
    latest: dict[str, dict] = {}
    history: dict[str, list] = {}
    for it in items:
        for t in it["tests"]:
            e = dict(t, date=it["date"], file=it["file"])
            history.setdefault(t["id"], []).append(e)
            latest[t["id"]] = e
    fun = [it["fun"] for it in items if it["fun"] is not None]
    env = [it["sections"].get("Entorno", {}) for it in items]

    def pick(prefix):
        vals = []
        for e in env:
            for k, v in e.items():
                if k.lower().startswith(prefix) and v not in vals:
                    vals.append(v)
        return vals

    return {
        "files": len(items),
        "latest": {k: latest[k] for k in sorted(latest, key=lambda s: int(s[1:]))},
        "history": history,
        "passed": sorted([k for k, v in latest.items() if v["pass"] is True], key=lambda s: int(s[1:])),
        "failed": sorted([k for k, v in latest.items() if v["pass"] is False], key=lambda s: int(s[1:])),
        "pending": [p for p in TESTS if p not in latest or latest[p]["pass"] is None],
        "fun_mean": round(sum(fun) / len(fun), 2) if fun else None,
        "phones": pick("móvil") + pick("movil"),
        "wifi": pick("wi-fi") + pick("wifi"),
        "audio": pick("salida de audio"),
        "songs": [it["sections"].get("Canción", {}) for it in items if it["sections"].get("Canción")],
        "failures": [{"file": it["file"], "date": it["date"], **it["failure"]} for it in items if it["failure"]],
    }


def main(argv=None):
    ap = argparse.ArgumentParser(description="Resume docs/feedback/*.md")
    ap.add_argument("--dir", default=str(ROOT / "docs" / "feedback"), help="carpeta de feedback")
    ap.add_argument("--json", action="store_true", help="salida JSON")
    a = ap.parse_args(argv)
    d = Path(a.dir)
    if not d.is_dir():
        print(f"ERROR: no existe {d}", file=sys.stderr)
        return 2
    items = [parse_feedback(p) for p in sorted(d.glob("*.md")) if p.name.lower() not in SKIP]
    s = summarize(items)
    if a.json:
        print(json.dumps({"items": items, "summary": s}, ensure_ascii=False, indent=2))
        return 0
    print(f"Feedbacks: {s['files']} en {d}")
    for it in sorted(items, key=lambda x: (x["date"], x["file"])):
        print(f"\n== {it['file']} · {it['moment'] or '?'} · {it['date'] or '?'}")
        for sec in ("Entorno", "Canción"):
            for k, v in it["sections"].get(sec, {}).items():
                print(f"   {k}: {v}")
        for t in it["tests"]:
            ok = {True: "sí", False: "NO", None: "?"}[t["pass"]]
            print(f"   {t['id']:>3} [{ok}] {t['result']}  {t['comment']}")
        for k, v in it["sections"].get("Sensaciones", {}).items():
            print(f"   > {k} {v}")
        if it["failure"]:
            print(f"   FALLO: {it['failure'].get('Qué pasó', '')}")
    print("\n== Resumen")
    for pid, e in s["latest"].items():
        ok = {True: "cumple", False: "NO cumple", None: "sin veredicto"}[e["pass"]]
        print(f"   {pid:>3}: {ok} ({e['date']}) {e['result']}")
    print(f"   Cumplen: {', '.join(s['passed']) or '—'} · No cumplen: {', '.join(s['failed']) or '—'}")
    print(f"   Pendientes: {', '.join(s['pending']) or '—'}")
    print(f"   Diversión media: {s['fun_mean'] if s['fun_mean'] is not None else '—'}")
    if s["phones"]:
        print(f"   Móviles: {'; '.join(s['phones'])}")
    if s["wifi"]:
        print(f"   Wi-Fi: {'; '.join(s['wifi'])}")
    print(f"   Fallos reportados: {len(s['failures'])}")
    for f in s["failures"]:
        print(f"     - {f['file']}: {f.get('Qué pasó', '')} (sesión: {f.get('Sesión grabada (archivo en data/sessions/real/)', '—')})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
