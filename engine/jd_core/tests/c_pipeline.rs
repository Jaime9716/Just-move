//! Agente C · pruebas de punta a punta de B3→B4→B5 con muestras sintéticas en memoria.
use jd_core::engine::{compute_features, judge_offline, Engine};
use jd_core::types::*;

const G: f64 = 9.80665;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
}

/// Muestras a 200 Hz de `t0` a `t1` con aceleración lineal (mundo = sensor, sin rotación) `f(t)`.
fn gen(limb: Limb, t0: f64, t1: f64, noise: f64, seed: u64, f: &dyn Fn(f64) -> [f64; 3]) -> Vec<MotionSample> {
    let mut rng = Lcg(seed);
    let n = ((t1 - t0) / 0.005).round() as usize;
    (0..n).map(|i| {
        let t = t0 + i as f64 * 0.005;
        let a = f(t);
        MotionSample {
            t_ns: (t * 1e9).round() as i64,
            source_id: limb as u32 + 1,
            limb: limb as u8,
            flags: SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO,
            _reserved: 0,
            accel: [(a[0] + noise * rng.next()) as f32, (a[1] + G + noise * rng.next()) as f32, (a[2] + noise * rng.next()) as f32],
            gyro: [(0.01 * rng.next()) as f32, (0.01 * rng.next()) as f32, (0.01 * rng.next()) as f32],
        }
    }).collect()
}

fn bump(t: f64, at: f64, amp: f64, sigma: f64) -> f64 { amp * (-(t - at) * (t - at) / (2.0 * sigma * sigma)).exp() }

/// Golpes (accent) en los instantes `hits`.
fn punches(limb: Limb, hits: Vec<f64>, noise: f64) -> Vec<MotionSample> {
    gen(limb, 0.0, 8.0, noise, 7, &move |t| {
        let v: f64 = hits.iter().map(|&h| bump(t, h, 25.0, 0.012)).sum();
        [0.3 * v, v, 0.0]
    })
}

/// Movimiento continuo no periódico desplazado `shift` s (y escalado).
fn dance(limb: Limb, shift: f64, scale: f64, seed: u64) -> Vec<MotionSample> {
    gen(limb, 0.0, 8.0, 0.2, seed, &move |t| {
        let x = t - shift;
        let env = if (1.0..7.0).contains(&x) { 1.0 } else { 0.0 };
        [
            scale * env * (6.0 * (2.3 * x).sin() + 4.0 * (5.7 * x + 0.4 * (1.3 * x).sin()).sin()),
            scale * env * (5.0 * (3.1 * x + 1.0).sin() + 3.0 * (7.9 * x).cos() * (0.7 * x).sin()),
            scale * env * 3.0 * (4.4 * x).sin(),
        ]
    })
}

fn accent_choreo(limb: Limb, ts: &[f64]) -> Choreo {
    Choreo {
        title: "sintética".into(),
        steps: ts.iter().map(|&t| Step { t, dur: 0.25, limbs: vec![limb], icon: "punch".into(), kind: StepKind::Accent }).collect(),
        ..Default::default()
    }
}

fn move_choreo(limbs: &[Limb]) -> Choreo {
    Choreo {
        title: "sintética".into(),
        steps: [1.5, 2.5, 3.5, 4.5, 5.5].iter().map(|&t| Step { t, dur: 0.8, limbs: limbs.to_vec(), icon: "wave".into(), kind: StepKind::Move }).collect(),
        ..Default::default()
    }
}


#[test]
fn peak_detected_within_one_sample() {
    let hits = vec![1.0, 2.003, 3.5, 4.25];
    let frames = compute_features(&punches(Limb::RightHand, hits.clone(), 0.3), 0.0);
    let peaks: Vec<f64> = frames.iter().filter(|f| f.is_peak).map(|f| f.peak_t_song).collect();
    assert_eq!(peaks.len(), hits.len(), "{:?}", peaks);
    // El LPF de 20 Hz retrasa el máximo unos 5–10 ms (2 muestras); el ±1 muestra exacto de B4 se
    // prueba en features.rs sobre muestras ya filtradas. Aquí: mismo retraso para todos los golpes.
    let lag = peaks[0] - hits[0];
    assert!((0.0..=0.010 + 1e-9).contains(&lag), "retraso {}", lag);
    for (p, h) in peaks.iter().zip(&hits) { assert!((p - h - lag).abs() <= 0.005 + 1e-9, "{} vs {}", p, h); }
}

