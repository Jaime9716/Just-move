"""Datos sintéticos de un MÓVIL Android en la mano (o atado al tobillo) para probar el juez.

Modelo físico (todo con semilla fija, reproducible):
  * Cada móvil tiene una trayectoria de aceleración en el mundo a_w(t) (Y arriba, X a la derecha del
    jugador, −Z hacia la pantalla/TV) y una velocidad angular en el marco del sensor ω(t).
  * La orientación q(t) (mundo ← sensor) se integra a 1 kHz desde q0 con ω(t), así que el giroscopio
    es coherente con la orientación usada para el acelerómetro.
  * Acelerómetro = R(q)ᵀ·(a_w + (0, G, 0)) + sesgo + ruido (σ 0.05 m/s²), saturado a ±8 g.
    Giroscopio = ω + sesgo + ruido (σ 0.01 rad/s), saturado a ±2000 °/s. 200 Hz con jitter.
  * Masa y gestos de un móvil (~200 g) en la mano: golpes con frenada de ~25 m/s² y giro de muñeca de
    ~4 rad/s; movimientos continuos de 6–8 m/s² a 1–2 Hz; reposo con balanceo lento (0.2 m/s²) y
    temblor fisiológico (~9 Hz). Mano: pantalla hacia arriba, parte superior hacia la TV, inclinado
    ~20° hacia el jugador. Pie: móvil vertical sujeto a la espinilla, pantalla hacia delante.
  * Humanización: cada gesto lleva un error de tiempo N(0, 8 ms) (golpes) o N(0, 15 ms) (moves) y una
    escala de amplitud U(0.9, 1.1).
"""

import math

import numpy as np

from .config import G

RATE_HZ = 200.0
GRID_DT = 0.001
ACC_SAT = 8.0 * G
GYRO_SAT = math.radians(2000.0)

# ----------------------------------------------------------------- coreografía de demo

def make_demo_choreo():
    """~30 s a 120 BPM (beat cada 0.5 s), golpes (accent) y movimientos (move), manos y pie derecho."""
    H2 = ["left_hand", "right_hand"]
    steps = [
        (2.0, 0.5, ["right_hand"], "punch_up", "accent"),
        (3.0, 0.5, ["left_hand"], "punch_up", "accent"),
        (4.0, 0.5, ["right_hand"], "punch_forward", "accent"),
        (5.0, 0.5, ["left_hand"], "punch_forward", "accent"),
        (6.0, 0.5, H2, "punch_up", "accent"),
        (7.0, 2.0, H2, "wave", "move"),
        (9.5, 1.5, ["right_hand"], "pump", "move"),
        (11.5, 2.0, ["left_hand"], "circle", "move"),
        (14.0, 0.5, ["right_hand"], "punch_up", "accent"),
        (14.5, 0.5, ["left_hand"], "punch_up", "accent"),
        (15.0, 2.0, H2, "swing", "move"),
        (17.5, 0.5, ["right_hand"], "punch_forward", "accent"),
        (18.0, 1.5, H2, "pump", "move"),
        (20.0, 0.5, ["right_foot"], "stomp", "accent"),
        (21.0, 0.5, ["right_foot"], "stomp", "accent"),
        (22.0, 2.0, ["right_foot"], "side_step", "move"),
        (24.5, 0.5, H2, "punch_up", "accent"),
        (25.0, 2.0, H2, "wave", "move"),
        (27.5, 0.5, ["right_foot"], "stomp", "accent"),
        (28.0, 0.5, H2, "punch_forward", "accent"),
    ]
    return {
        "format": "jd-choreo",
        "version": 1,
        "song": {"title": "Demo sintética", "artist": "jdproto", "audio": "audio.ogg", "video": None,
                 "bpm": 120.0, "offset_s": 0.0, "duration_s": 30.0},
        "steps": [{"t": t, "dur": d, "limbs": list(l), "icon": icon, "kind": kind}
                  for (t, d, l, icon, kind) in steps],
        "references": {"imu": "ref_imu.jsonl"},
    }


# ----------------------------------------------------------------- primitivas de movimiento

def _g(t, mu, sigma):
    return np.exp(-0.5 * ((t - mu) / sigma) ** 2)


