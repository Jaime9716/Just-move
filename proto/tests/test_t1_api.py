"""T1 · caja negra de la API Python del juez v1.1 (judge_song, load_session, load_choreo, CLI --json)."""
import json
import os
import subprocess
import sys

import pytest

from conftest import (G, PROTO, SESSIONS, SONG, SYN, choreo, move_signal, punches, rest, smp, write_session)
from jdproto.choreo import load_choreo
from jdproto.pipeline import judge_song
from jdproto.session import load_session

H, F = "right_hand", "left_foot"


def one(ch, player, ref=None):
    r = judge_song(ch, player, ref, None)
    assert len(r) == 1
    return r[0]


def shape_ok(r):
    assert set(r) >= {"step", "limb", "grade", "similarity", "timing_error_ms"}
    assert r["grade"] in ("miss", "good", "perfect")
    assert 0.0 <= r["similarity"] <= 1.0


@pytest.mark.parametrize("off,want", [(0.0, "perfect"), (0.040, "perfect"), (0.080, "good"), (0.150, "miss"),
                                      (-0.040, "perfect"), (-0.080, "good"), (-0.150, "miss")])
def test_accent_hand(off, want):
    r = one(choreo([(2.0, 0.5, [H], "accent")]), punches(H, [2.0 + off]))
    shape_ok(r)
    assert r["grade"] == want, r
    if want != "miss":
        assert abs(r["timing_error_ms"] / 1e3 - off) <= 0.010
        assert abs(r["similarity"] - max(0.0, 1 - abs(r["timing_error_ms"] / 1e3) / 0.1)) < 1e-6
    else:
        assert r["similarity"] == 0 and r["timing_error_ms"] == 0


@pytest.mark.parametrize("off,hand,foot", [(0.060, "good", "perfect"), (0.120, "miss", "good"), (-0.120, "miss", "good"), (0.200, "miss", "miss")])
def test_accent_feet_scale(off, hand, foot):
    assert one(choreo([(2.0, 0.5, ["left_hand"], "accent")]), punches("left_hand", [2.0 + off]))["grade"] == hand
    rf = one(choreo([(2.0, 0.5, [F], "accent")]), punches(F, [2.0 + off]))
    assert rf["grade"] == foot, rf


def mv(limb):
    return choreo([(4.0, 2.0, [limb], "move")])


@pytest.mark.parametrize("limb", [H, F])
def test_move_identical_perfect(limb):
    s = move_signal(limb, 4.0, 2.0)
    r = one(mv(limb), s, s)
    assert r["grade"] == "perfect" and r["similarity"] > 0.99 and r["timing_error_ms"] == 0


def test_move_shifts():
    ref = move_signal(H, 4.0, 2.0)
    assert one(mv(H), move_signal(H, 4.0, 2.0, 0.040), ref)["grade"] == "perfect"
    assert one(mv(H), move_signal(H, 4.0, 2.0, 0.080), ref)["grade"] == "good"
    r = one(mv(H), move_signal(H, 4.0, 2.0, 0.150), ref)
    assert r["grade"] == "miss" and abs(r["timing_error_ms"] - 150) <= 10.001, r
    assert one(mv(H), move_signal(H, 4.0, 2.0, -0.150), ref)["grade"] == "miss"
    reff = move_signal(F, 4.0, 2.0)
    assert one(mv(F), move_signal(F, 4.0, 2.0, 0.140), reff)["grade"] == "good"
    assert one(mv(F), move_signal(F, 4.0, 2.0, 0.220), reff)["grade"] == "miss"


def test_move_still_weak_missing():
    ref = move_signal(H, 4.0, 2.0)
    assert one(mv(H), rest(H), ref)["grade"] == "miss"
    w = one(mv(H), move_signal(H, 4.0, 2.0, gain=0.1), ref)
    assert w["grade"] == "miss" and w["similarity"] < 0.5
    assert one(mv(H), rest("left_hand"), ref)["grade"] == "miss"
    assert one(mv(H), [], ref)["grade"] == "miss"


