"""session_stats.py (P2) con sesiones sintéticas y, si están compiladas, con jd_record + phone_sim."""
import json
import os
import socket
import subprocess
import time
from pathlib import Path

import pytest

from t3_helpers import ROOT, run_tool, write_session


def stats_json(*args):
    p = run_tool("session_stats.py", *args, "--json")
    return p, json.loads(p.stdout)


def test_clean_session_passes(tmp_path):
    s = write_session(tmp_path / "ok.jsonl", devices=((1, "right_hand"), (2, "right_foot")))
    p, r = stats_json(s)
    assert p.returncode == 0
    assert r["pass"] and len(r["devices"]) == 2
    d = r["devices"][0]
    assert abs(d["rate_hz"] - 200) < 1 and d["gaps"] == 0 and d["non_monotonic"] == 0
    assert abs(d["rest_mag"] - 9.81) < 0.2


def test_gap_detected(tmp_path):
    s = write_session(tmp_path / "gap.jsonl", seconds=30, gaps=[(3.0, 120.0), (6.0, 60.0)])
    p, r = stats_json(s)
    assert p.returncode == 1 and not r["pass"]
    d = r["devices"][0]
    assert d["gaps"] == 2
    assert d["gap_list"][0]["ms"] > 115 and abs(d["gap_list"][0]["t"] - 3.0) < 0.01
    assert d["lost_est"] >= 30
    # tolerando 2 huecos pasa (las pérdidas siguen < 1 %)
    p, r = stats_json(s, "--max-gaps", "2")
    assert r["pass"]


def test_low_rate_and_losses(tmp_path):
    s = write_session(tmp_path / "low.jsonl", drop_every=3)       # pierde 1 de cada 3 → ~133 Hz
    _, r = stats_json(s)
    d = r["devices"][0]
    names = {c["name"]: c["ok"] for c in d["checks"]}
    assert names["tasa"] is False and names["pérdidas"] is False and names["huecos"] is True


def test_non_monotonic_and_range(tmp_path):
    s = write_session(tmp_path / "bad.jsonl", backstep_at=2.0, bad_accel_at=4.0)
    _, r = stats_json(s)
    d = r["devices"][0]
    assert d["non_monotonic"] >= 1
    assert d["accel_out_of_range"] == 1
    assert not r["pass"]


def test_rate_option_and_human_output(tmp_path):
    s = write_session(tmp_path / "r100.jsonl", rate=100.0)
    assert run_tool("session_stats.py", s).returncode == 1
    p = run_tool("session_stats.py", s, "--rate", "100")
    assert p.returncode == 0 and "APROBADO" in p.stdout


def test_synthetic_repo_sessions_and_glob():
    p = run_tool("session_stats.py", str(ROOT / "data/sessions/synthetic/*.jsonl"), "--json")
    r = json.loads(p.stdout)
    assert isinstance(r, list) and len(r) >= 3
    assert all("devices" in x for x in r)


def test_errors(tmp_path):
    bad = tmp_path / "bad.jsonl"
    bad.write_text('{"format":"otra"}\n', encoding="utf-8")
    p = run_tool("session_stats.py", bad)
    assert p.returncode == 2 and "ERROR" in p.stdout
    assert run_tool("session_stats.py", "--help").returncode == 0


def _free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _bin(name):
    d = Path(os.environ.get("JD_TOOLS_DIR", ROOT / "build" / "T3" / "debug"))
    for n in (name, name + ".exe"):
        if (d / n).is_file():
            return d / n
    pytest.skip(f"{name} no compilado (cd engine && CARGO_TARGET_DIR=../build/T3 cargo build -p jd_tools)")


def test_real_pipeline_jd_record_phone_sim(tmp_path):
    rec, sim = _bin("jd_record"), _bin("phone_sim")
    port = _free_port()
    out = tmp_path / "rec.jsonl"
    r = subprocess.Popen([str(rec), str(out), "--seconds", "5", "--code", "2468", "--port", str(port), "--note", "T3"],
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    try:
        time.sleep(0.5)
        subprocess.run([str(sim), f"127.0.0.1:{port}", "--code", "2468", "--seconds", "4"], capture_output=True,
                       timeout=20)
        r.wait(timeout=20)
    finally:
        if r.poll() is None:
            r.kill()
    p, res = stats_json(out)
    assert res["time_base"] == "mono"
    d = res["devices"][0]
    assert d["n"] > 600 and abs(d["rate_hz"] - 200) < 10
    assert d["non_monotonic"] == 0 and d["gaps"] == 0
