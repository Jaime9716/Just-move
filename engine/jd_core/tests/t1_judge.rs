//! T1 · B5 Juez v1.1 (caja negra): accent 0/40/80/150 ms y temprano, pies ×1.5, move idéntico → Perfect,
//! move +150 ms → Miss, quieto → Miss, sin referencia nunca Perfect, orden y momento de emisión.
#[path = "t1_util.rs"]
mod util;

use jd_core::engine::{compute_features, judge_offline, Engine};
use jd_core::judge::make_judge;
use jd_core::types::*;
use util::*;

const P: u8 = Grade::Perfect as u8;
const GD: u8 = Grade::Good as u8;
const M: u8 = Grade::Miss as u8;

fn judge(c: &Choreo, reference: &[MotionSample], player: &[MotionSample]) -> Vec<StepResult> {
    let rf = if reference.is_empty() { Vec::new() } else { compute_features(reference, 0.0) };
    judge_offline(c, &rf, player, &JudgeConfig::default())
}

fn accent_case(limb: Limb, offset_s: f64) -> StepResult {
    let c = choreo(vec![step(2.0, 0.5, &[limb], StepKind::Accent)]);
    let player = punches(limb, &[2.0 + offset_s], 0.0, 4.0);
    let r = judge(&c, &[], &player);
    assert_eq!(r.len(), 1);
    r[0]
}

#[test]
fn accent_hands_on_time_late_and_early() {
    // (desfase, nota esperada según pw = 50 ms, gw = 100 ms; el LPF añade ~+5 ms)
    for (off, want) in [(0.0, P), (0.040, P), (0.080, GD), (0.150, M), (-0.040, P), (-0.080, GD), (-0.150, M)] {
        let r = accent_case(Limb::RightHand, off);
        assert_eq!(r.grade, want, "accent {:+} ms → {} (err {:.1} ms)", off * 1e3, grade_name(r.grade), r.timing_error_ms);
        if want != M {
            let err = r.timing_error_ms as f64 / 1e3;
            assert!((err - off).abs() <= 0.010, "timing_error {:.1} ms para {:+} ms", r.timing_error_ms, off * 1e3);
            let sim = (1.0 - err.abs() / 0.1).max(0.0);
            assert!((r.similarity as f64 - sim).abs() < 1e-4, "similarity {} vs {}", r.similarity, sim);
        } else {
            assert_eq!(r.similarity, 0.0);
        }
        assert_eq!(r.limb, Limb::RightHand as u8);
        assert_eq!(r.step_index, 0);
    }
}

#[test]
fn accent_feet_windows_scaled_1_5() {
    // pies: pw = 75 ms, gw = 150 ms
    for (off, hand, foot) in [(0.060, GD, P), (0.120, M, GD), (-0.120, M, GD), (0.200, M, M)] {
        let h = accent_case(Limb::LeftHand, off);
        let f = accent_case(Limb::RightFoot, off);
        assert_eq!(h.grade, hand, "mano {:+} ms (err {:.1})", off * 1e3, h.timing_error_ms);
        assert_eq!(f.grade, foot, "pie {:+} ms (err {:.1})", off * 1e3, f.timing_error_ms);
        if foot != M {
            let err = f.timing_error_ms as f64 / 1e3;
            assert!((f.similarity as f64 - (1.0 - err.abs() / 0.15)).abs() < 1e-4);
        }
    }
}

#[test]
fn accent_picks_nearest_peak() {
    let c = choreo(vec![step(2.0, 0.5, &[Limb::RightHand], StepKind::Accent)]);
    let player = punches(Limb::RightHand, &[1.93, 2.03], 0.0, 4.0);
    let r = judge(&c, &[], &player);
    assert!((r[0].timing_error_ms - 30.0).abs() < 10.0, "eligió {} ms", r[0].timing_error_ms);
}

