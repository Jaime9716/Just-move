"""B5 · Juez v1 (contracts/judge_algorithm_v1.md §B5).

Trabaja con tramas indexadas por k (centro k·hop). Precisiones (ver proto/ALGORITHM.md):
  * Ventana de un move: tramas con k ∈ [ceil(t/hop − 1e-6), floor((t+dur)/hop + 1e-6)].
  * Desfases L = m·hop con |m| ≤ floor(gw·lag_search_scale/hop + 1e-6) (v1: lag_search_scale = 1).
    Orden de evaluación m = 0, −1, +1, −2, +2…; solo un score ESTRICTAMENTE mayor sustituye
    (empate → menor |L|, y a igual |L| gana el negativo).
  * Emparejado trama a trama: (ref k, jugador k+m), solo pares donde existen ambas tramas.
  * Varianzas poblacionales; ncc = Σ dr·dp / sqrt(Σ dr² · Σ dp²); "varianza < 1e-6" → ncc = 0.
  * Move con referencia ⇔ existe al menos una trama de referencia de esa extremidad en la ventana.
  * Accent: pico con |peak_t − t| ≤ gw + 1e-9; el más cercano (empate → el anterior).
    Miss sin pico → similarity 0, timing_error_ms 0.
  * Comparaciones de ventana de tiempo con tolerancia 1e-9 s.
"""

import math

from .config import FOOT_LIMBS, LIMB_ORDER, resolve_config

EPS = 1e-9


def windows(limb, cfg):
    pw = cfg["perfect_window_ms"] / 1000.0
    gw = cfg["good_window_ms"] / 1000.0
    if limb in FOOT_LIMBS:
        pw *= cfg["foot_window_scale"]
        gw *= cfg["foot_window_scale"]
    return pw, gw


def emit_time(step, limb, config=None):
    """song_time a partir del cual se emite el resultado (regla de emisión del contrato)."""
    cfg = resolve_config(config)
    _, gw = windows(limb, cfg)
    return step["t"] + step["dur"] + gw + cfg["emit_margin_s"]


def judge_accent(step, peaks_t, pw, gw):
    t = step["t"]
    best = None
    for pt in peaks_t:
        err = pt - t
        if abs(err) <= gw + EPS:
            if best is None or abs(err) < abs(best):
                best = err
    if best is None:
        return "miss", 0.0, 0.0
    if abs(best) <= pw + EPS:
        grade = "perfect"
    else:
        grade = "good"
    return grade, max(0.0, 1.0 - abs(best) / gw), best * 1000.0


def _ncc(er, ep):
    n = len(er)
    if n == 0:
        return 0.0
    mr = sum(er) / n
    mp = sum(ep) / n
    dr = [x - mr for x in er]
    dp = [x - mp for x in ep]
    sr = sum(x * x for x in dr)
    sp = sum(x * x for x in dp)
    if sr / n < 1e-6 or sp / n < 1e-6:
        return 0.0
    c = sum(a * b for a, b in zip(dr, dp)) / math.sqrt(sr * sp)
    return min(1.0, max(0.0, c))