def _env(t, t0, dur, ramp=0.12):
    """Ventana con rampas de coseno (0 fuera de [t0, t0+dur])."""
    x = np.zeros_like(t)
    r = min(ramp, dur / 2.0)
    inside = (t >= t0) & (t <= t0 + dur)
    x[inside] = 1.0
    up = (t >= t0) & (t < t0 + r)
    x[up] = 0.5 - 0.5 * np.cos(math.pi * (t[up] - t0) / r)
    dn = (t > t0 + dur - r) & (t <= t0 + dur)
    x[dn] = 0.5 - 0.5 * np.cos(math.pi * (t0 + dur - t[dn]) / r)
    return x


def _vec(arr3, scalar):
    return np.outer(scalar, np.asarray(arr3, dtype=np.float64))


PUNCH_DIRS = {"punch_up": (0.0, 1.0, 0.0), "punch_forward": (0.0, 0.0, -1.0)}


def m_punch(t, tb, s, icon, limb):
    d = PUNCH_DIRS.get(icon, (0.0, 1.0, 0.0))
    A = 26.0 * s
    prof = A * (0.55 * _g(t, tb - 0.075, 0.03) - _g(t, tb, 0.014))
    ret = 3.0 * s * (_g(t, tb + 0.20, 0.06) - _g(t, tb + 0.40, 0.06))  # recoger el brazo
    a = _vec(d, prof - ret)
    sign = 1.0 if limb == "right_hand" else -1.0  # pronación espejo
    w = _vec((0.0, 1.0, 0.0), sign * 4.0 * s * (_g(t, tb - 0.06, 0.03) - 0.75 * _g(t, tb + 0.08, 0.04)))
    w += _vec((1.0, 0.0, 0.0), 1.2 * s * (_g(t, tb - 0.05, 0.04) - _g(t, tb + 0.15, 0.04)))
    return a, w


def m_reach(t, tb, s, icon, limb):
    """Gesto equivocado para un golpe: empujar despacio (sin pico)."""
    d = PUNCH_DIRS.get(icon, (0.0, 1.0, 0.0))
    a = _vec(d, 4.0 * s * (_g(t, tb - 0.12, 0.08) - _g(t, tb + 0.12, 0.08)))
    w = _vec((1.0, 0.0, 0.0), 0.6 * s * (_g(t, tb - 0.1, 0.08) - _g(t, tb + 0.1, 0.08)))
    return a, w


def m_stomp(t, tb, s, icon, limb):
    ay = s * (5.0 * _g(t, tb - 0.33, 0.05) - 5.0 * _g(t, tb - 0.20, 0.05)
              - 6.0 * _g(t, tb - 0.07, 0.035) + 38.0 * _g(t, tb, 0.010))
    a = _vec((0.0, 1.0, 0.0), ay)
    w = _vec((1.0, 0.0, 0.0), 2.0 * s * (_g(t, tb - 0.30, 0.05) - _g(t, tb - 0.12, 0.05)))
    return a, w


def m_tap(t, tb, s, icon, limb):
    """Gesto equivocado para un pisotón: apoyar suave."""
    a = _vec((0.0, 1.0, 0.0), 3.5 * s * (_g(t, tb - 0.1, 0.06) - _g(t, tb + 0.05, 0.06)))
    w = _vec((1.0, 0.0, 0.0), 0.5 * s * (_g(t, tb - 0.1, 0.06) - _g(t, tb + 0.05, 0.06)))
    return a, w


def m_wave(t, t0, dur, s, limb):
    e = _env(t, t0, dur)
    p = 2.0 * math.pi * (t - t0)
    a = np.stack([7.0 * s * np.sin(p) * e, 2.0 * s * np.sin(2.0 * p) * e, np.zeros_like(t)], axis=1)
    w = _vec((0.0, 1.0, 0.0), 1.8 * s * np.cos(p) * e)
    return a, w


def m_pump(t, t0, dur, s, limb):
    e = _env(t, t0, dur)
    p = 2.0 * math.pi * 2.0 * (t - t0)
    a = _vec((0.0, 1.0, 0.0), 8.0 * s * np.sin(p) * e)
    w = _vec((1.0, 0.0, 0.0), 2.0 * s * np.cos(p) * e)
    return a, w


