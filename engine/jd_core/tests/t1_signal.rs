//! T1 · B3 Señal (caja negra): reposo estable, giro de 90° ±5°, lin_accel ≈ 0 en reposo,
//! aceleración lineal conocida, filtro paso bajo exacto, independencia por extremidad.
#[path = "t1_util.rs"]
mod util;

use jd_core::signal::make_signal_processor;
use jd_core::types::*;
use util::*;

fn run(samples: &[MotionSample]) -> Vec<FilteredSample> {
    let mut sp = make_signal_processor();
    let mut out = Vec::new();
    sp.process(samples, 0.0, &mut out);
    out
}

fn n3(v: [f32; 3]) -> f64 { ((v[0] as f64).powi(2) + (v[1] as f64).powi(2) + (v[2] as f64).powi(2)).sqrt() }

fn rot(q: [f32; 4], v: [f64; 3]) -> [f64; 3] {
    let q = [q[0] as f64, q[1] as f64, q[2] as f64, q[3] as f64];
    jd_core::signal::q_rotate(q, v)
}

/// Ángulo (grados) de la rotación relativa entre dos cuaterniones unitarios.
fn rel_angle_deg(a: [f32; 4], b: [f32; 4]) -> f64 {
    let d = (a[0] as f64 * b[0] as f64 + a[1] as f64 * b[1] as f64 + a[2] as f64 * b[2] as f64 + a[3] as f64 * b[3] as f64).abs();
    2.0 * d.min(1.0).acos().to_degrees()
}

#[test]
fn rest_upright_is_stable_and_lin_accel_zero() {
    let s = rest(Limb::RightHand, 0.0, 10.0, 0.05, 7);
    let f = run(&s);
    assert_eq!(f.len(), s.len());
    let q0 = f[0].quat;
    for x in &f {
        assert!(n3(x.lin_accel) < 0.15, "lin_accel en reposo = {:?} en t={}", x.lin_accel, x.t_song);
        assert!(rel_angle_deg(q0, x.quat) < 1.0, "la orientación deriva en reposo: t={}", x.t_song);
        // t_song = t_ns·1e-9 + mono_to_song_s
    }
    for (a, b) in s.iter().zip(&f) { assert_eq!(b.t_song, a.t_ns as f64 * 1e-9); }
}

#[test]
fn rest_tilted_lin_accel_zero_from_first_sample() {
    // Móvil inclinado 35° sobre X y 20° sobre Z: la orientación inicial lleva el accel a +Y.
    let q = jd_core::signal::q_mul(jd_core::signal::q_axis_angle([1.0, 0.0, 0.0], 35f64.to_radians()),
                                  jd_core::signal::q_axis_angle([0.0, 0.0, 1.0], 20f64.to_radians()));
    // accel en el sensor = R(q)^T · (0, G, 0)
    let qi = [q[0], -q[1], -q[2], -q[3]];
    let a = jd_core::signal::q_rotate(qi, [0.0, G, 0.0]);
    let s: Vec<_> = times(0.0, 3.0).into_iter().map(|t| sample(t, Limb::LeftFoot, a, [0.0; 3])).collect();
    let f = run(&s);
    for x in &f {
        assert!(n3(x.lin_accel) < 1e-3, "lin_accel {:?}", x.lin_accel);
        let up = rot(x.quat, [a[0] / G, a[1] / G, a[2] / G]);
        assert!((up[1] - 1.0).abs() < 1e-5, "R(q)·â no apunta a +Y: {:?}", up);
    }
}

#[test]
fn yaw_90_degrees_within_5() {
    // Giro alrededor del eje Y del sensor (= vertical) a 90°/s durante 1 s; gravedad constante.
    let w = std::f64::consts::FRAC_PI_2;
    let s: Vec<_> = times(0.0, 3.0).into_iter().map(|t| {
        let gy = if t >= 0.5 && t < 1.5 { w } else { 0.0 };
        sample(t, Limb::RightHand, [0.0, G, 0.0], [0.0, gy, 0.0])
    }).collect();
    let f = run(&s);
    let ang = rel_angle_deg(f[0].quat, f.last().unwrap().quat);
    assert!((ang - 90.0).abs() <= 5.0, "giro estimado {:.2}°", ang);
    // La guiñada no la corrige la gravedad: el resultado debe ser estable tras el giro.
    let mid = f.iter().find(|x| x.t_song > 2.0).unwrap().quat;
    assert!(rel_angle_deg(mid, f.last().unwrap().quat) < 0.5);
}