#[test]
fn accent_offsets() {
    let beats = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let c = accent_choreo(Limb::RightHand, &beats);
    let cfg = JudgeConfig::default();
    let expect = [(0.0, Grade::Perfect), (0.04, Grade::Perfect), (-0.04, Grade::Perfect), (0.08, Grade::Good), (-0.08, Grade::Good), (0.15, Grade::Miss)];
    for (off, g) in expect {
        let hits: Vec<f64> = beats.iter().map(|b| b + off).collect();
        let r = judge_offline(&c, &[], &punches(Limb::RightHand, hits, 0.3), &cfg);
        assert_eq!(r.len(), beats.len());
        for x in &r {
            assert_eq!(x.grade, g as u8, "desfase {} → {:?}", off, x);
            if g != Grade::Miss { assert!((x.timing_error_ms as f64 - off * 1000.0).abs() <= 10.0 + 1e-3, "{:?}", x); }
        }
    }
}

#[test]
fn feet_use_scaled_window() {
    let beats = [1.0, 2.0, 3.0];
    let cfg = JudgeConfig::default();
    let hits: Vec<f64> = beats.iter().map(|b| b + 0.06).collect(); // ~65 ms con el retraso del LPF
    let foot = judge_offline(&accent_choreo(Limb::LeftFoot, &beats), &[], &punches(Limb::LeftFoot, hits.clone(), 0.3), &cfg);
    let hand = judge_offline(&accent_choreo(Limb::LeftHand, &beats), &[], &punches(Limb::LeftHand, hits, 0.3), &cfg);
    assert!(foot.iter().all(|r| r.grade == Grade::Perfect as u8), "{:?}", foot);
    assert!(hand.iter().all(|r| r.grade == Grade::Good as u8), "{:?}", hand);
    let hits: Vec<f64> = beats.iter().map(|b| b + 0.125).collect(); // ~130 ms
    let foot = judge_offline(&accent_choreo(Limb::LeftFoot, &beats), &[], &punches(Limb::LeftFoot, hits.clone(), 0.3), &cfg);
    let hand = judge_offline(&accent_choreo(Limb::LeftHand, &beats), &[], &punches(Limb::LeftHand, hits, 0.3), &cfg);
    assert!(foot.iter().all(|r| r.grade == Grade::Good as u8), "{:?}", foot);
    assert!(hand.iter().all(|r| r.grade == Grade::Miss as u8), "{:?}", hand);
}

#[test]
fn move_with_reference() {
    let limbs = [Limb::LeftHand, Limb::RightFoot];
    let c = move_choreo(&limbs);
    let cfg = JudgeConfig::default();
    let mut reference = dance(Limb::LeftHand, 0.0, 1.0, 1);
    reference.extend(dance(Limb::RightFoot, 0.0, 1.0, 2));
    let ref_frames = compute_features(&reference, 0.0);

    let same = judge_offline(&c, &ref_frames, &reference, &cfg);
    assert_eq!(same.len(), 10);
    assert!(same.iter().all(|r| r.grade == Grade::Perfect as u8 && r.similarity > 0.95), "{:?}", same);

    // Misma danza con otro ruido: Perfect.
    let mut other = dance(Limb::LeftHand, 0.0, 1.0, 11);
    other.extend(dance(Limb::RightFoot, 0.0, 1.0, 12));
    let r = judge_offline(&c, &ref_frames, &other, &cfg);
    assert!(r.iter().all(|r| r.grade == Grade::Perfect as u8), "{:?}", r);

    // 40 ms tarde: Perfect o Good con error ≈ 40 ms.
    let mut late = dance(Limb::LeftHand, 0.04, 1.0, 3);
    late.extend(dance(Limb::RightFoot, 0.04, 1.0, 4));
    let r = judge_offline(&c, &ref_frames, &late, &cfg);
    for x in &r {
        assert!(x.grade >= Grade::Good as u8, "{:?}", x);
        assert!((x.timing_error_ms - 40.0).abs() <= 20.0 + 1e-3, "{:?}", x);
    }

    // 150 ms tarde (fuera de la ventana de manos): nunca Perfect y el desfase satura en +100 ms.
    // Nota: según el contrato v1 sale Good (L* = +gw con score alto); ver C-report (propuesta).
    let late = dance(Limb::LeftHand, 0.15, 1.0, 5);
    let r = judge_offline(&c, &ref_frames, &late, &cfg);
    for x in r.iter().filter(|x| x.limb == Limb::LeftHand as u8) {
        assert!(x.grade != Grade::Perfect as u8 && x.timing_error_ms == 100.0, "{:?}", x);
    }
    // 400 ms tarde: Miss casi siempre.
    let late = dance(Limb::LeftHand, 0.4, 1.0, 6);
    let r = judge_offline(&c, &ref_frames, &late, &cfg);
    let hands: Vec<_> = r.iter().filter(|x| x.limb == Limb::LeftHand as u8).collect();
    let misses = hands.iter().filter(|x| x.grade == Grade::Miss as u8).count();
    println!("400 ms: {:?}", hands);
    assert!(misses * 10 >= hands.len() * 6, "{:?}", hands);

    // Quieto: Miss en todo.
    let mut still = gen(Limb::LeftHand, 0.0, 8.0, 0.2, 9, &|_| [0.0; 3]);
    still.extend(gen(Limb::RightFoot, 0.0, 8.0, 0.2, 10, &|_| [0.0; 3]));
    let r = judge_offline(&c, &ref_frames, &still, &cfg);
    assert!(r.iter().all(|r| r.grade == Grade::Miss as u8), "{:?}", r);
    // Sin datos del jugador: Miss.
    let r = judge_offline(&c, &ref_frames, &[], &cfg);
    assert!(r.iter().all(|r| r.grade == Grade::Miss as u8));
}

