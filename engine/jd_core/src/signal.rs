//! B3 · Procesado de señal (contracts/judge_algorithm_v1.md). AGENTE C.
//!
//! Por extremidad: filtro paso bajo IIR de 1er orden (accel y gyro), orientación con filtro
//! complementario (mundo Y arriba, yaw inicial 0) y aceleración lineal sin gravedad.
//! Cálculo interno en f64; la salida (FilteredSample) en f32.
//!
//! Interpretaciones locales (ver docs/agents/C-report.md):
//! Sigue proto/ALGORITHM.md §1 (normativo desde v1.1):
//! - La primera muestra inicializa (filtro = entrada, q = rotación mínima del accel a +Y) y después
//!   pasa por los mismos pasos que las demás (integra con dt = 0.005, corrige, lin_accel).
//! - La puerta de gravedad usa la norma del accel FILTRADO.
//! - Si `flags` indica que falta accel o gyro, se usa el último valor BRUTO de ese sensor
//!   ((0,G,0) / 0 si nunca llegó). `flags == 0` se interpreta como "trae los dos" (local).
//! - Muestras con `limb` fuera de 0..3 se ignoran.
use crate::types::{FilteredSample, Limb, MotionSample, SignalProcessor, SAMPLE_HAS_ACCEL, SAMPLE_HAS_GYRO};

/// Parámetros de B3 (valores por defecto del contrato v1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignalParams {
    pub g: f64,
    pub lpf_cutoff_hz: f64,
    pub comp_alpha: f64,
    pub gravity_gate_lo: f64, // × G
    pub gravity_gate_hi: f64, // × G
    pub first_dt: f64,
    pub dt_min: f64,
    pub dt_max: f64,
}

impl Default for SignalParams {
    fn default() -> Self {
        SignalParams {
            g: 9.80665,
            lpf_cutoff_hz: 20.0,
            comp_alpha: 0.02,
            gravity_gate_lo: 0.8,
            gravity_gate_hi: 1.2,
            first_dt: 0.005,
            dt_min: 0.0005,
            dt_max: 0.05,
        }
    }
}

// ------------------------------------------------------------------ cuaterniones (w, x, y, z)
pub type Quat = [f64; 4];
pub type Vec3 = [f64; 3];