#[test]
fn roll_90_degrees_with_coherent_gravity_within_5() {
    // Giro alrededor del eje Z del sensor a 90°/s durante 1 s; el accel del sensor gira con él.
    let w = std::f64::consts::FRAC_PI_2;
    let s: Vec<_> = times(0.0, 3.0).into_iter().map(|t| {
        let th = ((t - 0.5).clamp(0.0, 1.0)) * w;
        let gz = if t >= 0.5 && t < 1.5 { w } else { 0.0 };
        sample(t, Limb::LeftHand, [G * th.sin(), G * th.cos(), 0.0], [0.0, 0.0, gz])
    }).collect();
    let f = run(&s);
    let last = f.last().unwrap();
    let ang = rel_angle_deg(f[0].quat, last.quat);
    assert!((ang - 90.0).abs() <= 5.0, "giro estimado {:.2}°", ang);
    assert!(n3(last.lin_accel) < 0.2, "lin_accel tras el giro {:?}", last.lin_accel);
    for x in f.iter().filter(|x| x.t_song < 0.5) { assert!(n3(x.lin_accel) < 1e-3); }
}

#[test]
fn known_linear_acceleration_outside_gravity_gate() {
    // +10 m/s² en X (|a| ≈ 14 m/s² > 1.2·G: la puerta rechaza la corrección) durante 0.5 s.
    let s: Vec<_> = times(0.0, 2.0).into_iter().map(|t| {
        let ax = if t >= 1.0 && t < 1.5 { 10.0 } else { 0.0 };
        sample(t, Limb::RightHand, [ax, G, 0.0], [0.0; 3])
    }).collect();
    let f = run(&s);
    for x in f.iter().filter(|x| x.t_song > 1.1 && x.t_song < 1.5) {
        // Mientras el LPF sube, |a| cruza la banda 0.8–1.2 G y la corrección inclina q un poco (lo que
        // dicta el contrato): se admite un 3 %.
        assert!((x.lin_accel[0] as f64 - 10.0).abs() < 0.3, "lin_x = {} en t={}", x.lin_accel[0], x.t_song);
        assert!((x.lin_accel[1] as f64).abs() < 0.3 && (x.lin_accel[2] as f64).abs() < 0.3, "{:?}", x.lin_accel);
    }
}

#[test]
fn lowpass_step_matches_contract_formula() {
    // y += k·(x − y), k = dt/(dt + 1/(2π·20)). Escalón fuera de la puerta para aislar el LPF.
    let s: Vec<_> = (0..40).map(|i| {
        let t = 1.0 + i as f64 * DT;
        let ax = if i >= 20 { 30.0 } else { 0.0 };
        sample(t, Limb::RightFoot, [ax, G, 0.0], [0.0; 3])
    }).collect();
    let f = run(&s);
    let k = DT / (DT + 1.0 / (2.0 * std::f64::consts::PI * 20.0));
    let mut y = 0.0f64;
    for (i, x) in f.iter().enumerate() {
        if i > 0 {
            let dt = (s[i].t_ns - s[i - 1].t_ns) as f64 * 1e-9;
            let k_i = dt / (dt + 1.0 / (2.0 * std::f64::consts::PI * 20.0));
            y += k_i * (s[i].accel[0] as f64 - y);
        }
        assert!((x.accel[0] as f64 - y).abs() < 1e-4, "muestra {}: filtrado {} esperado {}", i, x.accel[0], y);
    }
    assert!((k - 0.3858).abs() < 1e-3);
}

#[test]
fn limbs_have_independent_state() {
    let a = punches(Limb::LeftHand, &[1.0, 2.0], 0.0, 3.0);
    let b = move_signal(Limb::RightFoot, 0.5, 2.0, 0.0, 1.0, 0.0, 3.0);
    let mut both: Vec<_> = a.iter().chain(b.iter()).copied().collect();
    both.sort_by_key(|s| s.t_ns);
    let fa = run(&a);
    let fb = run(&b);
    let fab = run(&both);
    let ia: Vec<_> = fab.iter().filter(|x| x.limb == Limb::LeftHand).copied().collect();
    let ib: Vec<_> = fab.iter().filter(|x| x.limb == Limb::RightFoot).copied().collect();
    assert_eq!(ia, fa);
    assert_eq!(ib, fb);
}

#[test]
fn mapping_is_added_to_time() {
    let s = rest(Limb::RightHand, 100.0, 101.0, 0.0, 1);
    let mut sp = make_signal_processor();
    let mut out = Vec::new();
    sp.process(&s, -97.5, &mut out);
    for (a, b) in s.iter().zip(&out) { assert_eq!(b.t_song, a.t_ns as f64 * 1e-9 + -97.5); }
}

#[test]
fn limb_none_is_ignored() {
    let mut s = rest(Limb::RightHand, 0.0, 0.5, 0.0, 1);
    for x in s.iter_mut().step_by(2) { x.limb = 255; }
    let f = run(&s);
    assert_eq!(f.len(), s.len() / 2);
}