@pytest.mark.parametrize("gain", [0.3, 1.0, 3.0])
def test_move_noref_never_perfect(gain):
    r = one(mv(H), move_signal(H, 4.0, 2.0, gain=gain), None)
    assert r["grade"] != "perfect" and r["timing_error_ms"] == 0
    assert (r["grade"] == "good") == (r["similarity"] >= 0.5)
    # referencia solo de otra extremidad → este limb sin referencia
    r2 = one(mv(H), move_signal(H, 4.0, 2.0, gain=3.0), move_signal("left_hand", 4.0, 2.0))
    assert r2["grade"] == "good" and r2["timing_error_ms"] == 0


def test_order_step_then_limb():
    ch = choreo([(2.0, 0.5, ["right_foot", "right_hand", "left_hand"], "accent"), (2.0, 0.5, ["left_foot"], "accent"),
                 (3.0, 1.0, ["right_hand", "left_foot"], "move")])
    r = judge_song(ch, rest(H, 0, 5), None, None)
    assert [(x["step"], x["limb"]) for x in r] == [(0, "left_hand"), (0, "right_hand"), (0, "right_foot"), (1, "left_foot"),
                                                     (2, "right_hand"), (2, "left_foot")]


def test_emit_time_v11():
    from jdproto.judge import emit_time
    st = {"t": 2.0, "dur": 0.5, "limbs": [H], "kind": "accent"}
    assert abs(emit_time(st, H) - 2.75) < 1e-9
    assert abs(emit_time(st, F) - 2.85) < 1e-9


