"""B3 · Señal: filtro paso bajo, orientación (mundo con Y arriba) y aceleración lineal.

Fiel a contracts/judge_algorithm_v1.md §B3. Detalles que el contrato no fija y que aquí se
deciden (el motor Rust debe hacer lo mismo; ver proto/ALGORITHM.md §"Precisiones"):
  * Tiempo: t_song = round(t·1e9)·1e-9 + mono_to_song_s (igual que t_ns del motor).
  * El filtro se inicializa con la primera muestra (y = x).
  * La primera muestra fija q inicial y luego pasa por los mismos pasos que las demás (dt = 0.005).
  * La puerta de gravedad usa |a| FILTRADO.
  * Si falta `a` o `g` en una muestra se usa el último valor bruto recibido (o (0,G,0) / (0,0,0)).
  * Cuaterniones (w, x, y, z), mundo ← sensor; el gyro se integra en el marco del sensor (q ⊗ dq).
"""

import math

from .config import resolve_config


# ------------------------------------------------------------------ cuaterniones (tuplas)

def q_mul(a, b):
    aw, ax, ay, az = a
    bw, bx, by, bz = b
    return (aw * bw - ax * bx - ay * by - az * bz,
            aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw)


def q_normalize(q):
    n = math.sqrt(q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3])
    if n < 1e-12:
        return (1.0, 0.0, 0.0, 0.0)
    return (q[0] / n, q[1] / n, q[2] / n, q[3] / n)


def q_from_axis_angle(axis, angle):
    """axis unitario."""
    h = 0.5 * angle
    s = math.sin(h)
    return (math.cos(h), axis[0] * s, axis[1] * s, axis[2] * s)


def q_rotate(q, v):
    """R(q)·v (mundo ← sensor)."""
    w, x, y, z = q
    vx, vy, vz = v
    # t = 2 · (q_vec × v)
    tx = 2.0 * (y * vz - z * vy)
    ty = 2.0 * (z * vx - x * vz)
    tz = 2.0 * (x * vy - y * vx)
    return (vx + w * tx + (y * tz - z * ty),
            vy + w * ty + (z * tx - x * tz),
            vz + w * tz + (x * ty - y * tx))


def q_angle_between(q1, q2):
    """Ángulo (rad) de la rotación relativa entre dos orientaciones."""
    d = abs(q1[0] * q2[0] + q1[1] * q2[1] + q1[2] * q2[2] + q1[3] * q2[3])
    return 2.0 * math.acos(min(1.0, d))


def rotation_to_up(v):
    """Rotación mínima que lleva la dirección v a +Y (yaw = 0)."""
    n = math.sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
    if n < 1e-12:
        return (1.0, 0.0, 0.0, 0.0)
    ux, uy, uz = v[0] / n, v[1] / n, v[2] / n
    # eje = u × Y = (−uz, 0, ux)·(−1)?  u × (0,1,0) = (uy·0 − uz·1, uz·0 − ux·0, ux·1 − uy·0) = (−uz, 0, ux)
    axx, axy, axz = -uz, 0.0, ux
    s = math.sqrt(axx * axx + axz * axz)
    c = uy
    if s < 1e-12:
        if c > 0:
            return (1.0, 0.0, 0.0, 0.0)
        return (0.0, 1.0, 0.0, 0.0)  # 180° sobre X
    angle = math.atan2(s, c)
    return q_from_axis_angle((axx / s, axy / s, axz / s), angle)


# ------------------------------------------------------------------ procesador por extremidad

class LimbSignal:
    """Estado B3 de una extremidad."""

    def __init__(self, cfg):
        self.cfg = cfg
        self.tau = 1.0 / (2.0 * math.pi * cfg["lpf_cutoff_hz"])
        self.reset()

    def reset(self):
        self.last_t = None
        self.ya = None
        self.yg = None
        self.raw_a = (0.0, self.cfg["g"], 0.0)
        self.raw_g = (0.0, 0.0, 0.0)
        self.q = None

    def process(self, t_song, a, g):
        cfg = self.cfg
        G = cfg["g"]
        if a is not None:
            self.raw_a = (float(a[0]), float(a[1]), float(a[2]))
        if g is not None:
            self.raw_g = (float(g[0]), float(g[1]), float(g[2]))
        xa, xg = self.raw_a, self.raw_g

        if self.last_t is None:
            dt = 0.005
        else:
            dt = t_song - self.last_t
        dt = min(max(dt, 0.0005), 0.05)
        self.last_t = t_song

        # 2. paso bajo
        if self.ya is None:
            self.ya, self.yg = xa, xg
        else:
            k = dt / (dt + self.tau)
            ya, yg = self.ya, self.yg
            self.ya = (ya[0] + k * (xa[0] - ya[0]), ya[1] + k * (xa[1] - ya[1]), ya[2] + k * (xa[2] - ya[2]))
            self.yg = (yg[0] + k * (xg[0] - yg[0]), yg[1] + k * (xg[1] - yg[1]), yg[2] + k * (xg[2] - yg[2]))
        af, gf = self.ya, self.yg

        # 3. orientación
        if self.q is None:
            self.q = rotation_to_up(af)
        q = self.q
        wn = math.sqrt(gf[0] * gf[0] + gf[1] * gf[1] + gf[2] * gf[2])
        theta = wn * dt
        if theta > 1e-12:
            dq = q_from_axis_angle((gf[0] / wn, gf[1] / wn, gf[2] / wn), theta)
            q = q_mul(q, dq)
        q = q_normalize(q)

        an = math.sqrt(af[0] * af[0] + af[1] * af[1] + af[2] * af[2])
        if cfg["gravity_gate_lo"] * G <= an <= cfg["gravity_gate_hi"] * G:
            up = q_rotate(q, (af[0] / an, af[1] / an, af[2] / an))
            # eje = up × Y = (−up_z, 0, up_x)
            axx, axz = -up[2], up[0]
            s = math.sqrt(axx * axx + axz * axz)
            if s > 1e-12:
                angle = math.atan2(s, up[1])
                r = q_from_axis_angle((axx / s, 0.0, axz / s), cfg["comp_alpha"] * angle)
                q = q_normalize(q_mul(r, q))
        self.q = q

        # 4. aceleración lineal
        wa = q_rotate(q, af)
        lin = (wa[0], wa[1] - G, wa[2])
        return {"t": t_song, "a": af, "g": gf, "lin": lin, "q": q}


def song_time(t, mono_to_song_s=0.0):
    """Tiempo de canción tal y como lo ve el motor: t_ns = round(t·1e9)."""
    return round(t * 1e9) * 1e-9 + mono_to_song_s


def process_samples(samples, config=None, mono_to_song_s=0.0):
    """Muestras (dict t, src, limb, a, g) -> {limb: [filtrada]} con filtrada = dict(t, a, g, lin, q).

    Cada extremidad tiene su propio estado; las muestras de una extremidad se procesan en orden de
    tiempo (orden estable si hay empates)."""
    cfg = resolve_config(config)
    by_limb = {}
    for s in samples:
        if s["limb"] == "none":
            continue
        by_limb.setdefault(s["limb"], []).append(s)
    out = {}
    for limb, ss in by_limb.items():
        ss = sorted(ss, key=lambda s: s["t"])
        proc = LimbSignal(cfg)
        out[limb] = [proc.process(song_time(s["t"], mono_to_song_s), s["a"], s["g"]) for s in ss]
    return out