def m_circle(t, t0, dur, s, limb):
    e = _env(t, t0, dur)
    p = 2.0 * math.pi * (t - t0)
    a = np.stack([6.0 * s * np.cos(p) * e, 6.0 * s * np.sin(p) * e, np.zeros_like(t)], axis=1)
    w = _vec((0.0, 0.0, 1.0), 1.0 * s * np.sin(p) * e) + _vec((0.0, 1.0, 0.0), 0.8 * s * np.cos(p) * e)
    return a, w


def m_swing(t, t0, dur, s, limb):
    e = _env(t, t0, dur)
    p = 2.0 * math.pi * (t - t0)
    a = np.stack([np.zeros_like(t), 2.5 * s * np.cos(2.0 * p) * e, 6.5 * s * np.sin(p) * e], axis=1)
    w = _vec((1.0, 0.0, 0.0), 2.2 * s * np.cos(p) * e)
    return a, w


def m_side_step(t, t0, dur, s, limb):
    a = np.zeros((len(t), 3))
    w = np.zeros((len(t), 3))
    n = int(round(dur / 0.5))
    for i in range(n):
        b = t0 + 0.5 * i
        side = 1.0 if i % 2 == 0 else -1.0
        a[:, 1] += s * (4.0 * _g(t, b - 0.20, 0.05) - 4.0 * _g(t, b - 0.08, 0.05) + 14.0 * _g(t, b, 0.015))
        a[:, 0] += side * s * 3.0 * (_g(t, b - 0.20, 0.06) - _g(t, b - 0.04, 0.06))
        w[:, 0] += s * 1.5 * (_g(t, b - 0.2, 0.05) - _g(t, b - 0.05, 0.05))
    return a, w


def m_shuffle(t, t0, dur, s, limb):
    """Gesto equivocado para side_step: balanceo lento sin apoyos."""
    e = _env(t, t0, dur)
    p = 2.0 * math.pi * 0.5 * (t - t0)
    a = _vec((1.0, 0.0, 0.0), 2.0 * s * np.sin(p) * e)
    w = _vec((0.0, 0.0, 1.0), 0.4 * s * np.cos(p) * e)
    return a, w


ACCENTS = {"punch_up": m_punch, "punch_forward": m_punch, "stomp": m_stomp}
MOVES = {"wave": m_wave, "pump": m_pump, "circle": m_circle, "swing": m_swing, "side_step": m_side_step}
WRONG = {"punch_up": "reach", "punch_forward": "reach", "stomp": "tap",
         "wave": "pump", "pump": "swing", "circle": "swing", "swing": "pump", "side_step": "shuffle"}
EXTRA = {"reach": m_reach, "tap": m_tap, "shuffle": m_shuffle}


# ----------------------------------------------------------------- cuaterniones vectorizados

def _qmul(a, b):
    aw, ax, ay, az = a
    bw, bx, by, bz = b
    return (aw * bw - ax * bx - ay * by - az * bz,
            aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw)


def _qaxis(axis, ang):
    n = math.sqrt(sum(c * c for c in axis))
    h = 0.5 * ang
    s = math.sin(h) / n
    return (math.cos(h), axis[0] * s, axis[1] * s, axis[2] * s)


def _rotate_inv(q, v):
    """R(q)ᵀ·v para arrays: q (N,4), v (N,3)."""
    w = q[:, 0:1]
    u = -q[:, 1:4]  # conjugado
    t = 2.0 * np.cross(u, v)
    return v + w * t + np.cross(u, t)


def _integrate(q0, w_grid, dt):
    n = len(w_grid)
    qs = np.empty((n, 4))
    q = q0
    qs[0] = q
    for i in range(1, n):
        wx, wy, wz = (w_grid[i - 1] + w_grid[i]) * 0.5
        wn = math.sqrt(wx * wx + wy * wy + wz * wz)
        th = wn * dt
        if th > 1e-12:
            s = math.sin(0.5 * th) / wn
            q = _qmul(q, (math.cos(0.5 * th), wx * s, wy * s, wz * s))
            nq = math.sqrt(q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3])
            q = (q[0] / nq, q[1] / nq, q[2] / nq, q[3] / nq)
        qs[i] = q
    return qs


