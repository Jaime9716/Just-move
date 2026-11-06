#!/usr/bin/env python3
"""yt-dlp simulado para las pruebas: no usa red. Copia los medios de FAKE_YT_SRC.

Entiende lo que usa yt_import.py: --dump-single-json --skip-download URL, y -f FMT -o TMPL URL.
Registra cada llamada en FAKE_YT_LOG (una línea JSON por llamada).
"""
import json
import os
import shutil
import sys
from pathlib import Path

INFO = {"id": "abc123XYZ", "title": "Canción de prueba", "channel": "Canal Falso",
        "uploader": "Canal Falso", "duration": 12.0, "webpage_url": "https://www.youtube.com/watch?v=abc123XYZ"}


def main(argv):
    log = os.environ.get("FAKE_YT_LOG")
    if log:
        with open(log, "a", encoding="utf-8") as f:
            f.write(json.dumps(argv) + "\n")
    if os.environ.get("FAKE_YT_FAIL"):
        print("ERROR: [youtube] abc123XYZ: Video unavailable", file=sys.stderr)
        return 1
    if "--dump-single-json" in argv:
        print(json.dumps(INFO, ensure_ascii=False))
        return 0
    fmt = argv[argv.index("-f") + 1]
    tmpl = argv[argv.index("-o") + 1]
    src = Path(os.environ["FAKE_YT_SRC"])
    f = src / ("audio_src.mka" if fmt.startswith("bestaudio") else "video_src.mkv")
    shutil.copy(f, tmpl.replace("%(ext)s", f.suffix[1:]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
