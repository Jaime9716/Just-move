"""CLI: python -m jdproto.judge_cli <song_dir> <player_session.jsonl> [--json] (PYTHONPATH=proto).

Opciones extra (no rompen el contrato):
  --no-ref            ignora ref_imu.jsonl (todos los moves sin referencia)
  --variant NAME      v1 (por defecto) | v1b  (ver proto/ALGORITHM.md)
  --config FILE.json  sobrescribe constantes (claves de jdproto.config.DEFAULT_CONFIG)
  --offset S          mono_to_song_s para sesiones con time_base="mono" (t_song = t + S)
"""

import argparse
import json
import os
import sys

from .choreo import ChoreoError, load_choreo
from .pipeline import judge_song, summarize
from .session import SessionError, load_session, to_song_time


def main(argv=None):
    ap = argparse.ArgumentParser(prog="python -m jdproto.judge_cli",
                                 description="Juez v1: puntúa una sesión grabada contra una canción.")
    ap.add_argument("song_dir")
    ap.add_argument("session")
    ap.add_argument("--json", action="store_true", help="salida JSON (lista de resultados)")
    ap.add_argument("--no-ref", action="store_true")
    ap.add_argument("--variant", default=None)
    ap.add_argument("--config", default=None)
    ap.add_argument("--offset", type=float, default=None)
    args = ap.parse_args(argv)

    try:
        choreo = load_choreo(args.song_dir)
        player = load_session(args.session)
        ref = None
        ref_name = choreo.get("references", {}).get("imu")
        if ref_name and not args.no_ref:
            ref_path = os.path.join(args.song_dir, ref_name)
            if os.path.exists(ref_path):
                ref = load_session(ref_path)
            else:
                print(f"aviso: no existe la referencia {ref_path}; moves sin referencia", file=sys.stderr)
    except (ChoreoError, SessionError, OSError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 2

    samples = player["samples"]
    if args.offset is not None:
        samples = to_song_time(samples, args.offset)
    elif player["header"].get("time_base") == "mono":
        print("aviso: la sesión tiene time_base=\"mono\"; usa --offset S para alinearla con la canción "
              "(se asume S = 0)", file=sys.stderr)

    config = {}
    if args.config:
        with open(args.config, "r", encoding="utf-8") as f:
            config.update(json.load(f))
    if args.variant:
        config["variant"] = args.variant
    try:
        results = judge_song(choreo, samples, ref["samples"] if ref else None, config or None)
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2

    if args.json:
        json.dump(results, sys.stdout, indent=1)
        sys.stdout.write("\n")
        return 0
    steps = choreo["steps"]
    print(f"{'paso':>4} {'t':>7} {'tipo':<6} {'icono':<14} {'extremidad':<11} {'nota':<8} {'sim':>5} {'err_ms':>7}")
    for r in results:
        st = steps[r["step"]]
        print(f"{r['step']:>4} {st['t']:>7.2f} {st['kind']:<6} {st.get('icon', ''):<14} {r['limb']:<11} "
              f"{r['grade']:<8} {r['similarity']:>5.2f} {r['timing_error_ms']:>7.1f}")
    c = summarize(results)
    print(f"\nPerfect {c['perfect']} · Good {c['good']} · Miss {c['miss']} (total {len(results)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
