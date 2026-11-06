"""Grabador de coreografías con una sesión sintética (móvil en la mano derecha y en el pie izquierdo)."""
import json

import numpy as np
import pytest

import choreo_record as cr
import choreo_validate as cv
import jd_media

G = 9.80665
BPM, OFFSET, DUR = 120.0, 1.0, 20.0


def make_package(tmp_path, steps=None):
    pkg = tmp_path / "song"
    pkg.mkdir()
    c = jd_media.make_choreo("S", "A", BPM, OFFSET, DUR, steps or [], video=None)
    cv.save(pkg / "choreo.json", c)
    return pkg


def make_session(path, rec_offset, hits, time_base="mono", rate=100.0, seed=0):
    """hits: {limb: [t_song,...]}; t_sesión = t_song - rec_offset."""
    rng = np.random.default_rng(seed)
    devs = [{"source_id": 1, "name": "Móvil A", "kind": "phone", "limb": "right_hand"},
            {"source_id": 2, "name": "Móvil B", "kind": "phone", "limb": "left_foot"}]
    lines = [json.dumps({"format": "jd-session", "version": 1, "time_base": time_base, "song_id": None,
                         "recorded_at": "2026-11-06T20:00:00Z", "note": "prueba", "devices": devs})]
    t0, t1 = 0.0, DUR - rec_offset + 1.0
    for src, limb in ((1, "right_hand"), (2, "left_foot")):
        for k in range(int((t1 - t0) * rate)):
            ts = t0 + k / rate
            ss = ts + rec_offset
            a = np.array([0.0, G, 0.0]) + 0.2 * rng.standard_normal(3)
            for h in hits.get(limb, []):
                a += np.array([18.0, 4.0, 0.0]) * np.exp(-0.5 * ((ss - h) / 0.02) ** 2)
            lines.append(json.dumps({"t": round(ts, 6), "src": src, "limb": limb,
                                     "a": [round(x, 4) for x in a], "g": [0.0, 0.0, 0.0]}))
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def test_record_proposes_quantized_steps(tmp_path):
    pkg = make_package(tmp_path)
    sess = tmp_path / "s.jsonl"
    # Grabación empezada 2 s antes de la música → offset = -2.
    make_session(sess, -2.0, {"right_hand": [3.0, 4.03, 4.96, 7.5], "left_foot": [6.5, 7.48]})
    assert cr.main([str(sess), "--song", str(pkg), "--offset", "-2"]) == 0
    c = cv.load(pkg / "choreo.json")
    got = [(s["t"], s["limbs"]) for s in c["steps"]]
    assert got == [(3.0, ["right_hand"]), (4.0, ["right_hand"]), (5.0, ["right_hand"]),
                   (6.5, ["left_foot"]), (7.5, ["right_hand", "left_foot"])]
    assert all(s["kind"] == "accent" and s["auto"] for s in c["steps"])
    assert c["references"] == {"imu": "ref_imu.jsonl"}
    assert (pkg / "choreo.json.bak").exists()

    lines = (pkg / "ref_imu.jsonl").read_text(encoding="utf-8").splitlines()
    h = json.loads(lines[0])
    assert h["format"] == "jd-session" and h["time_base"] == "song" and h["song_id"] == "song"
    ts = [json.loads(l)["t"] for l in lines[1:]]
    assert min(ts) >= 0.0 and max(ts) <= DUR
    assert abs(min(ts)) < 0.011  # la sesión empieza en t_song = -2 → se recorta en 0


def test_grid_and_max_dev(tmp_path):
    pkg = make_package(tmp_path)
    sess = tmp_path / "s.jsonl"
    make_session(sess, 0.0, {"right_hand": [3.25, 5.0]}, time_base="song")
    # Rejilla de negras: 3.25 está a 0.5 de rejilla → descartado.
    assert cr.main([str(sess), "--song", str(pkg)]) == 0
    assert [s["t"] for s in cv.load(pkg / "choreo.json")["steps"]] == [5.0]
    # Corcheas: entra 3.25.
    assert cr.main([str(sess), "--song", str(pkg), "--grid", "0.5"]) == 0
    assert [s["t"] for s in cv.load(pkg / "choreo.json")["steps"]] == [3.25, 5.0]


def test_merge_keeps_existing(tmp_path):
    existing = [{"t": 4.0, "dur": 1.0, "limbs": ["right_hand"], "icon": "wave", "kind": "move"},
                {"t": 10.0, "dur": 0.5, "limbs": ["left_hand"], "icon": "x", "kind": "accent"}]
    pkg = make_package(tmp_path, existing)
    sess = tmp_path / "s.jsonl"
    make_session(sess, 0.0, {"right_hand": [4.0, 6.0], "left_foot": [4.0]}, time_base="song")
    assert cr.main([str(sess), "--song", str(pkg), "--merge"]) == 0
    steps = cv.load(pkg / "choreo.json")["steps"]
    assert steps[0] == existing[0]
    assert (steps[1]["t"], steps[1]["limbs"]) == (4.0, ["left_foot"])
    assert (steps[2]["t"], steps[2]["limbs"]) == (6.0, ["right_hand"])
    assert steps[3] == existing[1]


def test_mono_requires_offset_and_dry_run(tmp_path, capsys):
    pkg = make_package(tmp_path)
    sess = tmp_path / "s.jsonl"
    make_session(sess, 0.0, {"right_hand": [3.0]})
    assert cr.main([str(sess), "--song", str(pkg)]) == 1
    assert "--offset" in capsys.readouterr().err
    before = (pkg / "choreo.json").read_text()
    assert cr.main([str(sess), "--song", str(pkg), "--offset", "0", "--dry-run"]) == 0
    assert (pkg / "choreo.json").read_text() == before and not (pkg / "ref_imu.jsonl").exists()


def test_bad_session(tmp_path, capsys):
    pkg = make_package(tmp_path)
    p = tmp_path / "bad.jsonl"
    p.write_text('{"format":"jd-session","version":1,"time_base":"song","devices":[]}\n{"t": oops}\n')
    assert cr.main([str(p), "--song", str(pkg)]) == 1
    assert "bad.jsonl:2:" in capsys.readouterr().err


def test_peak_rules():
    t = np.arange(0, 2, 0.01)
    mag = np.zeros_like(t)
    mag[50], mag[60], mag[150] = 10.0, 12.0, 7.0  # 0.5 y 0.6 a < 0.15 s: gana el mayor; 7 < PEAK_MIN
    assert cr.detect_peaks(t, mag) == [(pytest.approx(0.6), 12.0)]
