#!/usr/bin/env python3
"""Genera data/sessions/synthetic/: canción demo, referencia, sesiones del jugador y golden.

Uso:  python3 proto/synth.py [--out data/sessions/synthetic] [--variant v1]
Es determinista (semillas fijas): ejecutarlo dos veces produce los mismos archivos.
Los golden se calculan leyendo los .jsonl ya escritos (mismo redondeo que verá el motor Rust).
"""

import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from jdproto import synth  # noqa: E402
from jdproto.choreo import load_choreo  # noqa: E402
from jdproto.pipeline import judge_song, summarize  # noqa: E402
from jdproto.session import load_session, write_session  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.join(ROOT, "data", "sessions", "synthetic"))
    ap.add_argument("--variant", default=None, help="solo para la tabla de consola; los golden son v1")
    args = ap.parse_args(argv)

    song_dir = os.path.join(args.out, "song_demo")
    gold_dir = os.path.join(args.out, "golden")
    os.makedirs(song_dir, exist_ok=True)
    os.makedirs(gold_dir, exist_ok=True)

    choreo = synth.make_demo_choreo()
    with open(os.path.join(song_dir, "choreo.json"), "w", encoding="utf-8") as f:
        json.dump(choreo, f, indent=2, ensure_ascii=False)
        f.write("\n")

    note, kw = synth.REF
    write_session(os.path.join(song_dir, "ref_imu.jsonl"), synth.session_header(kw["limbs"], note),
                  synth.render_session(choreo, **kw))
    for name, (note, kw) in synth.SESSIONS.items():
        write_session(os.path.join(args.out, name + ".jsonl"), synth.session_header(kw["limbs"], note),
                      synth.render_session(choreo, **kw))

    # golden (v1) a partir de los archivos escritos
    choreo = load_choreo(song_dir)
    ref = load_session(os.path.join(song_dir, "ref_imu.jsonl"))["samples"]
    print(f"{'sesión':<14} {'perfect':>7} {'good':>5} {'miss':>5}")
    for name in list(synth.SESSIONS) + ["perfect_noref"]:
        sess = "perfect" if name == "perfect_noref" else name
        player = load_session(os.path.join(args.out, sess + ".jsonl"))["samples"]
        use_ref = None if name == "perfect_noref" else ref
        res = judge_song(choreo, player, use_ref, None)
        with open(os.path.join(gold_dir, name + ".json"), "w", encoding="utf-8") as f:
            json.dump(res, f, indent=1)
            f.write("\n")
        line = summarize(res)
        if args.variant:
            alt = summarize(judge_song(choreo, player, use_ref, {"variant": args.variant}))
            extra = f"   {args.variant}: {alt['perfect']}/{alt['good']}/{alt['miss']}"
        else:
            extra = ""
        print(f"{name:<14} {line['perfect']:>7} {line['good']:>5} {line['miss']:>5}{extra}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