pub fn q_mul(a: Quat, b: Quat) -> Quat {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

pub fn q_normalize(q: Quat) -> Quat {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n < 1e-300 { return [1.0, 0.0, 0.0, 0.0]; }
    [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
}

/// Rota v con q (R(q)·v).
pub fn q_rotate(q: Quat, v: Vec3) -> Vec3 {
    let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
    // v' = v + 2w(u×v) + 2u×(u×v), u = (x,y,z)
    let t = [2.0 * (y * v[2] - z * v[1]), 2.0 * (z * v[0] - x * v[2]), 2.0 * (x * v[1] - y * v[0])];
    [
        v[0] + w * t[0] + (y * t[2] - z * t[1]),
        v[1] + w * t[1] + (z * t[0] - x * t[2]),
        v[2] + w * t[2] + (x * t[1] - y * t[0]),
    ]
}

/// Cuaternión de eje (no necesariamente unitario) y ángulo.
pub fn q_axis_angle(axis: Vec3, angle: f64) -> Quat {
    let n = norm(axis);
    if n < 1e-300 { return [1.0, 0.0, 0.0, 0.0]; }
    let h = 0.5 * angle;
    let s = h.sin() / n;
    [h.cos(), axis[0] * s, axis[1] * s, axis[2] * s]
}

pub fn norm(v: Vec3) -> f64 { (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() }
pub fn dot(a: Vec3, b: Vec3) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

const UP: Vec3 = [0.0, 1.0, 0.0];

/// Rotación mínima que lleva el vector unitario `from` a `to` (antiparalelos: 180° sobre X).
fn q_from_to(from: Vec3, to: Vec3) -> Quat {
    let axis = cross(from, to);
    let an = norm(axis);
    if an < 1e-12 {
        if dot(from, to) > 0.0 { return [1.0, 0.0, 0.0, 0.0]; }
        return q_axis_angle([1.0, 0.0, 0.0], std::f64::consts::PI);
    }
    q_axis_angle(axis, an.atan2(dot(from, to)))
}

#[derive(Clone, Copy, Debug)]
struct LimbState {
    started: bool,
    last_t: f64,
    raw_a: Vec3,
    raw_g: Vec3,
    acc: Vec3,
    gyr: Vec3,
    q: Quat,
}

impl Default for LimbState {
    fn default() -> Self {
        LimbState {
            started: false, last_t: 0.0, raw_a: [0.0, 9.80665, 0.0], raw_g: [0.0; 3],
            acc: [0.0; 3], gyr: [0.0; 3], q: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

pub struct DefaultSignalProcessor {
    p: SignalParams,
    limbs: [LimbState; 4],
}

impl DefaultSignalProcessor {
    pub fn new(p: SignalParams) -> Self {
        let mut st = LimbState::default();
        st.raw_a = [0.0, p.g, 0.0];
        DefaultSignalProcessor { p, limbs: [st; 4] }
    }

    fn step(&mut self, s: &MotionSample, mono_to_song_s: f64) -> Option<FilteredSample> {
        let limb = s.limb();
        if limb == Limb::None { return None; }
        let p = self.p;
        let st = &mut self.limbs[limb as usize];
        let t = s.t_ns as f64 * 1e-9 + mono_to_song_s;
        let has_a = s.flags == 0 || s.flags & SAMPLE_HAS_ACCEL != 0;
        let has_g = s.flags == 0 || s.flags & SAMPLE_HAS_GYRO != 0;
        // Sensor ausente: último valor BRUTO recibido (o (0,G,0) / 0 si nunca llegó).
        if has_a { st.raw_a = [s.accel[0] as f64, s.accel[1] as f64, s.accel[2] as f64]; }
        if has_g { st.raw_g = [s.gyro[0] as f64, s.gyro[1] as f64, s.gyro[2] as f64]; }
        let (ax, gx) = (st.raw_a, st.raw_g);
        let g = p.g;

        let dt;
        if !st.started {
            st.started = true;
            dt = p.first_dt;
            st.acc = ax;
            st.gyr = gx;
            let n = norm(st.acc);
            st.q = if n > 0.0 {
                q_from_to([st.acc[0] / n, st.acc[1] / n, st.acc[2] / n], UP)
            } else {
                [1.0, 0.0, 0.0, 0.0]
            };
        } else {
            dt = (t - st.last_t).clamp(p.dt_min, p.dt_max);
            let k = dt / (dt + 1.0 / (2.0 * std::f64::consts::PI * p.lpf_cutoff_hz));
            for i in 0..3 { st.acc[i] += k * (ax[i] - st.acc[i]); }
            for i in 0..3 { st.gyr[i] += k * (gx[i] - st.gyr[i]); }
        }
        st.last_t = t;
        // Integración del gyro (marco del sensor): q ← normalize(q ⊗ exp(½·ω·dt))
        let w = st.gyr;
        let theta = norm(w) * dt;
        if theta > 1e-12 {
            st.q = q_mul(st.q, q_axis_angle(w, theta));
        }
        st.q = q_normalize(st.q);
        // Corrección complementaria hacia la gravedad (marco del mundo).
        let an = norm(st.acc);
        if an >= p.gravity_gate_lo * g && an <= p.gravity_gate_hi * g {
            let ahat = [st.acc[0] / an, st.acc[1] / an, st.acc[2] / an];
            let up = q_rotate(st.q, ahat);
            let axis = cross(up, UP);
            let axn = norm(axis);
            if axn > 1e-12 {
                let ang = axn.atan2(up[1]);
                st.q = q_normalize(q_mul(q_axis_angle(axis, p.comp_alpha * ang), st.q));
            }
        }
        let wa = q_rotate(st.q, st.acc);
        let lin = [wa[0], wa[1] - g, wa[2]];
        Some(FilteredSample {
            t_song: t,
            limb,
            accel: f3(st.acc),
            lin_accel: f3(lin),
            gyro: f3(st.gyr),
            quat: [st.q[0] as f32, st.q[1] as f32, st.q[2] as f32, st.q[3] as f32],
        })
    }
}

fn f3(v: Vec3) -> [f32; 3] { [v[0] as f32, v[1] as f32, v[2] as f32] }

impl SignalProcessor for DefaultSignalProcessor {
    fn process(&mut self, input: &[MotionSample], mono_to_song_s: f64, out: &mut Vec<FilteredSample>) {
        out.reserve(input.len());
        for s in input {
            if let Some(f) = self.step(s, mono_to_song_s) { out.push(f); }
        }
    }
    fn reset(&mut self) { *self = DefaultSignalProcessor::new(self.p); }
}

pub fn make_signal_processor() -> Box<dyn SignalProcessor> {
    Box::new(DefaultSignalProcessor::new(SignalParams::default()))
}

pub fn make_signal_processor_with(p: SignalParams) -> Box<dyn SignalProcessor> {
    Box::new(DefaultSignalProcessor::new(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(t: f64, limb: Limb, a: [f32; 3], g: [f32; 3]) -> MotionSample {
        MotionSample {
            t_ns: (t * 1e9).round() as i64, source_id: 1, limb: limb as u8,
            flags: SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO, _reserved: 0, accel: a, gyro: g,
        }
    }

    fn q_angle_deg(q: [f32; 4]) -> f64 {
        (2.0 * (q[0] as f64).abs().min(1.0).acos()).to_degrees()
    }

    #[test]
    fn rest_is_stable() {
        let mut sp = make_signal_processor();
        let input: Vec<_> = (0..2000).map(|i| sample(i as f64 * 0.005, Limb::LeftHand, [0.0, 9.80665, 0.0], [0.0; 3])).collect();
        let mut out = Vec::new();
        sp.process(&input, 0.0, &mut out);
        assert_eq!(out.len(), 2000);
        for f in &out {
            let n = norm([f.lin_accel[0] as f64, f.lin_accel[1] as f64, f.lin_accel[2] as f64]);
            assert!(n < 1e-3, "lin {:?}", f.lin_accel);
            assert!(q_angle_deg(f.quat) < 0.01);
        }
    }

    #[test]
    fn tilted_rest_initialises_up() {
        // Móvil tumbado boca arriba: gravedad en +Z del sensor.
        let mut sp = make_signal_processor();
        let input: Vec<_> = (0..400).map(|i| sample(i as f64 * 0.005, Limb::RightFoot, [0.3, 0.2, 9.79], [0.0; 3])).collect();
        let mut out = Vec::new();
        sp.process(&input, 0.0, &mut out);
        let last = out.last().unwrap();
        let n = norm([last.lin_accel[0] as f64, last.lin_accel[1] as f64, last.lin_accel[2] as f64]);
        assert!(n < 0.02, "{:?}", last.lin_accel);
    }

    #[test]
    fn yaw_90_degrees() {
        // Giro de 90° alrededor del eje vertical (Y del sensor = Y mundo) en 1 s.
        let mut sp = make_signal_processor();
        let w = std::f32::consts::FRAC_PI_2;
        let mut input = Vec::new();
        for i in 0..800 {
            let t = i as f64 * 0.005;
            let gy = if (0.5..1.5).contains(&t) { w } else { 0.0 };
            input.push(sample(t, Limb::LeftHand, [0.0, 9.80665, 0.0], [0.0, gy, 0.0]));
        }
        let mut out = Vec::new();
        sp.process(&input, 0.0, &mut out);
        let ang = q_angle_deg(out.last().unwrap().quat);
        assert!((ang - 90.0).abs() <= 5.0, "ángulo {}", ang);
    }

    #[test]
    fn roll_90_degrees_with_consistent_gravity() {
        // Giro de 90° sobre Z del sensor; el accel medido rota de forma coherente.
        let mut sp = make_signal_processor();
        let w = std::f64::consts::FRAC_PI_2;
        let g = 9.80665;
        let mut input = Vec::new();
        let mut theta: f64 = 0.0;
        for i in 0..800 {
            let t = i as f64 * 0.005;
            let wz = if (0.5..1.5).contains(&t) { w } else { 0.0 };
            if i > 0 { theta += wz * 0.005; }
            // gravedad en marco del sensor tras rotar el sensor +theta sobre Z: R^T·(0,g,0)
            let a = [(g * theta.sin()) as f32, (g * theta.cos()) as f32, 0.0];
            input.push(sample(t, Limb::LeftHand, a, [0.0, 0.0, wz as f32]));
        }
        let mut out = Vec::new();
        sp.process(&input, 0.0, &mut out);
        let last = out.last().unwrap();
        let ang = q_angle_deg(last.quat);
        assert!((ang - 90.0).abs() <= 5.0, "ángulo {}", ang);
        let n = norm([last.lin_accel[0] as f64, last.lin_accel[1] as f64, last.lin_accel[2] as f64]);
        assert!(n < 0.5, "lin {:?}", last.lin_accel);
    }

    #[test]
    fn limbs_are_independent_and_time_mapping_applies() {
        let mut sp = make_signal_processor();
        let input = vec![
            sample(1.0, Limb::LeftHand, [0.0, 9.8, 0.0], [0.0; 3]),
            sample(1.0, Limb::RightHand, [9.8, 0.0, 0.0], [0.0; 3]),
            sample(1.005, Limb::LeftHand, [0.0, 9.8, 0.0], [0.0; 3]),
            MotionSample { limb: 255, ..sample(1.0, Limb::LeftHand, [0.0; 3], [0.0; 3]) },
        ];
        let mut out = Vec::new();
        sp.process(&input, 2.5, &mut out);
        assert_eq!(out.len(), 3);
        assert!((out[0].t_song - 3.5).abs() < 1e-9);
        assert_eq!(out[1].limb, Limb::RightHand);
        for f in &out { assert!(f.lin_accel.iter().all(|v| v.abs() < 0.01)); }
        sp.reset();
        out.clear();
        sp.process(&input[..1], 0.0, &mut out);
        assert!((out[0].t_song - 1.0).abs() < 1e-9);
    }

    #[test]
    fn lpf_step_response() {
        let mut sp = make_signal_processor();
        let mut input = vec![sample(0.0, Limb::LeftHand, [0.0, 9.80665, 0.0], [0.0; 3])];
        for i in 1..100 { input.push(sample(i as f64 * 0.005, Limb::LeftHand, [0.0, 9.80665, 0.0], [1.0, 0.0, 0.0])); }
        let mut out = Vec::new();
        sp.process(&input, 0.0, &mut out);
        let k = 0.005 / (0.005 + 1.0 / (2.0 * std::f64::consts::PI * 20.0));
        assert!((out[1].gyro[0] as f64 - k).abs() < 1e-6);
        assert!((out[99].gyro[0] - 1.0).abs() < 1e-3);
    }
}
