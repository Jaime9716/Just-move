"""T1 · determinismo del prototipo: misma entrada → mismo resultado; regenerar los datos sintéticos da
archivos idénticos byte a byte."""
import filecmp
import json
import os
import subprocess
import sys

from conftest import PROTO, ROOT, SESSIONS, SONG, SYN, choreo, move_signal, punches
from jdproto.choreo import load_choreo
from jdproto.pipeline import judge_song
from jdproto.session import load_session


def test_judge_twice_identical():
    ch = load_choreo(SONG)
    ref = load_session(os.path.join(SONG, "ref_imu.jsonl"))["samples"]
    p = load_session(os.path.join(SYN, "noisy.jsonl"))["samples"]
    a = judge_song(ch, p, ref, None)
    b = judge_song(ch, p, ref, None)
    assert json.dumps(a) == json.dumps(b)  # incluye la representación exacta de los float


def test_input_order_of_limbs_irrelevant():
    ch = choreo([(2.0, 0.5, ["right_hand"], "accent"), (4.0, 2.0, ["left_foot"], "move")])
    s = punches("right_hand", [2.03], 0, 9) + move_signal("left_foot", 4.0, 2.0)
    s2 = sorted(s, key=lambda x: (x["limb"] != "left_foot", x["t"]))
    ref = move_signal("left_foot", 4.0, 2.0)
    assert json.dumps(judge_song(ch, s, ref, None)) == json.dumps(judge_song(ch, s2, ref, None))


def test_synth_regenerates_identical_files(tmp_path):
    out = tmp_path / "syn"
    r = subprocess.run([sys.executable, os.path.join(PROTO, "synth.py"), "--out", str(out)], capture_output=True, text=True,
                       timeout=600, cwd=ROOT)
    assert r.returncode == 0, r.stderr
    names = [n + ".jsonl" for n in SESSIONS]
    m, mis, err = filecmp.cmpfiles(SYN, out, names, shallow=False)
    assert not mis and not err, (mis, err)
    m, mis, err = filecmp.cmpfiles(os.path.join(SYN, "song_demo"), out / "song_demo", ["choreo.json", "ref_imu.jsonl"], shallow=False)
    assert not mis and not err, (mis, err)
    g = [n + ".json" for n in SESSIONS + ["perfect_noref"]]
    m, mis, err = filecmp.cmpfiles(os.path.join(SYN, "golden"), out / "golden", g, shallow=False)
    assert not mis and not err, (mis, err)
