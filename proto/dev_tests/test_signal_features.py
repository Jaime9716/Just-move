"""B3 y B4 con señales sintéticas simples (docs/06)."""

import math

import pytest

from jdproto import features, signal
from jdproto.config import G, merge_config


def _samples(n, fn, limb="right_hand", t0=0.0013, dt=0.005):
    out = []
    for i in range(n):
        t = t0 + i * dt
        a, g = fn(t)
        out.append({"t": t, "src": 1, "limb": limb, "a": a, "g": g})
    return out


def test_rest_is_stable():
    s = _samples(2000, lambda t: ([0.0, 0.0, G], [0.0, 0.0, 0.0]))  # pantalla arriba
    f = signal.process_samples(s)["right_hand"]
    for x in f[::50]:
        assert math.sqrt(sum(v * v for v in x["lin"])) < 1e-6
    assert signal.q_angle_between(f[0]["q"], f[-1]["q"]) < 1e-6


def test_initial_orientation_maps_gravity_to_up():
    for a in ([0.0, 0.0, G], [G, 0.0, 0.0], [0.0, -G, 0.0], [1.0, 2.0, 9.0]):
        q = signal.rotation_to_up(a)
        up = signal.q_rotate(q, [v / math.sqrt(sum(c * c for c in a)) for v in a])
        assert up[1] == pytest.approx(1.0, abs=1e-9)


def _rotation_test(axis, total=math.pi / 2, dur=1.0):
    """Gira `total` rad sobre `axis` (sensor) en `dur` s con acelerómetro y gyro coherentes."""
    w = total / dur
    q_true = [(1.0, 0.0, 0.0, 0.0)]

    def truth(t):
        ang = w * min(max(t - 0.5, 0.0), dur)
        return signal.q_from_axis_angle(axis, ang)

    def fn(t):
        q = truth(t)
        # accel = R(q)ᵀ·(0,G,0): rota el vector con la conjugada
        qc = (q[0], -q[1], -q[2], -q[3])
        a = signal.q_rotate(qc, (0.0, G, 0.0))
        omega = w if 0.5 <= t <= 0.5 + dur else 0.0
        return list(a), [axis[0] * omega, axis[1] * omega, axis[2] * omega]

    s = _samples(600, fn)
    f = signal.process_samples(s)["right_hand"]
    del q_true
    return signal.q_angle_between(f[-1]["q"], truth(s[-1]["t"]))


@pytest.mark.parametrize("axis", [(1.0, 0.0, 0.0), (0.0, 0.0, 1.0), (0.0, 1.0, 0.0)])
def test_rotation_90deg_error_below_5deg(axis):
    assert math.degrees(_rotation_test(axis)) <= 5.0


def test_lpf_step_response():
    cfg = merge_config()
    s = _samples(10, lambda t: ([0.0, G, 0.0], [0.0, 0.0, 0.0]) if t < 0.02 else ([0.0, G + 1.0, 0.0], [0.0, 0.0, 0.0]))
    f = signal.process_samples(s, cfg)["right_hand"]
    tau = 1.0 / (2 * math.pi * cfg["lpf_cutoff_hz"])
    k = 0.005 / (0.005 + tau)
    i = next(i for i, x in enumerate(s) if x["t"] >= 0.02)
    assert f[i]["a"][1] == pytest.approx(G + k, rel=1e-9)


def test_missing_fields_hold_last_value():
    s = _samples(5, lambda t: ([0.0, G, 0.0], [0.1, 0.0, 0.0]))
    s[3]["g"] = None
    s[3]["a"] = None
    f = signal.process_samples(s)["right_hand"]
    assert f[3]["a"] == pytest.approx(f[2]["a"])
    assert f[3]["g"][0] == pytest.approx(0.1)


def _pulse_session(times, amp=20.0, sigma=0.014, n=1200):
    def fn(t):
        a = sum(amp * math.exp(-0.5 * ((t - tp) / sigma) ** 2) for tp in times)
        return [0.0, G + a, 0.0], [0.0, 0.0, 0.0]
    return _samples(n, fn)


def test_peaks_within_one_sample():
    times = [1.0013, 2.0013, 2.5013, 4.1013]
    f = signal.process_samples(_pulse_session(times))["right_hand"]
    frames, peaks = features.compute_frames(f)
    assert len(peaks) == len(times)
    for (pt, _), tt in zip(peaks, times):
        assert abs(pt - tt) <= 0.005 + 1e-9
    marked = sorted(fr["peak_t_song"] for fr in frames.values() if fr["is_peak"])
    assert marked == [p for p, _ in peaks]


def _two_pulses(a1, a2, gap=0.1):
    def fn(t):
        a = a1 * math.exp(-0.5 * ((t - 1.0013) / 0.01) ** 2) + a2 * math.exp(-0.5 * ((t - 1.0013 - gap) / 0.01) ** 2)
        return [0.0, G + a, 0.0], [0.0, 0.0, 0.0]
    return signal.process_samples(_samples(600, fn))["right_hand"]


def test_peak_min_separation_keeps_larger():
    _, peaks = features.compute_frames(_two_pulses(15.0, 25.0))
    assert len(peaks) == 1 and abs(peaks[0][0] - 1.1013) <= 0.005 + 1e-9
    _, peaks = features.compute_frames(_two_pulses(25.0, 15.0))
    assert len(peaks) == 1 and abs(peaks[0][0] - 1.0013) <= 0.005 + 1e-9
    _, peaks = features.compute_frames(_two_pulses(20.0, 20.0, gap=0.2))
    assert len(peaks) == 2


def test_peak_below_min_ignored():
    f = signal.process_samples(_pulse_session([1.0013], amp=5.0))["right_hand"]
    assert features.compute_frames(f)[1] == []


def test_frames_grid_and_membership():
    s = _samples(100, lambda t: ([0.0, G, 0.0], [0.0, 0.0, 0.0]))
    f = signal.process_samples(s)["right_hand"]
    frames, _ = features.compute_frames(f)
    cfg = merge_config()
    for k, fr in frames.items():
        assert fr["t"] == k * cfg["frame_hop_s"]
        assert fr["dir"] == (0.0, 0.0, 0.0)
    # primera muestra t=0.0013 → trama k=-2 (centro −0.04, cubre hasta 0.01) existe; k=-3 no
    assert -2 in frames and -3 not in frames
    # última muestra 0.4963 → k=27 (0.54, desde 0.49) existe; k=28 no
    assert 27 in frames and 28 not in frames


def test_energy_and_dir_of_constant_accel():
    # 2 m/s² constantes hacia +X (mundo) con el móvil en reposo vertical (sensor Y arriba)
    s = _samples(400, lambda t: ([0.0, G, 0.0], [0.0, 0.0, 0.0]))
    for x in s[200:]:
        x["a"] = [2.0, G, 0.0]
    f = signal.process_samples(s)["right_hand"]
    frames, _ = features.compute_frames(f)
    fr = frames[50]  # t = 1.0 s, 0.0 s después del escalón
    fr2 = frames[60]
    assert fr["energy"] > 0.0
    assert fr2["dir"][0] > 0.9