#[test]
fn accent_custom_config_windows() {
    let c = choreo(vec![step(2.0, 0.5, &[Limb::RightHand], StepKind::Accent)]);
    let player = punches(Limb::RightHand, &[2.060], 0.0, 4.0);
    let cfg = JudgeConfig { perfect_window_ms: 80.0, good_window_ms: 120.0, foot_window_scale: 1.0 };
    let r = judge_offline(&c, &[], &player, &cfg);
    assert_eq!(r[0].grade, P);
}

fn move_choreo(limb: Limb) -> Choreo { choreo(vec![step(4.0, 2.0, &[limb], StepKind::Move)]) }

#[test]
fn move_identical_is_perfect() {
    for limb in [Limb::LeftHand, Limb::RightFoot] {
        let s = move_signal(limb, 4.0, 2.0, 0.0, 1.0, 0.0, 9.0);
        let r = judge(&move_choreo(limb), &s, &s);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].grade, P, "{:?}", r[0]);
        assert!(r[0].similarity > 0.99, "similarity {}", r[0].similarity);
        assert_eq!(r[0].timing_error_ms, 0.0);
    }
}

#[test]
fn move_shifted() {
    let limb = Limb::RightHand;
    let reference = move_signal(limb, 4.0, 2.0, 0.0, 1.0, 0.0, 9.0);
    let c = move_choreo(limb);
    let r40 = judge(&c, &reference, &move_signal(limb, 4.0, 2.0, 0.040, 1.0, 0.0, 9.0))[0];
    assert_eq!(r40.grade, P, "move +40 ms: {:?}", r40);
    assert!((r40.timing_error_ms - 40.0).abs() < 0.5);
    let r80 = judge(&c, &reference, &move_signal(limb, 4.0, 2.0, 0.080, 1.0, 0.0, 9.0))[0];
    assert_eq!(r80.grade, GD, "move +80 ms: {:?}", r80);
    let r150 = judge(&c, &reference, &move_signal(limb, 4.0, 2.0, 0.150, 1.0, 0.0, 9.0))[0];
    assert_eq!(r150.grade, M, "move +150 ms debe ser Miss (v1.1): {:?}", r150);
    assert!((r150.timing_error_ms - 150.0).abs() <= 10.0 + 1e-3, "L* = {}", r150.timing_error_ms);
    let rm150 = judge(&c, &reference, &move_signal(limb, 4.0, 2.0, -0.150, 1.0, 0.0, 9.0))[0];
    assert_eq!(rm150.grade, M, "move −150 ms: {:?}", rm150);
    // pie: gw = 150 ms → +140 ms es Good (búsqueda hasta ±300 ms)
    let f = Limb::LeftFoot;
    let rf = judge(&move_choreo(f), &move_signal(f, 4.0, 2.0, 0.0, 1.0, 0.0, 9.0), &move_signal(f, 4.0, 2.0, 0.140, 1.0, 0.0, 9.0))[0];
    assert_eq!(rf.grade, GD, "pie +140 ms: {:?}", rf);
    let rf2 = judge(&move_choreo(f), &move_signal(f, 4.0, 2.0, 0.0, 1.0, 0.0, 9.0), &move_signal(f, 4.0, 2.0, 0.220, 1.0, 0.0, 9.0))[0];
    assert_eq!(rf2.grade, M, "pie +220 ms: {:?}", rf2);
}

#[test]
fn move_still_is_miss_and_weak_is_gated() {
    let limb = Limb::LeftHand;
    let reference = move_signal(limb, 4.0, 2.0, 0.0, 1.0, 0.0, 9.0);
    let c = move_choreo(limb);
    let still = judge(&c, &reference, &rest(limb, 0.0, 9.0, 0.05, 3))[0];
    assert_eq!(still.grade, M, "quieto: {:?}", still);
    assert!(still.similarity < 0.2);
    // mismo gesto con 10 % de amplitud: la puerta de energía (0.25) escala el score → Miss
    let weak = judge(&c, &reference, &move_signal(limb, 4.0, 2.0, 0.0, 0.1, 0.0, 9.0))[0];
    assert_eq!(weak.grade, M, "gesto débil: {:?}", weak);
    assert!(weak.similarity < 0.5);
    // sin datos de la extremidad
    let none = judge(&c, &reference, &rest(Limb::RightHand, 0.0, 9.0, 0.0, 1))[0];
    assert_eq!(none.grade, M);
}

