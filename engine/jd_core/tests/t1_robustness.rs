//! T1 · Robustez, determinismo bit a bit y rendimiento del motor (caja negra).
#[path = "t1_util.rs"]
mod util;

use jd_core::engine::{compute_features, judge_offline, reference_frames, Engine};
use jd_core::session::load_session;
use jd_core::types::*;
use std::panic::{catch_unwind, AssertUnwindSafe};
use util::*;

fn demo() -> (Choreo, Vec<FeatureFrame>) {
    let d = synth_dir().join("song_demo");
    let c = jd_core::choreo::load_choreo(&d.join("choreo.json")).unwrap();
    let r = reference_frames(&load_session(&d.join("ref_imu.jsonl")).unwrap());
    (c, r)
}

fn n_results(c: &Choreo) -> usize {
    c.steps.iter().map(|s| { let mut l = s.limbs.clone(); l.sort(); l.dedup(); l.len() }).sum()
}

fn engine_all(c: &Choreo, rf: &[FeatureFrame], s: &[MotionSample], chunk: usize) -> Vec<StepResult> {
    let mut e = Engine::new();
    e.load_choreo(c.clone(), rf.to_vec());
    let mut out = Vec::new();
    for ch in s.chunks(chunk.max(1)) {
        e.push_samples(ch);
        let t = ch.last().unwrap().t_ns as f64 * 1e-9;
        e.update(t, &mut out);
    }
    e.flush();
    e.update(f64::MAX, &mut out);
    out
}

fn bits(r: &[StepResult]) -> Vec<(i32, u8, u8, u32, u32)> {
    r.iter().map(|x| (x.step_index, x.limb, x.grade, x.similarity.to_bits(), x.timing_error_ms.to_bits())).collect()
}

fn ok_results(c: &Choreo, r: &[StepResult]) {
    assert_eq!(r.len(), n_results(c), "faltan o sobran resultados");
    for x in r {
        assert!(x.grade <= 2);
        assert!(x.similarity.is_finite() && x.similarity >= 0.0 && x.similarity <= 1.0, "similarity {:?}", x);
        assert!(x.timing_error_ms.is_finite(), "timing {:?}", x);
    }
}

// ------------------------------------------------------------------ determinismo

#[test]
fn offline_twice_is_bit_identical() {
    let (c, rf) = demo();
    let s = load_session(&synth_dir().join("noisy.jsonl")).unwrap().samples;
    let a = judge_offline(&c, &rf, &s, &JudgeConfig::default());
    let b = judge_offline(&c, &rf, &s, &JudgeConfig::default());
    assert_eq!(bits(&a), bits(&b));
    let f1 = compute_features(&s, 0.0);
    let f2 = compute_features(&s, 0.0);
    assert_eq!(f1, f2);
}

#[test]
fn engine_chunking_and_reset_are_bit_identical_to_offline() {
    let (c, rf) = demo();
    let s = sort_t(load_session(&synth_dir().join("late_40ms.jsonl")).unwrap().samples);
    let off = judge_offline(&c, &rf, &s, &JudgeConfig::default());
    for chunk in [1usize, 8, 37, 100000] {
        assert_eq!(bits(&engine_all(&c, &rf, &s, chunk)), bits(&off), "trozos de {}", chunk);
    }
    // reset y otra vuelta en el mismo motor
    let mut e = Engine::new();
    e.load_choreo(c.clone(), rf.clone());
    let mut first = Vec::new();
    e.push_samples(&s);
    e.flush();
    e.update(f64::MAX, &mut first);
    e.reset();
    let mut second = Vec::new();
    e.push_samples(&s);
    e.flush();
    e.update(f64::MAX, &mut second);
    assert_eq!(bits(&first), bits(&second), "reset no deja el motor como nuevo");
    assert_eq!(bits(&first), bits(&off));
}

#[test]
fn golden_grades_via_engine() {
    let (c, rf) = demo();
    let mut total = 0;
    let mut same = 0;
    for name in GOLDEN_SESSIONS {
        let s = load_session(&synth_dir().join(format!("{}.jsonl", name))).unwrap().samples;
        let r = judge_offline(&c, &rf, &s, &JudgeConfig::default());
        let g = load_golden(name);
        assert_eq!(r.len(), g.len(), "{}", name);
        for (a, b) in r.iter().zip(&g) {
            assert_eq!(a.step_index as i64, b.0);
            assert_eq!(Limb::from_u8(a.limb).name(), b.1);
            total += 1;
            if grade_name(a.grade) == b.2 { same += 1; }
            assert!((a.similarity as f64 - b.3).abs() < 1e-3, "{} paso {}: sim {} vs {}", name, b.0, a.similarity, b.3);
            assert!((a.timing_error_ms as f64 - b.4).abs() < 0.05, "{} paso {}: err {} vs {}", name, b.0, a.timing_error_ms, b.4);
        }
    }
    assert_eq!(same, total, "grades distintos del golden: {}/{}", same, total);
}

// ------------------------------------------------------------------ robustez