def test_robust_unordered_and_backwards():
    ch = choreo([(2.0, 0.5, [H], "accent"), (4.0, 0.5, [H], "accent")])
    base = punches(H, [2.0, 4.0], 0, 6)
    ref = judge_song(ch, base, None, None)
    un = list(base)
    for i in range(0, len(un) - 1, 2):
        un[i], un[i + 1] = un[i + 1], un[i]
    assert judge_song(ch, un, None, None) == ref  # se ordenan por t
    back = [dict(s, t=s["t"] - 1.0) if i > len(base) // 2 else s for i, s in enumerate(base)]
    r = judge_song(ch, back, None, None)
    assert len(r) == 2
    for x in r:
        shape_ok(x)


def test_samples_missing_gyro():
    s = [dict(x, g=None) for x in punches(H, [2.0])]
    assert one(choreo([(2.0, 0.5, [H], "accent")]), s)["grade"] == "perfect"


def test_judge_does_not_mutate_inputs():
    ch = choreo([(2.0, 0.5, [H], "accent")])
    s = punches(H, [2.0])
    ch_copy, s_copy = json.dumps(ch), json.dumps(s)
    judge_song(ch, s, s, None)
    assert json.dumps(s) == s_copy
    assert json.loads(ch_copy)["steps"] == ch["steps"]


# ------------------------------------------------------------------ carga


def test_load_session_and_choreo_shapes():
    s = load_session(os.path.join(SYN, "perfect.jsonl"))
    assert s["header"]["format"] == "jd-session"
    assert len(s["samples"]) > 1000
    x = s["samples"][0]
    assert set(x) >= {"t", "src", "limb", "a", "g"}
    c = load_choreo(SONG)
    assert c == load_choreo(os.path.join(SONG, "choreo.json"))
    assert len(c["steps"]) == 20


@pytest.mark.parametrize("text,frag", [
    ("", None),
    ('{"format":"x","version":1}\n', ":1:"),
    ('{"format":"jd-session","version":1}\n{"t":1,"limb":"left_hand","a":[1,2]}\n', ":2"),
    ('{"format":"jd-session","version":1}\n{"t":', ":2:"),
    ('{"format":"jd-session","version":1}\n{"limb":"left_hand","a":[1,2,3]}\n', ":2:"),
])
def test_load_session_errors(tmp_path, text, frag):
    from jdproto.session import SessionError
    p = tmp_path / "s.jsonl"
    p.write_text(text, encoding="utf-8")
    with pytest.raises(SessionError) as e:
        load_session(str(p))
    assert "s.jsonl" in str(e.value)
    if frag:
        assert frag in str(e.value)


@pytest.mark.parametrize("patch,path", [
    ({"format": "otro"}, "format"),
    ({"version": 2}, "version"),
    ({"steps": [{"t": 2, "dur": 0.5, "limbs": ["right_hand"], "kind": "accent"}, {"t": 1, "dur": 0.5, "limbs": ["right_hand"], "kind": "accent"}]}, "steps[1].t"),
    ({"steps": [{"t": 2, "dur": 0.5, "limbs": [], "kind": "accent"}]}, "steps[0].limbs"),
    ({"steps": [{"t": 2, "dur": 0.5, "limbs": ["cabeza"], "kind": "accent"}]}, "steps[0].limbs"),
    ({"steps": [{"t": 2, "dur": 0.5, "limbs": ["right_hand"], "kind": "golpe"}]}, "steps[0].kind"),
    ({"steps": [{"t": 2, "dur": -1, "limbs": ["right_hand"], "kind": "accent"}]}, "steps[0].dur"),
    ({"steps": [{"t": "x", "dur": 1, "limbs": ["right_hand"], "kind": "accent"}]}, "steps[0].t"),
])
def test_load_choreo_errors(tmp_path, patch, path):
    from jdproto.choreo import ChoreoError
    c = choreo([(2.0, 0.5, [H], "accent")])
    c.update(patch)
    (tmp_path / "choreo.json").write_text(json.dumps(c), encoding="utf-8")
    with pytest.raises(ChoreoError) as e:
        load_choreo(str(tmp_path))
    assert path in str(e.value)


def test_load_choreo_corrupt(tmp_path):
    from jdproto.choreo import ChoreoError
    (tmp_path / "choreo.json").write_text('{"format": "jd-choreo",,', encoding="utf-8")
    with pytest.raises(ChoreoError):
        load_choreo(str(tmp_path))
    (tmp_path / "choreo.json").write_text("", encoding="utf-8")
    with pytest.raises(ChoreoError):
        load_choreo(str(tmp_path))


# ------------------------------------------------------------------ CLI


def cli(*args):
    env = dict(os.environ, PYTHONPATH=PROTO)
    return subprocess.run([sys.executable, "-m", "jdproto.judge_cli", *args], capture_output=True, text=True, env=env, timeout=300)


def test_cli_json_matches_api_and_golden():
    for name in ("late_40ms", "foot_only"):
        r = cli(SONG, os.path.join(SYN, name + ".jsonl"), "--json")
        assert r.returncode == 0, r.stderr
        out = json.loads(r.stdout)
        api = judge_song(load_choreo(SONG), load_session(os.path.join(SYN, name + ".jsonl"))["samples"],
                         load_session(os.path.join(SONG, "ref_imu.jsonl"))["samples"], None)
        assert out == json.loads(json.dumps(api))
        with open(os.path.join(SYN, "golden", name + ".json"), encoding="utf-8") as f:
            assert out == json.load(f)


def test_cli_errors_and_text_mode(tmp_path):
    r = cli(str(tmp_path), os.path.join(SYN, "perfect.jsonl"), "--json")
    assert r.returncode != 0 and "error" in r.stderr
    r = cli(SONG, str(tmp_path / "nada.jsonl"))
    assert r.returncode != 0
    r = cli(SONG, os.path.join(SYN, "still.jsonl"))
    assert r.returncode == 0 and "Miss 27" in r.stdout


def test_golden_files_consistent():
    for name in SESSIONS + ["perfect_noref"]:
        with open(os.path.join(SYN, "golden", name + ".json"), encoding="utf-8") as f:
            g = json.load(f)
        assert len(g) == 27
        for x in g:
            shape_ok(x)
