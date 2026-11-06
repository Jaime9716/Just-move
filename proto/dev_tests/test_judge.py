"""B5 con tramas construidas a mano."""

import pytest

from jdproto import judge
from jdproto.config import merge_config

CFG = merge_config()


def frame(k, energy=1.0, d=(0.0, 0.0, 0.0), peak_t=None):
    return {"k": k, "t": k * 0.02, "energy": energy, "peak": energy, "is_peak": peak_t is not None,
            "peak_t_song": peak_t or 0.0, "dir": d, "quat": (1.0, 0.0, 0.0, 0.0)}


def feats(frames):
    return {"frames": {f["k"]: f for f in frames}, "peaks": []}


def choreo(*steps):
    return {"steps": [dict(t=t, dur=d, limbs=l, kind=k, icon="") for (t, d, l, k) in steps]}


def peak_feats(limb, times):
    return {limb: feats([frame(round(t / 0.02), peak_t=t) for t in times])}


@pytest.mark.parametrize("err_ms,grade", [(0, "perfect"), (49, "perfect"), (-50, "perfect"), (51, "good"),
                                          (-100, "good"), (101, "miss"), (150, "miss")])
def test_accent_windows_hand(err_ms, grade):
    c = choreo((2.0, 0.5, ["right_hand"], "accent"))
    r = judge.judge_steps(c, peak_feats("right_hand", [2.0 + err_ms / 1000.0]), None, CFG)[0]
    assert r["grade"] == grade
    if grade != "miss":
        assert r["timing_error_ms"] == pytest.approx(err_ms)
        assert r["similarity"] == pytest.approx(max(0.0, 1 - abs(err_ms) / 100.0))
    else:
        assert r["similarity"] == 0.0 and r["timing_error_ms"] == 0.0


@pytest.mark.parametrize("err_ms,grade", [(70, "perfect"), (76, "good"), (150, "good"), (151, "miss")])
def test_accent_windows_foot(err_ms, grade):
    c = choreo((2.0, 0.5, ["left_foot"], "accent"))
    r = judge.judge_steps(c, peak_feats("left_foot", [2.0 + err_ms / 1000.0]), None, CFG)[0]
    assert r["grade"] == grade


def test_accent_takes_closest_peak():
    c = choreo((2.0, 0.5, ["right_hand"], "accent"))
    r = judge.judge_steps(c, peak_feats("right_hand", [1.93, 2.06]), None, CFG)[0]
    assert r["timing_error_ms"] == pytest.approx(60.0)


def test_order_by_step_then_limb():
    c = choreo((1.0, 0.5, ["right_foot", "left_hand", "right_hand"], "accent"),
               (2.0, 0.5, ["right_hand", "left_hand"], "accent"))
    res = judge.judge_steps(c, {}, None, CFG)
    assert [(r["step"], r["limb"]) for r in res] == [
        (0, "left_hand"), (0, "right_hand"), (0, "right_foot"), (1, "left_hand"), (1, "right_hand")]
    assert all(r["grade"] == "miss" for r in res)


def _pattern(k, shift=0):
    import math
    x = k - shift
    return 2.0 + 3.0 * math.exp(-((x - 55) / 5.0) ** 2) + 1.5 * math.exp(-((x - 68) / 3.0) ** 2)


def _move_case(shift_frames, scale=1.0, dirs=True):
    ks_ref = range(40, 80)          # t = 0.8 … 1.58
    ks_pl = range(20, 110)
    d = (1.0, 0.0, 0.0) if dirs else (0.0, 0.0, 0.0)
    ref = {"right_hand": feats([frame(k, _pattern(k), d) for k in ks_ref])}
    pl = {"right_hand": feats([frame(k, scale * _pattern(k, shift_frames), d) for k in ks_pl])}
    c = choreo((0.8, 0.78, ["right_hand"], "move"))
    return c, pl, ref


@pytest.mark.parametrize("shift,grade,lag", [(0, "perfect", 0.0), (2, "perfect", 40.0), (-2, "perfect", -40.0),
                                             (3, "good", 60.0), (5, "good", 100.0)])