#[test]
fn empty_inputs() {
    assert!(compute_features(&[], 0.0).is_empty());
    let (c, rf) = demo();
    let r = judge_offline(&c, &rf, &[], &JudgeConfig::default());
    ok_results(&c, &r);
    assert!(r.iter().all(|x| x.grade == Grade::Miss as u8));
    // coreografía sin pasos
    let empty = choreo(vec![]);
    assert!(judge_offline(&empty, &rf, &punches(Limb::RightHand, &[1.0], 0.0, 2.0), &JudgeConfig::default()).is_empty());
    // archivos vacíos
    let d = tmp_dir("rob");
    std::fs::write(d.join("e.jsonl"), "").unwrap();
    assert!(load_session(&d.join("e.jsonl")).is_err());
    std::fs::create_dir_all(d.join("song")).unwrap();
    std::fs::write(d.join("song/choreo.json"), "").unwrap();
    let mut e = Engine::new();
    assert!(e.load_song(&d.join("song")).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn reference_file_missing_is_an_error() {
    let d = tmp_dir("rob_ref");
    let src = std::fs::read_to_string(synth_dir().join("song_demo/choreo.json")).unwrap();
    std::fs::write(d.join("choreo.json"), src).unwrap();
    let mut e = Engine::new();
    let err = e.load_song(&d).unwrap_err();
    assert!(err.contains("ref_imu"), "{}", err);
    let _ = std::fs::remove_dir_all(&d);
}

fn poisoned(kind: &str) -> Vec<MotionSample> {
    let mut s = punches(Limb::RightHand, &[2.0, 3.0, 6.0], 0.0, 8.0);
    for (i, x) in s.iter_mut().enumerate() {
        if i % 97 == 50 {
            match kind {
                "nan" => x.accel[0] = f32::NAN,
                "inf" => x.accel[1] = f32::INFINITY,
                "gyro_nan" => x.gyro[2] = f32::NAN,
                _ => {}
            }
        }
    }
    s
}

/// Muestras con NaN/inf (sensor defectuoso, paquete corrupto): el motor no debe entrar en pánico,
/// los resultados deben ser números válidos y una muestra aislada no debe anular la canción.
/// Hoy un solo NaN entra en el LPF IIR y en el cuaternión y los deja en NaN para siempre: todos los
/// pasos posteriores de esa extremidad dan Miss.
#[test]
#[ignore = "BUG (agente C, signal.rs): una sola muestra con NaN/inf en accel o gyro contamina para siempre el LPF y la orientación de esa extremidad (todos los pasos siguientes = Miss); no hay saneado de muestras no finitas"]
fn nan_and_inf_samples_do_not_break_the_engine() {
    let c = choreo(vec![
        step(2.0, 0.5, &[Limb::RightHand], StepKind::Accent),
        step(3.0, 0.5, &[Limb::RightHand], StepKind::Accent),
        step(6.0, 0.5, &[Limb::RightHand], StepKind::Accent),
    ]);
    let mut fails = Vec::new();
    for kind in ["nan", "inf", "gyro_nan"] {
        let s = poisoned(kind);
        let r = catch_unwind(AssertUnwindSafe(|| judge_offline(&c, &[], &s, &JudgeConfig::default())));
        let Ok(r) = r else { fails.push(format!("{}: pánico", kind)); continue };
        ok_results(&c, &r);
        // Las muestras buenas siguen puntuando (un valor no finito aislado no debe anular la canción).
        let p = r.iter().filter(|x| x.grade == Grade::Perfect as u8).count();
        if p < 2 { fails.push(format!("{}: {} Perfect de 3", kind, p)); }
    }
    assert!(fails.is_empty(), "{:?}", fails);
}

#[test]
fn nan_samples_do_not_panic_through_engine_push() {
    // Variante mínima: solo exige ausencia de pánico al empujar (lo que haría la API C).
    let c = choreo(vec![step(2.0, 0.5, &[Limb::RightHand], StepKind::Accent)]);
    let mut e = Engine::new();
    e.load_choreo(c, Vec::new());
    let s = poisoned("nan");
    e.push_samples(&s);
    e.flush();
}

#[test]
fn time_jumps_backwards_and_unordered_samples() {
    let c = choreo(vec![
        step(2.0, 0.5, &[Limb::RightHand], StepKind::Accent),
        step(4.0, 0.5, &[Limb::RightHand], StepKind::Accent),
        step(5.0, 1.0, &[Limb::LeftHand], StepKind::Move),
    ]);
    let mut base = punches(Limb::RightHand, &[2.0, 4.0], 0.0, 7.0);
    base.extend(move_signal(Limb::LeftHand, 5.0, 1.0, 0.0, 1.0, 0.0, 7.0));
    let base = sort_t(base);
    // 1) salto hacia atrás de 1 s a mitad (reloj del móvil corregido de golpe)
    let mut back = base.clone();
    let mid = back.len() / 2;
    for x in back[mid..].iter_mut() { x.t_ns -= 1_000_000_000; }
    // 2) desordenadas: intercambio de pares vecinos
    let mut unord = base.clone();
    for i in (0..unord.len() - 1).step_by(2) { unord.swap(i, i + 1); }
    // 3) duplicadas
    let mut dup = base.clone();
    let extra: Vec<_> = base.iter().step_by(5).copied().collect();
    dup.extend(extra);
    for (name, s) in [("atrás", back), ("desordenadas", unord.clone()), ("duplicadas", dup)] {
        let r = catch_unwind(AssertUnwindSafe(|| {
            let mut e = Engine::new();
            e.load_choreo(c.clone(), compute_features(&base, 0.0));
            let mut out = Vec::new();
            for ch in s.chunks(3) { e.push_samples(ch); }
            e.flush();
            e.update(f64::MAX, &mut out);
            out
        }));
        let r = r.unwrap_or_else(|_| panic!("pánico con muestras {}", name));
        ok_results(&c, &r);
    }
    // compute_features ordena: desordenadas offline == ordenadas offline (por extremidad)
    let key = |f: &FeatureFrame| (f.limb as u8, (f.t_song * 1000.0).round() as i64);
    let mut a = compute_features(&unord, 0.0);
    let mut b = compute_features(&base, 0.0);
    a.sort_by_key(key);
    b.sort_by_key(key);
    assert_eq!(a, b);
}

#[test]
fn absurd_timestamps_and_limbs() {
    let c = choreo(vec![step(2.0, 0.5, &[Limb::RightHand], StepKind::Accent)]);
    let mut s = punches(Limb::RightHand, &[2.0], 0.0, 3.0);
    s[10].t_ns = i64::MAX / 2;
    s[11].t_ns = i64::MIN / 2;
    s[12].limb = 77;
    s[13].flags = 0;
    let r = catch_unwind(AssertUnwindSafe(|| {
        let mut e = Engine::new();
        e.load_choreo(c.clone(), Vec::new());
        let mut out = Vec::new();
        e.push_samples(&s);
        e.flush();
        e.update(f64::MAX, &mut out);
        out
    }));
    let r = r.expect("pánico con t_ns extremos / limb desconocido");
    ok_results(&c, &r);
}

#[test]
fn update_with_nan_or_backwards_time_is_harmless() {
    let (c, rf) = demo();
    let mut e = Engine::new();
    e.load_choreo(c.clone(), rf);
    let mut out = Vec::new();
    e.update(f64::NAN, &mut out);
    e.update(-5.0, &mut out);
    e.update(10.0, &mut out);
    let n = out.len();
    e.update(5.0, &mut out); // atrás: no re-emite
    assert_eq!(out.len(), n);
    e.update(f64::INFINITY, &mut out);
    ok_results(&c, &out);
}

// ------------------------------------------------------------------ rendimiento

#[test]
fn perf_four_limbs_200hz() {
    // 4 extremidades × 200 Hz × 60 s, fotogramas de 16 ms (push + update por fotograma).
    let steps: Vec<Step> = (0..100).map(|i| {
        let t = 1.0 + i as f64 * 0.55;
        let l = Limb::ALL[i % 4];
        step(t, 0.5, &[l], if i % 3 == 0 { StepKind::Move } else { StepKind::Accent })
    }).collect();
    let c = choreo(steps);
    let mut s = Vec::new();
    for (k, l) in Limb::ALL.iter().enumerate() {
        let hits: Vec<f64> = (0..100).map(|i| 1.0 + i as f64 * 0.55 + k as f64 * 0.01).collect();
        let mut v = punches(*l, &hits, 0.0, 60.0);
        for (j, x) in v.iter_mut().enumerate() { x.accel[0] += (j as f32 * 0.37).sin() * 3.0; }
        s.extend(v);
    }
    let s = sort_t(s);
    let rf = compute_features(&s, 0.0);
    let mut e = Engine::new();
    e.load_choreo(c, rf);
    let mut out = Vec::new();
    let mut idx = 0;
    let mut t = 0.0;
    let mut frames = 0u32;
    let mut worst = 0.0f64;
    let t_start = std::time::Instant::now();
    while t < 60.5 {
        t += 0.016;
        let st = idx;
        while idx < s.len() && s[idx].t_ns as f64 * 1e-9 <= t { idx += 1; }
        let f0 = std::time::Instant::now();
        e.push_samples(&s[st..idx]);
        e.update(t, &mut out);
        worst = worst.max(f0.elapsed().as_secs_f64());
        frames += 1;
    }
    let mean_ms = t_start.elapsed().as_secs_f64() * 1e3 / frames as f64;
    let mode = if cfg!(debug_assertions) { "debug" } else { "release" };
    println!("T1 PERF [{}]: {} muestras, {} fotogramas, media {:.4} ms/fotograma, peor {:.3} ms", mode, s.len(), frames, mean_ms, worst * 1e3);
    assert!(out.len() == 100);
    if cfg!(debug_assertions) { assert!(mean_ms < 10.0); } else { assert!(mean_ms < 1.0, "objetivo < 1 ms: {:.4}", mean_ms); }
}