def _dcos(pairs):
    cs = []
    for rf, pf in pairs:
        a, b = rf["dir"], pf["dir"]
        if (a[0] == 0.0 and a[1] == 0.0 and a[2] == 0.0) or (b[0] == 0.0 and b[1] == 0.0 and b[2] == 0.0):
            continue
        na = math.sqrt(a[0] * a[0] + a[1] * a[1] + a[2] * a[2])
        nb = math.sqrt(b[0] * b[0] + b[1] * b[1] + b[2] * b[2])
        cs.append((a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (na * nb))
    if not cs:
        return None
    return min(1.0, max(0.0, sum(cs) / len(cs)))


def move_window_keys(step, hop):
    k0 = math.ceil(step["t"] / hop - 1e-6)
    k1 = math.floor((step["t"] + step["dur"]) / hop + 1e-6)
    return k0, k1


def lag_order(m_max):
    order = [0]
    for m in range(1, m_max + 1):
        order.extend((-m, m))
    return order


def judge_move_ref(ref_keys, ref_frames, player_frames, pw, gw, cfg, detail=None):
    hop = cfg["frame_hop_s"]
    m_max = math.floor(gw * cfg["lag_search_scale"] / hop + 1e-6)
    best = None  # (score, m, pairs)
    for m in lag_order(m_max):
        pairs = [(ref_frames[k], player_frames[k + m]) for k in ref_keys if (k + m) in player_frames]
        if not pairs:
            score = 0.0
        else:
            ncc = _ncc([p[0]["energy"] for p in pairs], [p[1]["energy"] for p in pairs])
            dc = _dcos(pairs)
            score = ncc if dc is None else cfg["w_ncc"] * ncc + cfg["w_dir"] * dc
        if detail is not None:
            detail.setdefault("lags", []).append((m * hop, score))
        if best is None or score > best[0]:
            best = (score, m, pairs)
    score, m, pairs = best
    if pairs:
        mean_er = sum(p[0]["energy"] for p in pairs) / len(pairs)
        mean_ep = sum(p[1]["energy"] for p in pairs) / len(pairs)
    else:
        mean_er = sum(ref_frames[k]["energy"] for k in ref_keys) / len(ref_keys)
        mean_ep = 0.0
    gate = cfg["energy_gate"] * mean_er
    if gate > 0.0 and mean_ep < gate:
        score *= mean_ep / gate
    lag = m * hop
    if score >= cfg["perfect_sim"] and abs(lag) <= pw + EPS:
        grade = "perfect"
    elif score >= cfg["good_sim"] and abs(lag) <= gw + EPS:
        grade = "good"
    else:
        grade = "miss"
    return grade, score, lag * 1000.0


def judge_move_noref(step, player_frames, cfg):
    k0, k1 = move_window_keys(step, cfg["frame_hop_s"])
    es = [player_frames[k]["energy"] for k in range(k0, k1 + 1) if k in player_frames]
    mean = sum(es) / len(es) if es else 0.0
    sim = min(1.0, max(0.0, mean / cfg["noref_energy_full"]))
    return ("good" if sim >= cfg["good_sim"] else "miss"), sim, 0.0


def judge_steps(choreo, player_feats, ref_feats=None, config=None, details=None):
    """player_feats/ref_feats: {limb: {"frames": {k: trama}, ...}} (ver features.compute_features).

    -> lista de dict(step, limb, grade, similarity, timing_error_ms) en orden de paso y extremidad."""
    cfg = resolve_config(config)
    hop = cfg["frame_hop_s"]
    results = []
    empty = {"frames": {}, "peaks": []}
    for i, step in enumerate(choreo["steps"]):
        limbs = sorted(set(step["limbs"]), key=lambda l: LIMB_ORDER[l])
        for limb in limbs:
            pw, gw = windows(limb, cfg)
            pf = player_feats.get(limb, empty)["frames"]
            detail = {} if details is not None else None
            if step["kind"] == "accent":
                peaks_t = sorted(f["peak_t_song"] for f in pf.values() if f["is_peak"])
                grade, sim, terr = judge_accent(step, peaks_t, pw, gw)
            else:
                rf = ref_feats.get(limb, empty)["frames"] if ref_feats is not None else {}
                k0, k1 = move_window_keys(step, hop)
                ref_keys = [k for k in range(k0, k1 + 1) if k in rf]
                if ref_keys:
                    grade, sim, terr = judge_move_ref(ref_keys, rf, pf, pw, gw, cfg, detail)
                else:
                    grade, sim, terr = judge_move_noref(step, pf, cfg)
            results.append({"step": i, "limb": limb, "grade": grade,
                            "similarity": float(sim), "timing_error_ms": float(terr)})
            if details is not None:
                details.append(detail)
    return results