def test_move_lag_search(shift, grade, lag):
    c, pl, ref = _move_case(shift)
    r = judge.judge_steps(c, pl, ref, CFG)[0]
    assert r["grade"] == grade
    assert r["timing_error_ms"] == pytest.approx(lag)
    assert r["similarity"] == pytest.approx(1.0)


def test_move_energy_gate():
    c, pl, ref = _move_case(0, scale=0.1)
    r = judge.judge_steps(c, pl, ref, CFG)[0]
    # ncc = 1, dcos = 1 → score 1 × (0.1 / 0.25)
    assert r["similarity"] == pytest.approx(0.4)
    assert r["grade"] == "miss"


def test_move_without_dirs_uses_ncc_only():
    c, pl, ref = _move_case(0, dirs=False)
    r = judge.judge_steps(c, pl, ref, CFG)[0]
    assert r["similarity"] == pytest.approx(1.0)


def test_move_tie_prefers_smaller_lag_then_negative():
    # energía constante → ncc = 0 en todos los desfases; dcos = 1 → score 0.4 en todos
    ref = {"right_hand": feats([frame(k, 2.0, (1.0, 0.0, 0.0)) for k in range(40, 80)])}
    pl = {"right_hand": feats([frame(k, 2.0, (1.0, 0.0, 0.0)) for k in range(20, 110)])}
    c = choreo((0.8, 0.78, ["right_hand"], "move"))
    r = judge.judge_steps(c, pl, ref, CFG)[0]
    assert r["timing_error_ms"] == 0.0 and r["similarity"] == pytest.approx(0.4)
    assert judge.lag_order(2) == [0, -1, 1, -2, 2]


def test_move_noref():
    pl = {"right_hand": feats([frame(k, 4.5) for k in range(0, 200)])}
    c = choreo((1.0, 1.0, ["right_hand"], "move"))
    r = judge.judge_steps(c, pl, None, CFG)[0]
    assert r["grade"] == "good" and r["similarity"] == pytest.approx(0.75) and r["timing_error_ms"] == 0.0
    pl = {"right_hand": feats([frame(k, 2.0) for k in range(0, 200)])}
    assert judge.judge_steps(c, pl, None, CFG)[0]["grade"] == "miss"
    pl = {"right_hand": feats([frame(k, 60.0) for k in range(0, 200)])}
    assert judge.judge_steps(c, pl, None, CFG)[0]["similarity"] == 1.0  # nunca Perfect
    assert judge.judge_steps(c, pl, None, CFG)[0]["grade"] == "good"


def test_move_window_keys():
    assert judge.move_window_keys({"t": 7.0, "dur": 2.0}, 0.02) == (350, 450)
    assert judge.move_window_keys({"t": 9.5, "dur": 1.5}, 0.02) == (475, 550)


def test_emit_time():
    st = {"t": 2.0, "dur": 0.5}
    # v1.1: t + dur + LAG_SEARCH_SCALE·gw_escalado + EMIT_MARGIN_S
    assert judge.emit_time(st, "right_hand") == pytest.approx(2.0 + 0.5 + 0.2 + 0.05)
    assert judge.emit_time(st, "right_foot") == pytest.approx(2.0 + 0.5 + 0.3 + 0.05)
    assert judge.emit_time(st, "right_hand", {"variant": "v1"}) == pytest.approx(2.0 + 0.5 + 0.1 + 0.05)


def test_lag_range_v11_default_vs_v1():
    c, pl, ref = _move_case(7)  # 140 ms tarde
    v11 = judge.judge_steps(c, pl, ref, CFG)[0]
    v1 = judge.judge_steps(c, pl, ref, merge_config({"variant": "v1"}))[0]
    assert v1["timing_error_ms"] == pytest.approx(100.0) and v1["grade"] == "good"
    assert v11["timing_error_ms"] == pytest.approx(140.0) and v11["grade"] == "miss"
