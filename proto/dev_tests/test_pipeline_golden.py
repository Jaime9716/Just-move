"""Punta a punta con los datos sintéticos: golden, comportamientos de docs/06, CLI y config."""

import io
import json
import os
from contextlib import redirect_stdout

import pytest

SYN = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "data", "sessions", "synthetic")
from jdproto import synth
from jdproto.choreo import ChoreoError, load_choreo, validate_choreo
from jdproto.config import merge_config
from jdproto.judge_cli import main as cli_main
from jdproto.pipeline import judge_song, summarize
from jdproto.session import SessionError, load_session, parse_session_lines

SONG = os.path.join(SYN, "song_demo")
NAMES = list(synth.SESSIONS)


@pytest.fixture(scope="module")
def data():
    if not os.path.exists(os.path.join(SONG, "choreo.json")):
        pytest.skip("faltan datos sintéticos: ejecuta python3 proto/synth.py")
    c = load_choreo(SONG)
    ref = load_session(os.path.join(SONG, "ref_imu.jsonl"))["samples"]
    cache = {}

    def run(name, variant=None, use_ref=True):
        key = (name, variant, use_ref)
        if key not in cache:
            p = load_session(os.path.join(SYN, name + ".jsonl"))["samples"]
            cache[key] = judge_song(c, p, ref if use_ref else None, {"variant": variant} if variant else None)
        return cache[key]
    return c, run


@pytest.mark.parametrize("name", NAMES + ["perfect_noref"])
def test_matches_golden(data, name):
    _, run = data
    res = run("perfect", use_ref=False) if name == "perfect_noref" else run(name)
    with open(os.path.join(SYN, "golden", name + ".json"), encoding="utf-8") as f:
        gold = json.load(f)
    assert res == gold  # determinista: igual bit a bit


def _frac(res, grades):
    return sum(r["grade"] in grades for r in res) / len(res)


def test_perfect_mostly_perfect(data):
    assert _frac(data[1]("perfect"), {"perfect"}) > 0.5


def test_late_40_good_or_perfect(data):
    assert _frac(data[1]("late_40ms"), {"perfect", "good"}) == 1.0


def test_still_all_miss(data):
    assert _frac(data[1]("still"), {"miss"}) == 1.0


def test_wrong_mostly_miss(data):
    assert _frac(data[1]("wrong_moves"), {"miss"}) > 0.5


def test_noisy_mostly_hits(data):
    assert _frac(data[1]("noisy"), {"perfect", "good"}) > 0.8


def test_late_150_accents_miss(data):
    c, run = data
    res = run("late_150ms")
    hands = [r for r in res if c["steps"][r["step"]]["kind"] == "accent" and r["limb"].endswith("hand")]
    assert all(r["grade"] == "miss" for r in hands)


def test_late_150_v1_moves_not_miss_documented(data):
    """Problema conocido de v1 (proto/ALGORITHM.md): los moves 150 ms tarde dan Good."""
    c, run = data
    moves = [r for r in run("late_150ms") if c["steps"][r["step"]]["kind"] == "move"]
    assert any(r["grade"] != "miss" for r in moves)


def test_late_150_v1b_all_miss(data):
    assert _frac(data[1]("late_150ms", "v1b"), {"miss"}) == 1.0


def test_v1b_keeps_good_sessions(data):
    _, run = data
    assert summarize(run("perfect", "v1b")) == summarize(run("perfect"))
    assert _frac(run("late_40ms", "v1b"), {"perfect", "good"}) == 1.0


def test_foot_only(data):
    c, run = data
    for r in run("foot_only"):
        if r["limb"] == "right_foot":
            assert r["grade"] != "miss"
        else:
            assert r["grade"] == "miss"


def test_cli_json_equals_golden(data):
    buf = io.StringIO()
    with redirect_stdout(buf):
        rc = cli_main([SONG, os.path.join(SYN, "still.jsonl"), "--json"])
    assert rc == 0
    with open(os.path.join(SYN, "golden", "still.json"), encoding="utf-8") as f:
        assert json.loads(buf.getvalue()) == json.load(f)