#[test]
fn move_without_reference_never_perfect() {
    let c = move_choreo(&[Limb::RightHand]);
    let cfg = JudgeConfig::default();
    let r = judge_offline(&c, &[], &dance(Limb::RightHand, 0.0, 2.0, 1), &cfg);
    assert!(r.iter().all(|x| x.grade == Grade::Good as u8 && x.timing_error_ms == 0.0), "{:?}", r);
    let r = judge_offline(&c, &[], &gen(Limb::RightHand, 0.0, 8.0, 0.2, 2, &|_| [0.0; 3]), &cfg);
    assert!(r.iter().all(|x| x.grade == Grade::Miss as u8), "{:?}", r);
    for scale in [0.5, 1.0, 3.0, 10.0] {
        let r = judge_offline(&c, &[], &dance(Limb::RightHand, 0.0, scale, 3), &cfg);
        assert!(r.iter().all(|x| x.grade != Grade::Perfect as u8));
    }
}

#[test]
fn deterministic_bit_for_bit() {
    let mut c = move_choreo(&[Limb::LeftHand, Limb::RightHand]);
    c.steps.extend(accent_choreo(Limb::RightHand, &[1.2, 2.2, 6.6]).steps);
    c.steps.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
    let mut reference = dance(Limb::LeftHand, 0.0, 1.0, 1);
    reference.extend(dance(Limb::RightHand, 0.0, 1.0, 2));
    let ref_frames = compute_features(&reference, 0.0);
    let mut player = dance(Limb::LeftHand, 0.03, 0.8, 5);
    player.extend(punches(Limb::RightHand, vec![1.23, 2.1, 6.6], 0.5));
    let cfg = JudgeConfig::default();

    let a = judge_offline(&c, &ref_frames, &player, &cfg);
    let b = judge_offline(&c, &ref_frames, &player, &cfg);
    let bits = |v: &[StepResult]| v.iter().map(|r| (r.step_index, r.limb, r.grade, r.similarity.to_bits(), r.timing_error_ms.to_bits())).collect::<Vec<_>>();
    assert_eq!(bits(&a), bits(&b));
    let fa = compute_features(&player, 0.0);
    let fb = compute_features(&player, 0.0);
    assert_eq!(fa.len(), fb.len());
    for (x, y) in fa.iter().zip(&fb) {
        assert_eq!((x.t_song.to_bits(), x.energy.to_bits(), x.is_peak, x.peak_t_song.to_bits()), (y.t_song.to_bits(), y.energy.to_bits(), y.is_peak, y.peak_t_song.to_bits()));
    }

    // El motor en tiempo real (trozos de 16 ms) da exactamente lo mismo que el juicio offline.
    let mut sorted = player.clone();
    sorted.sort_by_key(|s| s.t_ns);
    let mut e = Engine::new();
    e.load_choreo(c.clone(), ref_frames.clone());
    let mut rt = Vec::new();
    let mut idx = 0;
    let mut t = 0.0;
    while t < 9.0 {
        t += 0.016;
        let s = idx;
        while idx < sorted.len() && sorted[idx].t_ns as f64 * 1e-9 <= t { idx += 1; }
        e.push_samples(&sorted[s..idx]);
        e.update(t, &mut rt);
    }
    e.flush();
    e.update(f64::MAX, &mut rt);
    let mut rt_sorted = rt.clone();
    rt_sorted.sort_by_key(|r| (r.step_index, r.limb));
    assert_eq!(bits(&rt_sorted), bits(&a));
}
