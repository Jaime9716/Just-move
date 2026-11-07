//! Utilidades compartidas de los tests de T1 (se incluye con `#[path = "t1_util.rs"] mod util;`).
//! Generadores deterministas de muestras sintéticas: solo usan la API pública de jd_core.
#![allow(dead_code)]

use jd_core::types::*;
use std::path::PathBuf;

pub const G: f64 = 9.80665;
pub const DT: f64 = 0.005; // 200 Hz
/// Fase para que las muestras no caigan en el borde de una trama.
pub const PHASE: f64 = 0.0013;

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}
pub fn synth_dir() -> PathBuf { repo_root().join("data").join("sessions").join("synthetic") }

pub fn sample(t: f64, limb: Limb, a: [f64; 3], g: [f64; 3]) -> MotionSample {
    MotionSample {
        t_ns: (t * 1e9).round() as i64,
        source_id: limb as u32 + 1,
        limb: limb as u8,
        flags: SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO,
        _reserved: 0,
        accel: [a[0] as f32, a[1] as f32, a[2] as f32],
        gyro: [g[0] as f32, g[1] as f32, g[2] as f32],
    }
}

pub fn times(t0: f64, t1: f64) -> Vec<f64> {
    let n = ((t1 - t0) / DT).floor() as usize;
    (0..n).map(|i| t0 + PHASE + i as f64 * DT).collect()
}

/// Generador congruencial determinista (sin dependencias).
pub struct Lcg(pub u64);
impl Lcg {
    pub fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    /// Uniforme en [-1, 1).
    pub fn uni(&mut self) -> f64 { self.next_u32() as f64 / (1u64 << 31) as f64 - 1.0 }
}

fn gauss(t: f64, mu: f64, sigma: f64) -> f64 { (-(t - mu) * (t - mu) / (2.0 * sigma * sigma)).exp() }

/// Móvil vertical en reposo (Y arriba) con golpes gaussianos en +Y (amplitud 20 m/s², σ = 12 ms)
/// en los instantes `hits`, de `t0` a `t1`.
pub fn punches(limb: Limb, hits: &[f64], t0: f64, t1: f64) -> Vec<MotionSample> {
    times(t0, t1).into_iter().map(|t| {
        let mut ay = G;
        for &h in hits { ay += 20.0 * gauss(t, h, 0.012); }
        sample(t, limb, [0.0, ay, 0.0], [0.0; 3])
    }).collect()
}

/// Señal de "move" no periódica en [start, start+dur] (envolvente suave), desplazada `shift` s.
/// Aceleración lineal en X y Z (el móvil sigue vertical); fuera de la ventana, reposo.
pub fn move_signal(limb: Limb, start: f64, dur: f64, shift: f64, gain: f64, t0: f64, t1: f64) -> Vec<MotionSample> {
    times(t0, t1).into_iter().map(|t| {
        let u = t - shift - start;
        let (mut ax, mut az) = (0.0, 0.0);
        if u > -0.2 && u < dur + 0.2 {
            let env = (std::f64::consts::PI * ((u + 0.2) / (dur + 0.4))).sin().powi(2);
            ax = gain * env * (7.0 * (2.0 * std::f64::consts::PI * 1.3 * u).sin() + 3.0 * (2.0 * std::f64::consts::PI * 3.1 * u + 0.7).sin());
            az = gain * env * 4.0 * (2.0 * std::f64::consts::PI * 2.2 * u + 1.1).cos();
        }
        sample(t, limb, [ax, G, az], [0.0; 3])
    }).collect()
}

/// Muestras en reposo (vertical) con un poco de ruido determinista.
pub fn rest(limb: Limb, t0: f64, t1: f64, noise: f64, seed: u64) -> Vec<MotionSample> {
    let mut r = Lcg(seed);
    times(t0, t1).into_iter().map(|t| {
        sample(t, limb, [noise * r.uni(), G + noise * r.uni(), noise * r.uni()], [0.01 * noise * r.uni(), 0.0, 0.0])
    }).collect()
}

pub fn sort_t(mut v: Vec<MotionSample>) -> Vec<MotionSample> { v.sort_by_key(|s| s.t_ns); v }

pub fn step(t: f64, dur: f64, limbs: &[Limb], kind: StepKind) -> Step {
    Step { t, dur, limbs: limbs.to_vec(), icon: "x".into(), kind }
}

pub fn choreo(steps: Vec<Step>) -> Choreo {
    Choreo { title: "t1".into(), audio: "audio.ogg".into(), bpm: 120.0, duration_s: 60.0, steps, ..Default::default() }
}

pub fn grade_name(g: u8) -> &'static str { match g { 0 => "miss", 1 => "good", 2 => "perfect", _ => "?" } }

/// Golden de B: lista de (step, limb, grade, similarity, timing_error_ms).
pub fn load_golden(name: &str) -> Vec<(i64, String, String, f64, f64)> {
    let v = jd_core::json::parse_file(&synth_dir().join("golden").join(format!("{}.json", name))).unwrap();
    v.as_array().unwrap().iter().map(|r| (
        r.get("step").as_f64().unwrap() as i64,
        r.get("limb").as_str().unwrap().to_string(),
        r.get("grade").as_str().unwrap().to_string(),
        r.get("similarity").as_f64().unwrap(),
        r.get("timing_error_ms").as_f64().unwrap(),
    )).collect()
}

pub const GOLDEN_SESSIONS: [&str; 8] = ["perfect", "late_40ms", "late_150ms", "early_80ms", "wrong_moves", "still", "noisy", "foot_only"];

pub fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("jd_t1_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
