//! Agente C · rendimiento: 4 extremidades × 200 Hz × 60 s en fotogramas de 16 ms.
//! Objetivo: < 1 ms de media por fotograma en release (`cargo test --release --test c_perf`).
use jd_core::engine::{compute_features, Engine};
use jd_core::types::*;
use std::time::Instant;

fn gen(limb: Limb, dur: f64, shift: f64) -> Vec<MotionSample> {
    let n = (dur / 0.005) as usize;
    (0..n).map(|i| {
        let t = i as f64 * 0.005;
        let x = t - shift + limb as u8 as f64;
        let hit = 25.0 * (-((x % 0.5) - 0.25).powi(2) / (2.0 * 0.012f64.powi(2))).exp();
        MotionSample {
            t_ns: (t * 1e9).round() as i64,
            source_id: limb as u32,
            limb: limb as u8,
            flags: SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO,
            _reserved: 0,
            accel: [(5.0 * (2.3 * x).sin() + hit) as f32, (9.8 + 4.0 * (3.1 * x).cos()) as f32, (3.0 * (4.4 * x).sin()) as f32],
            gyro: [(0.8 * (1.1 * x).sin()) as f32, (0.5 * (0.7 * x).cos()) as f32, 0.2],
        }
    }).collect()
}

#[test]
fn realtime_budget() {
    let dur = 60.0;
    let mut steps = Vec::new();
    let mut t = 1.0;
    let mut i = 0;
    while t < dur - 1.0 {
        let kind = if i % 3 == 0 { StepKind::Move } else { StepKind::Accent };
        steps.push(Step { t, dur: if kind == StepKind::Move { 1.0 } else { 0.25 }, limbs: Limb::ALL.to_vec(), icon: "x".into(), kind });
        t += 0.5;
        i += 1;
    }
    let choreo = Choreo { steps, ..Default::default() };
    let mut reference = Vec::new();
    for l in Limb::ALL { reference.extend(gen(l, dur, 0.0)); }
    let ref_frames = compute_features(&reference, 0.0);
    let mut player = Vec::new();
    for l in Limb::ALL { player.extend(gen(l, dur, 0.03)); }
    player.sort_by_key(|s| s.t_ns);

    let mut e = Engine::new();
    e.load_choreo(choreo, ref_frames);
    let mut out = Vec::new();
    let mut idx = 0;
    let mut frames = 0u32;
    let mut worst = 0.0f64;
    let start = Instant::now();
    let mut song = 0.0;
    while song < dur + 1.0 {
        song += 0.016;
        let f0 = Instant::now();
        let s = idx;
        while idx < player.len() && player[idx].t_ns as f64 * 1e-9 <= song { idx += 1; }
        e.push_samples(&player[s..idx]);
        e.update(song, &mut out);
        worst = worst.max(f0.elapsed().as_secs_f64());
        frames += 1;
    }
    let total = start.elapsed().as_secs_f64();
    let mean_ms = total * 1000.0 / frames as f64;
    println!("c_perf: {} fotogramas, media {:.4} ms, peor {:.4} ms, {} resultados", frames, mean_ms, worst * 1000.0, out.len());
    assert!(!out.is_empty());
    if !cfg!(debug_assertions) {
        assert!(mean_ms < 1.0, "media {:.4} ms por fotograma", mean_ms);
    }
}