def test_cli_table(data):
    buf = io.StringIO()
    with redirect_stdout(buf):
        rc = cli_main([SONG, os.path.join(SYN, "foot_only.jsonl")])
    assert rc == 0 and "Perfect" in buf.getvalue()


def test_synth_deterministic():
    c = synth.make_demo_choreo()
    a = synth.render_limb(c, "right_hand", 7, 8, 3.0)
    b = synth.render_limb(c, "right_hand", 7, 8, 3.0)
    assert a == b


def test_synth_rest_noise_level():
    import numpy as np
    c = synth.make_demo_choreo()
    s = synth.render_limb(c, "right_hand", 7, 8, 1.5)  # antes del primer paso (t=2)
    acc = np.array([x["a"] for x in s])
    norm = np.linalg.norm(acc, axis=1)
    assert abs(norm.mean() - 9.80665) < 0.3
    assert acc[:, 2].mean() > 8.5  # pantalla hacia arriba


def test_synth_gyro_coherent_with_accel():
    """Integrando solo el giroscopio, la gravedad predicha sigue al acelerómetro."""
    import numpy as np
    from jdproto import signal as sg
    c = synth.make_demo_choreo()
    s = synth.render_limb(c, "right_hand", 7, 8, 9.0, acc_noise=0.0, gyro_noise=0.0)
    # mira el wave (7–9 s) donde hay mucha rotación; la aceleración lineal es pequeña a baja frecuencia
    seg = [x for x in s if 6.5 <= x["t"] <= 9.0]
    q = sg.rotation_to_up(seg[0]["a"])
    for prev, cur in zip(seg, seg[1:]):
        dt = cur["t"] - prev["t"]
        w = [(a + b) / 2 for a, b in zip(prev["g"], cur["g"])]
        wn = sum(v * v for v in w) ** 0.5
        if wn > 0:
            q = sg.q_normalize(sg.q_mul(q, sg.q_from_axis_angle([v / wn for v in w], wn * dt)))
    # comprobación fuerte: tras el move el móvil vuelve casi a reposo y la gravedad estimada coincide
    a_end = np.array(seg[-1]["a"]) / np.linalg.norm(seg[-1]["a"])
    qc = (q[0], -q[1], -q[2], -q[3])
    ang = np.degrees(np.arccos(np.clip(np.array(sg.q_rotate(qc, (0.0, 1.0, 0.0))) @ a_end, -1, 1)))
    assert ang < 3.0


def test_config_merge():
    cfg = merge_config({"PEAK_MIN": 9, "variant": "v1b"})
    assert cfg["peak_min"] == 9.0 and cfg["lag_search_scale"] == 2.0
    with pytest.raises(ValueError):
        merge_config({"nope": 1})


def test_choreo_validation_messages():
    c = synth.make_demo_choreo()
    validate_choreo(json.loads(json.dumps(c)))
    bad = json.loads(json.dumps(c))
    bad["steps"][3]["t"] = 0.0
    with pytest.raises(ChoreoError, match=r"steps\[3\]\.t: debe ser >= steps\[2\]\.t"):
        validate_choreo(bad)
    bad = json.loads(json.dumps(c))
    bad["steps"][0]["limbs"] = []
    with pytest.raises(ChoreoError, match=r"steps\[0\]\.limbs"):
        validate_choreo(bad)


def test_session_parse():
    lines = ['{"format":"jd-session","version":1,"time_base":"song","devices":[{"source_id":1,"limb":"left_hand"}]}',
             "# comentario", "", '{"t":1.5,"src":1,"a":[0,9.8,0]}']
    s = parse_session_lines(lines)
    assert s["samples"][0]["limb"] == "left_hand" and s["samples"][0]["g"] is None
    with pytest.raises(SessionError):
        parse_session_lines(['{"format":"x"}'])