# ----------------------------------------------------------------- render

DEVICES = {  # source_id (orden de emparejado del protocolo), fase de reloj
    "right_hand": (1, 0.0021),
    "left_hand": (2, 0.0013),
    "right_foot": (3, 0.0037),
    "left_foot": (4, 0.0029),
}


def initial_orientation(limb, rng):
    tilt = math.radians(20.0 + rng.normal(0.0, 3.0))
    yaw = math.radians(rng.normal(0.0, 3.0))
    q_yaw = _qaxis((0.0, 1.0, 0.0), yaw)
    if limb.endswith("hand"):
        # pantalla arriba, parte superior hacia la TV, inclinado hacia el jugador
        return _qmul(q_yaw, _qaxis((1.0, 0.0, 0.0), -math.pi / 2.0 + tilt))
    # pie: vertical en la espinilla, pantalla hacia delante (180° sobre Y)
    roll = math.radians(rng.normal(0.0, 4.0))
    return _qmul(q_yaw, _qmul(_qaxis((0.0, 1.0, 0.0), math.pi), _qaxis((0.0, 0.0, 1.0), roll)))


def plan_actions(choreo, limb, rng, mode="good"):
    """Lista de (fn, kind, t, dur, scale, icon) para una extremidad."""
    acts = []
    for st in choreo["steps"]:
        if limb not in st["limbs"]:
            continue
        icon = st["icon"]
        name = WRONG.get(icon, icon) if mode == "wrong" else icon
        scale = rng.uniform(0.9, 1.1)
        if st["kind"] == "accent":
            dt = rng.normal(0.0, 0.008)
            fn = ACCENTS.get(name) or EXTRA[name]
            acts.append((fn, "accent", st["t"] + dt, st["dur"], scale, icon))
        else:
            dt = rng.normal(0.0, 0.015)
            fn = MOVES.get(name) or EXTRA[name]
            acts.append((fn, "move", st["t"] + dt, st["dur"], scale, icon))
    return acts


def motion_at(t, limb, acts, idle):
    """a_w (N,3) y ω (N,3) en los instantes t."""
    a = np.zeros((len(t), 3))
    w = np.zeros((len(t), 3))
    amp, ph = idle
    hand = limb.endswith("hand")
    sway = 0.2 * amp if hand else 0.03 * amp
    trem = 0.04 * amp if hand else 0.01 * amp
    gsw = 0.06 * amp if hand else 0.01 * amp
    for j in range(3):
        a[:, j] += sway * np.sin(2.0 * math.pi * (0.31 + 0.07 * j) * t + ph[j])
        a[:, j] += trem * np.sin(2.0 * math.pi * (8.7 + 0.6 * j) * t + ph[3 + j])
        w[:, j] += gsw * np.sin(2.0 * math.pi * (0.27 + 0.05 * j) * t + ph[6 + j])
    for fn, kind, t0, dur, s, icon in acts:
        if kind == "accent":
            da, dw = fn(t, t0, s, icon, limb)
        else:
            da, dw = fn(t, t0, dur, s, limb)
        a += da
        w += dw
    return a, w


