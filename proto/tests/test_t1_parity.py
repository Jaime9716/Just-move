"""T1 · paridad Python ↔ Rust: jd_judge (engine/jd_tools, release) contra judge_song, sesión a sesión,
con los datos sintéticos de B y con escenarios nuevos generados aquí (incluidos bordes de trama)."""
import math
import os

import pytest

from conftest import (G, SESSIONS, SONG, SYN, choreo, move_signal, punches, rest, run_jd_judge, smp, times,
                      write_session, write_song)
from jdproto.choreo import load_choreo
from jdproto.pipeline import judge_song
from jdproto.session import load_session

SIM_TOL = 1e-3
ERR_TOL = 0.05  # ms


def py_judge(song_dir, session, use_ref=True):
    ch = load_choreo(song_dir)
    ref = None
    name = ch.get("references", {}).get("imu")
    if use_ref and name:
        ref = load_session(os.path.join(song_dir, name))["samples"]
    return judge_song(ch, load_session(session)["samples"], ref, None)


def compare(rust, py, label):
    rr = rust["results"]
    assert len(rr) == len(py), f"{label}: {len(rr)} vs {len(py)} resultados"
    diffs = []
    for a, b in zip(rr, py):
        assert (a["step"], a["limb"]) == (b["step"], b["limb"]), label
        if a["grade"] != b["grade"]:
            diffs.append(f"paso {b['step']} {b['limb']}: rust {a['grade']} ({a['similarity']:.5f}) vs py {b['grade']} ({b['similarity']:.5f})")
        elif abs(a["similarity"] - b["similarity"]) > SIM_TOL or abs(a["timing_error_ms"] - b["timing_error_ms"]) > ERR_TOL:
            diffs.append(f"paso {b['step']} {b['limb']}: sim {a['similarity']:.5f}/{b['similarity']:.5f} err {a['timing_error_ms']:.3f}/{b['timing_error_ms']:.3f}")
    return diffs


@pytest.mark.parametrize("name", SESSIONS)
def test_parity_synthetic_sessions(jd_judge_bin, name):
    path = os.path.join(SYN, name + ".jsonl")
    rust = run_jd_judge(jd_judge_bin, SONG, path)
    py = py_judge(SONG, path)
    d = compare(rust, py, name)
    assert not d, "\n".join(d)
    s = rust["summary"]
    assert s["results"] == 27 and s["perfect"] + s["good"] + s["miss"] == 27
    assert s["has_reference"] is True


def test_parity_noref(jd_judge_bin, tmp_path):
    ch = load_choreo(SONG)
    ch["references"] = {}
    d = write_song(str(tmp_path / "noref"), ch)
    path = os.path.join(SYN, "perfect.jsonl")
    rust = run_jd_judge(jd_judge_bin, d, path)
    assert rust["summary"]["has_reference"] is False
    diffs = compare(rust, py_judge(d, path), "perfect_noref")
    assert not diffs, "\n".join(diffs)


class Rng:
    def __init__(self, seed):
        self.s = seed

    def uni(self):
        self.s = (self.s * 6364136223846793005 + 1442695040888963407) % (1 << 64)
        return (self.s >> 33) / float(1 << 31) - 1.0


def jitter(samples, rng, amp):
    return [dict(s, t=s["t"] + amp * rng.uni()) for s in samples]


def scenarios():
    """(nombre, choreo, referencia, jugador): datos nuevos, no los que B usó para ajustar."""
    hands = ["left_hand", "right_hand"]
    steps = [(2.0, 0.5, ["right_hand"], "accent"), (2.5, 0.5, ["left_foot"], "accent"), (3.0, 2.0, hands, "move"),
             (5.5, 1.5, ["left_foot"], "move"), (7.5, 0.5, ["left_hand", "right_foot"], "accent"), (8.0, 1.0, ["right_hand"], "move")]
    ch = choreo(steps)

    def perf(shift_acc=0.0, shift_mov=0.0, gain=1.0, phase=0.0013):
        s = punches("right_hand", [2.0 + shift_acc], 0, 10, phase)
        s += punches("left_foot", [2.5 + shift_acc], 0, 10, phase)
        s += [x for x in move_signal("left_hand", 3.0, 2.0, shift_mov, gain, 0, 10, phase)]
        s += punches("right_foot", [7.5 + shift_acc], 0, 10, phase)
        # right_hand: accent + moves (se suman en una sola serie)
        mv1 = move_signal("right_hand", 3.0, 2.0, shift_mov, gain, 0, 10, phase)
        mv2 = move_signal("right_hand", 8.0, 1.0, shift_mov, gain, 0, 10, phase)
        rh = [x for x in s if x["limb"] == "right_hand"]
        for a, b, c in zip(rh, mv1, mv2):
            a["a"] = [a["a"][0] + b["a"][0] + c["a"][0], a["a"][1], a["a"][2] + b["a"][2] + c["a"][2]]
        lh = [x for x in s if x["limb"] == "left_hand"]
        pl = punches("left_hand", [7.5 + shift_acc], 0, 10, phase)
        for a, b in zip(lh, pl):
            a["a"][1] = b["a"][1]
        lf = [x for x in s if x["limb"] == "left_foot"]
        for a, b in zip(lf, move_signal("left_foot", 5.5, 1.5, shift_mov, gain, 0, 10, phase)):
            a["a"] = [a["a"][0] + b["a"][0], a["a"][1], a["a"][2] + b["a"][2]]
        return s

    ref = perf()
    rng = Rng(7)
    yield "identico", ch, ref, perf()
    yield "tarde_60", ch, ref, perf(0.060, 0.060)
    yield "temprano_110", ch, ref, perf(-0.110, -0.110)
    yield "debil", ch, ref, perf(gain=0.3)
    yield "jitter", ch, ref, jitter(perf(0.02, 0.03), rng, 0.002)
    # muestras exactamente en múltiplos de 5 ms: caen en los bordes de las tramas (±50 ms de k·20 ms)
    yield "bordes", ch, perf(phase=0.0), perf(0.040, 0.020, phase=0.0)
    yield "sin_gyro", ch, ref, [dict(x, g=None) if i % 3 == 0 else x for i, x in enumerate(perf(0.01, 0.01))]
    yield "quieto", ch, ref, rest("left_hand", 0, 10) + rest("right_hand", 0, 10)
    # mesetas: pico con dos muestras iguales (desempates de máximo local)
    flat = punches("right_hand", [2.0], 0, 4, 0.0)
    for x in flat:
        if abs(x["t"] - 2.0) < 1e-9 or abs(x["t"] - 2.005) < 1e-9:
            x["a"][1] = G + 25.0
    yield "meseta", choreo([(2.0, 0.5, ["right_hand"], "accent")]), None, flat


@pytest.mark.parametrize("sc", list(scenarios()), ids=lambda sc: sc[0])
def test_parity_new_scenarios(jd_judge_bin, tmp_path, sc):
    name, ch, ref, player = sc
    song = write_song(str(tmp_path / "song"), ch, ref)
    sess = str(tmp_path / (name + ".jsonl"))
    write_session(sess, sorted(player, key=lambda x: x["t"]))
    rust = run_jd_judge(jd_judge_bin, song, sess)
    py = py_judge(song, sess)
    d = compare(rust, py, name)
    assert not d, name + ":\n" + "\n".join(d)