#[test]
fn move_without_reference_never_perfect() {
    let limb = Limb::RightHand;
    let c = move_choreo(limb);
    for gain in [0.3, 1.0, 3.0] {
        let r = judge(&c, &[], &move_signal(limb, 4.0, 2.0, 0.0, gain, 0.0, 9.0))[0];
        assert_ne!(r.grade, P, "sin referencia nunca Perfect (gain {}): {:?}", gain, r);
        assert_eq!(r.timing_error_ms, 0.0);
        assert!(r.similarity >= 0.0 && r.similarity <= 1.0);
        assert_eq!(r.grade == GD, r.similarity >= 0.5);
    }
    let big = judge(&c, &[], &move_signal(limb, 4.0, 2.0, 0.0, 3.0, 0.0, 9.0))[0];
    assert_eq!(big.grade, GD);
    let still = judge(&c, &[], &rest(limb, 0.0, 9.0, 0.05, 2))[0];
    assert_eq!(still.grade, M);
    // referencia que solo cubre OTRA extremidad → este limb va por el camino "sin referencia"
    let other = move_signal(Limb::LeftHand, 4.0, 2.0, 0.0, 1.0, 0.0, 9.0);
    let r = judge(&c, &other, &move_signal(limb, 4.0, 2.0, 0.0, 3.0, 0.0, 9.0))[0];
    assert_eq!(r.grade, GD, "{:?}", r);
    assert_eq!(r.timing_error_ms, 0.0);
}

#[test]
fn result_order_step_then_limb() {
    let c = choreo(vec![
        step(2.0, 0.5, &[Limb::RightFoot, Limb::RightHand, Limb::LeftHand], StepKind::Accent),
        step(2.0, 0.5, &[Limb::LeftFoot], StepKind::Accent),
        step(3.0, 1.0, &[Limb::RightHand, Limb::LeftFoot], StepKind::Move),
    ]);
    let r = judge(&c, &[], &rest(Limb::RightHand, 0.0, 5.0, 0.0, 1));
    let got: Vec<(i32, u8)> = r.iter().map(|x| (x.step_index, x.limb)).collect();
    assert_eq!(got, vec![(0, 0), (0, 1), (0, 3), (1, 2), (2, 1), (2, 2)]);
}

#[test]
fn emission_time_matches_contract_offline() {
    // Judge con todas las tramas ya empujadas: se emite en t + dur + 2·gw_escalado + 0.05.
    let c = choreo(vec![
        step(2.0, 0.5, &[Limb::RightHand], StepKind::Accent),
        step(2.0, 0.5, &[Limb::LeftFoot], StepKind::Accent),
        step(3.0, 1.0, &[Limb::LeftHand], StepKind::Move),
    ]);
    let mut s = punches(Limb::RightHand, &[2.0], 0.0, 8.0);
    s.extend(punches(Limb::LeftFoot, &[2.0], 0.0, 8.0));
    s.extend(rest(Limb::LeftHand, 0.0, 8.0, 0.0, 1));
    let frames = compute_features(&sort_t(s), 0.0);
    let mut j = make_judge();
    j.load(&c, &[], &JudgeConfig::default());
    j.push(&frames);
    let mut out = Vec::new();
    let checks = [(2.0 + 0.5 + 0.2 + 0.05, 0), (2.0 + 0.5 + 0.3 + 0.05, 1), (3.0 + 1.0 + 0.2 + 0.05, 2)];
    for (due, step_i) in checks {
        out.clear();
        j.update(due - 1e-3, &mut out);
        assert!(out.iter().all(|r| r.step_index != step_i), "paso {} emitido antes de {}", step_i, due);
        out.clear();
        j.update(due + 1e-6, &mut out);
        assert!(out.iter().any(|r| r.step_index == step_i), "paso {} no emitido en {}", step_i, due);
    }
    out.clear();
    j.update(1e9, &mut out);
    assert!(out.is_empty(), "un resultado se emitió dos veces");
}