def render_limb(choreo, limb, perf_seed, sensor_seed, duration_s, mode="good",
                acc_noise=0.05, gyro_noise=0.01, drop_rate=0.0, idle_amp=1.0, jitter_s=0.0004):
    """Muestras (dict t, src, limb, a, g) de un móvil en `limb`."""
    rp = np.random.default_rng(perf_seed)
    rs = np.random.default_rng(sensor_seed)
    q0 = initial_orientation(limb, rp)
    acts = [] if mode == "still" else plan_actions(choreo, limb, rp, mode)
    idle = (idle_amp, rp.uniform(0.0, 2.0 * math.pi, size=9))

    tg = np.arange(0.0, duration_s + 0.2, GRID_DT)
    _, wg = motion_at(tg, limb, acts, idle)
    qg = _integrate(q0, wg, GRID_DT)

    src, phase = DEVICES[limb]
    n = int(duration_s * RATE_HZ)
    ts = phase + np.arange(n) / RATE_HZ + rs.uniform(-jitter_s, jitter_s, size=n)
    aw, ws = motion_at(ts, limb, acts, idle)
    # orientación en ts: nlerp entre puntos de la rejilla
    idx = np.clip(np.floor(ts / GRID_DT).astype(int), 0, len(tg) - 2)
    fr = ((ts - tg[idx]) / GRID_DT)[:, None]
    q1, q2 = qg[idx], qg[idx + 1]
    q2 = np.where(np.sum(q1 * q2, axis=1, keepdims=True) < 0, -q2, q2)
    qs = q1 * (1.0 - fr) + q2 * fr
    qs /= np.linalg.norm(qs, axis=1, keepdims=True)

    acc = _rotate_inv(qs, aw + np.array([0.0, G, 0.0]))
    acc += rs.normal(0.0, 0.03, size=3) + rs.normal(0.0, acc_noise, size=(n, 3))
    gyr = ws + rs.normal(0.0, 0.003, size=3) + rs.normal(0.0, gyro_noise, size=(n, 3))
    acc = np.clip(acc, -ACC_SAT, ACC_SAT)
    gyr = np.clip(gyr, -GYRO_SAT, GYRO_SAT)
    keep = np.ones(n, dtype=bool) if drop_rate <= 0 else rs.uniform(size=n) >= drop_rate
    return [{"t": float(ts[i]), "src": src, "limb": limb, "a": acc[i].tolist(), "g": gyr[i].tolist()}
            for i in range(n) if keep[i]]


def render_session(choreo, limbs, perf_seed, sensor_seed, duration_s=30.0, time_shift_s=0.0, **kw):
    out = []
    for limb in limbs:
        sid = DEVICES[limb][0]  # semilla por extremidad (independiente de qué otros móviles haya)
        out += render_limb(choreo, limb, perf_seed * 10 + sid, sensor_seed * 10 + sid, duration_s, **kw)
    for s in out:
        s["t"] += time_shift_s
    out.sort(key=lambda s: (s["t"], s["src"]))
    return out


def session_header(limbs, note, song_id="song_demo"):
    names = {"right_hand": "Android sintético (mano der.)", "left_hand": "Android sintético (mano izq.)",
             "right_foot": "Android sintético (pie der.)", "left_foot": "Android sintético (pie izq.)"}
    return {"format": "jd-session", "version": 1, "time_base": "song", "song_id": song_id,
            "recorded_at": "2026-11-06T12:00:00Z", "note": note,
            "devices": [{"source_id": DEVICES[l][0], "name": names[l], "kind": "phone", "limb": l}
                        for l in sorted(limbs, key=lambda l: DEVICES[l][0])]}


ALL3 = ["left_hand", "right_hand", "right_foot"]

# nombre -> (nota, kwargs de render_session)
SESSIONS = {
    "perfect": ("igual que la referencia (otra ejecución humana)", dict(limbs=ALL3, perf_seed=2001, sensor_seed=2002)),
    "late_40ms": ("perfect desplazada +40 ms", dict(limbs=ALL3, perf_seed=2001, sensor_seed=2002, time_shift_s=0.040)),
    "late_150ms": ("perfect desplazada +150 ms", dict(limbs=ALL3, perf_seed=2001, sensor_seed=2002, time_shift_s=0.150)),
    "early_80ms": ("perfect desplazada −80 ms", dict(limbs=ALL3, perf_seed=2001, sensor_seed=2002, time_shift_s=-0.080)),
    "wrong_moves": ("gestos equivocados en todos los pasos", dict(limbs=ALL3, perf_seed=3001, sensor_seed=3002, mode="wrong")),
    "still": ("móviles sujetos sin bailar", dict(limbs=ALL3, perf_seed=4001, sensor_seed=4002, mode="still", idle_amp=0.5)),
    "noisy": ("perfect con ruido ×8, sesgo y 2 % de muestras perdidas",
              dict(limbs=ALL3, perf_seed=2001, sensor_seed=5002, acc_noise=0.4, gyro_noise=0.08, drop_rate=0.02, jitter_s=0.0008)),
    "foot_only": ("solo un móvil en el pie derecho, bien bailado", dict(limbs=["right_foot"], perf_seed=2001, sensor_seed=2002)),
}
REF = ("referencia del coreógrafo", dict(limbs=ALL3, perf_seed=1001, sensor_seed=1002))
