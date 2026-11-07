"""T1 · utilidades de los tests de caja negra del prototipo Python (proto/tests/)."""
import json
import math
import os
import shutil
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
PROTO = os.path.dirname(HERE)
ROOT = os.path.dirname(PROTO)
SYN = os.path.join(ROOT, "data", "sessions", "synthetic")
SONG = os.path.join(SYN, "song_demo")
ENGINE = os.path.join(ROOT, "engine")
TARGET = os.path.join(ROOT, "build", "T1")
SESSIONS = ["perfect", "late_40ms", "late_150ms", "early_80ms", "wrong_moves", "still", "noisy", "foot_only"]

if PROTO not in sys.path:
    sys.path.insert(0, PROTO)

G = 9.80665
DT = 0.005
PHASE = 0.0013


def times(t0, t1, phase=PHASE):
    n = int(math.floor((t1 - t0) / DT))
    return [t0 + phase + i * DT for i in range(n)]


def smp(t, limb, a, g=(0.0, 0.0, 0.0), src=None):
    return {"t": t, "src": src if src is not None else {"left_hand": 1, "right_hand": 2, "left_foot": 3, "right_foot": 4}[limb],
            "limb": limb, "a": [float(x) for x in a], "g": None if g is None else [float(x) for x in g]}


def gauss(t, mu, s):
    return math.exp(-(t - mu) ** 2 / (2 * s * s))


def punches(limb, hits, t0=0.0, t1=4.0, phase=PHASE):
    return [smp(t, limb, (0.0, G + sum(20.0 * gauss(t, h, 0.012) for h in hits), 0.0)) for t in times(t0, t1, phase)]


def move_signal(limb, start, dur, shift=0.0, gain=1.0, t0=0.0, t1=9.0, phase=PHASE):
    out = []
    for t in times(t0, t1, phase):
        u = t - shift - start
        ax = az = 0.0
        if -0.2 < u < dur + 0.2:
            env = math.sin(math.pi * ((u + 0.2) / (dur + 0.4))) ** 2
            ax = gain * env * (7.0 * math.sin(2 * math.pi * 1.3 * u) + 3.0 * math.sin(2 * math.pi * 3.1 * u + 0.7))
            az = gain * env * 4.0 * math.cos(2 * math.pi * 2.2 * u + 1.1)
        out.append(smp(t, limb, (ax, G, az)))
    return out


def rest(limb, t0=0.0, t1=9.0):
    return [smp(t, limb, (0.0, G, 0.0)) for t in times(t0, t1)]


def choreo(steps, ref=None):
    return {"format": "jd-choreo", "version": 1,
            "song": {"title": "t1", "artist": "", "audio": "audio.ogg", "video": None, "bpm": 120.0, "offset_s": 0.0, "duration_s": 60.0},
            "steps": [{"t": t, "dur": d, "limbs": list(l), "icon": "x", "kind": k} for (t, d, l, k) in steps],
            "references": {"imu": ref} if ref else {}}


def write_session(path, samples, note="t1"):
    limbs = sorted({s["limb"] for s in samples})
    hdr = {"format": "jd-session", "version": 1, "time_base": "song", "song_id": None, "recorded_at": "2026-11-07T00:00:00Z",
           "note": note, "devices": [{"source_id": s, "name": l, "kind": "phone", "limb": l}
                                     for l, s in {x["limb"]: x["src"] for x in samples}.items()]}
    with open(path, "w", encoding="utf-8") as f:
        f.write(json.dumps(hdr) + "\n")
        for s in samples:
            o = {"t": float(f"{s['t']:.9f}"), "src": s["src"], "limb": s["limb"]}
            if s.get("a") is not None:
                o["a"] = [float(f"{v:.6g}") for v in s["a"]]
            if s.get("g") is not None:
                o["g"] = [float(f"{v:.6g}") for v in s["g"]]
            f.write(json.dumps(o, separators=(",", ":")) + "\n")
    return limbs


def write_song(d, ch, ref_samples=None):
    os.makedirs(d, exist_ok=True)
    if ref_samples is not None:
        write_session(os.path.join(d, "ref_imu.jsonl"), ref_samples, "ref")
        ch = dict(ch, references={"imu": "ref_imu.jsonl"})
    with open(os.path.join(d, "choreo.json"), "w", encoding="utf-8") as f:
        json.dump(ch, f)
    return d


@pytest.fixture(scope="session")
def jd_judge_bin():
    """Compila jd_judge (release, CARGO_TARGET_DIR=build/T1) y devuelve la ruta; skip si no hay cargo."""
    cargo = shutil.which("cargo")
    if cargo is None:
        pytest.skip("cargo no disponible")
    env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
    r = subprocess.run([cargo, "build", "--release", "-q", "-p", "jd_tools", "--bin", "jd_judge"], cwd=ENGINE, env=env,
                       capture_output=True, text=True, timeout=900)
    if r.returncode != 0:
        pytest.fail("no compila jd_judge:\n" + r.stderr[-3000:])
    exe = os.path.join(TARGET, "release", "jd_judge" + (".exe" if os.name == "nt" else ""))
    assert os.path.exists(exe)
    return exe


def run_jd_judge(exe, song_dir, session):
    r = subprocess.run([exe, song_dir, session, "--json"], capture_output=True, text=True, timeout=300)
    assert r.returncode == 0, r.stderr
    return json.loads(r.stdout)
