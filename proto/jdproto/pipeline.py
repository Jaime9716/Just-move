"""judge_song(): B3 → B4 → B5 sobre jugador y referencia (API de contracts/judge_algorithm_v1.md)."""

from . import features, judge, signal
from .config import merge_config, resolve_config


def extract(samples, config=None):
    """Muestras (tiempo de canción) -> {limb: {"frames", "peaks"}}."""
    cfg = resolve_config(config)
    return features.compute_features(signal.process_samples(samples, cfg), cfg)


def judge_song(choreo, player_samples, ref_samples=None, config=None, details=None):
    """(choreo: dict, player_samples: list, ref_samples: list|None, config: dict|None)
    -> list[dict(step, limb, grade, similarity, timing_error_ms)].

    Las muestras usan tiempo de canción. `config` sobrescribe constantes (ver config.DEFAULT_CONFIG;
    admite "variant": "v1b"). Sin `ref_samples` todos los moves se juzgan sin referencia."""
    cfg = merge_config(config)
    player = extract(player_samples, cfg)
    ref = extract(ref_samples, cfg) if ref_samples is not None else None
    return judge.judge_steps(choreo, player, ref, cfg, details=details)


def summarize(results):
    """Cuenta de grados {"perfect": n, "good": n, "miss": n}."""
    out = {"perfect": 0, "good": 0, "miss": 0}
    for r in results:
        out[r["grade"]] += 1
    return out