#[test]
fn emission_without_data_is_exact_contract_time() {
    let c = choreo(vec![step(2.0, 0.5, &[Limb::RightHand], StepKind::Accent)]);
    let mut e = Engine::new();
    e.load_choreo(c, Vec::new());
    let mut out = Vec::new();
    e.update(2.749, &mut out);
    assert!(out.is_empty());
    e.update(2.7501, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].grade, M);
}

/// Simula tiempo real: fotogramas de 16 ms, en cada uno llegan las muestras con t ≤ ahora − latencia.
fn realtime(c: &Choreo, rf: Vec<FeatureFrame>, s: &[MotionSample], t_end: f64, latency: f64) -> Vec<(f64, StepResult)> {
    let mut e = Engine::new();
    e.load_choreo(c.clone(), rf);
    let mut out = Vec::new();
    let mut res = Vec::new();
    let mut idx = 0;
    let mut t = 0.0;
    while t < t_end {
        t += 0.016;
        let st = idx;
        while idx < s.len() && s[idx].t_ns as f64 * 1e-9 <= t - latency { idx += 1; }
        e.push_samples(&s[st..idx]);
        out.clear();
        e.update(t, &mut out);
        for r in &out { res.push((t, *r)); }
    }
    res
}

#[test]
fn realtime_emission_within_contract_plus_0_5s_and_same_grades() {
    let c = choreo(vec![
        step(2.0, 0.5, &[Limb::RightHand], StepKind::Accent),
        step(3.0, 0.5, &[Limb::RightHand], StepKind::Accent),
        step(4.0, 2.0, &[Limb::LeftHand], StepKind::Move),
    ]);
    let mut s = punches(Limb::RightHand, &[2.09, 3.0], 0.0, 9.0);
    let refm = move_signal(Limb::LeftHand, 4.0, 2.0, 0.0, 1.0, 0.0, 9.0);
    s.extend(refm.iter().copied());
    let s = sort_t(s);
    let rf = compute_features(&refm, 0.0);
    let off = judge_offline(&c, &rf, &s, &JudgeConfig::default());
    for latency in [0.0, 0.03] {
        let rt = realtime(&c, rf.clone(), &s, 9.0, latency);
        assert_eq!(rt.len(), off.len());
        for ((t, r), o) in rt.iter().zip(&off) {
            let st = &c.steps[r.step_index as usize];
            let due = st.t + st.dur + 0.2 + 0.05;
            assert!(*t >= due - 1e-9 && *t <= due + 0.5 + 0.016 + 1e-9, "paso {} emitido en {} (contrato {})", r.step_index, t, due);
            assert_eq!(r.grade, o.grade, "tiempo real ≠ offline en paso {} (latencia {})", r.step_index, latency);
        }
    }
}

/// Contrato B5: "se emite ... en orden de paso y luego de extremidad". En tiempo real, si una
/// extremidad espera sus tramas (extensión de C, hasta 0.5 s) y otra no tiene datos, la segunda
/// se emite antes aunque vaya después en el orden.
#[test]
fn realtime_emission_order_with_waiting_limb() {
    let c = choreo(vec![
        step(2.0, 0.5, &[Limb::LeftHand, Limb::RightHand], StepKind::Accent),
        step(2.1, 0.5, &[Limb::RightFoot], StepKind::Accent),
    ]);
    // Solo hay datos de left_hand y llegan con 0.6 s de latencia (Wi-Fi a tirones).
    let s = punches(Limb::LeftHand, &[2.0], 0.0, 5.0);
    let rt = realtime(&c, Vec::new(), &s, 5.0, 0.6);
    let got: Vec<(i32, u8)> = rt.iter().map(|(_, r)| (r.step_index, r.limb)).collect();
    assert_eq!(got, vec![(0, 0), (0, 1), (1, 3)], "orden de emisión");
}
