//! T1 · B4 Rasgos (caja negra): tramas alineadas a 0.02 s, energía/pico/dirección, picos a ±1 muestra
//! (sobre la señal filtrada) y a ±10 ms del instante real, separación mínima, streaming = offline.
#[path = "t1_util.rs"]
mod util;

use jd_core::engine::compute_features;
use jd_core::features::make_feature_extractor;
use jd_core::signal::make_signal_processor;
use jd_core::types::*;
use util::*;

fn filtered(s: &[MotionSample]) -> Vec<FilteredSample> {
    let mut sp = make_signal_processor();
    let mut out = Vec::new();
    sp.process(s, 0.0, &mut out);
    out
}

fn peaks(frames: &[FeatureFrame], limb: Limb) -> Vec<f64> {
    frames.iter().filter(|f| f.limb == limb && f.is_peak).map(|f| f.peak_t_song).collect()
}

#[test]
fn frames_are_aligned_to_hop_and_ordered() {
    let s = punches(Limb::RightHand, &[1.0], 0.0, 3.0);
    let fr = compute_features(&s, 0.0);
    assert!(!fr.is_empty());
    let mut prev = f64::NEG_INFINITY;
    for f in &fr {
        let k = (f.t_song / 0.02).round();
        assert!((f.t_song - k * 0.02).abs() < 1e-9, "centro no alineado: {}", f.t_song);
        assert!(f.t_song > prev, "tramas desordenadas");
        prev = f.t_song;
    }
    // Cobertura: de la primera a la última muestra, sin huecos.
    let n = fr.len() as f64;
    let span = (fr.last().unwrap().t_song - fr[0].t_song) / 0.02 + 1.0;
    assert!((n - span).abs() < 0.5, "faltan tramas: {} de {}", n, span);
    assert!(fr[0].t_song <= s[0].t_ns as f64 * 1e-9 + 0.05 + 1e-9);
}

#[test]
fn frame_stats_match_definition() {
    let s = punches(Limb::LeftHand, &[1.0, 1.6], 0.0, 2.5);
    let fs = filtered(&s);
    let fr = compute_features(&s, 0.0);
    for f in fr.iter().step_by(7) {
        let inwin: Vec<_> = fs.iter().filter(|x| (x.t_song - f.t_song).abs() <= 0.05 + 1e-9).collect();
        assert!(!inwin.is_empty());
        let m: Vec<f64> = inwin.iter().map(|x| {
            let l = x.lin_accel;
            ((l[0] as f64).powi(2) + (l[1] as f64).powi(2) + (l[2] as f64).powi(2)).sqrt()
        }).collect();
        let e = (m.iter().map(|v| v * v).sum::<f64>() / m.len() as f64).sqrt();
        let p = m.iter().cloned().fold(0.0, f64::max);
        assert!((f.energy as f64 - e).abs() < 1e-4 * (1.0 + e), "energy {} vs {}", f.energy, e);
        assert!((f.peak as f64 - p).abs() < 1e-4 * (1.0 + p), "peak {} vs {}", f.peak, p);
        assert_eq!(f.quat, inwin.last().unwrap().quat, "quat = la de la última muestra de la trama");
        let mut mean = [0.0f64; 3];
        for x in &inwin { for i in 0..3 { mean[i] += x.lin_accel[i] as f64 / inwin.len() as f64; } }
        let mn = (mean[0] * mean[0] + mean[1] * mean[1] + mean[2] * mean[2]).sqrt();
        if mn < 0.5 - 1e-4 { assert_eq!(f.dir, [0.0; 3]); }
        else if mn > 0.5 + 1e-4 {
            for i in 0..3 { assert!((f.dir[i] as f64 - mean[i] / mn).abs() < 1e-4); }
        }
    }
}

#[test]
fn peak_within_one_sample_of_filtered_max_and_10ms_of_truth() {
    let hits = [1.0, 2.0071, 3.5133, 4.25, 6.0];
    let s = punches(Limb::RightHand, &hits, 0.0, 7.0);
    let fs = filtered(&s);
    let fr = compute_features(&s, 0.0);
    let p = peaks(&fr, Limb::RightHand);
    assert_eq!(p.len(), hits.len(), "picos: {:?}", p);
    for (&h, &pt) in hits.iter().zip(&p) {
        // máximo de |lin| filtrado cerca del golpe
        let (mut bt, mut bm) = (0.0, -1.0);
        for x in fs.iter().filter(|x| (x.t_song - h).abs() < 0.05) {
            let l = x.lin_accel;
            let m = ((l[0] as f64).powi(2) + (l[1] as f64).powi(2) + (l[2] as f64).powi(2)).sqrt();
            if m > bm { bm = m; bt = x.t_song; }
        }
        assert!((pt - bt).abs() <= DT + 1e-9, "pico {} vs máximo filtrado {}", pt, bt);
        assert!(pt - h >= -0.010 && pt - h <= 0.010, "pico {} a {:.1} ms del golpe {}", pt, (pt - h) * 1e3, h);
        // is_peak en la trama de centro más cercano
        let f = fr.iter().find(|f| f.is_peak && f.peak_t_song == pt).unwrap();
        assert!((f.t_song - pt).abs() <= 0.01 + 1e-9);
    }
    // peak_t_song = instante de una muestra
    for pt in &p { assert!(fs.iter().any(|x| x.t_song == *pt)); }
}

#[test]
fn min_separation_keeps_the_larger() {
    // Dos golpes a 100 ms: solo uno (el mayor, el segundo). Otro par a 200 ms: ambos.
    let s: Vec<_> = times(0.0, 5.0).into_iter().map(|t| {
        let g = |mu: f64, a: f64| a * (-(t - mu) * (t - mu) / (2.0 * 0.01 * 0.01)).exp();
        sample(t, Limb::LeftHand, [0.0, G + g(1.0, 15.0) + g(1.1, 25.0) + g(3.0, 20.0) + g(3.2, 20.0), 0.0], [0.0; 3])
    }).collect();
    let fr = compute_features(&s, 0.0);
    let p = peaks(&fr, Limb::LeftHand);
    assert_eq!(p.len(), 3, "picos {:?}", p);
    assert!((p[0] - 1.1).abs() < 0.015, "debe ganar el mayor: {:?}", p);
    assert!((p[1] - 3.0).abs() < 0.015 && (p[2] - 3.2).abs() < 0.015);
    for w in p.windows(2) { assert!(w[1] - w[0] >= 0.150 - 1e-9); }
}

#[test]
fn below_peak_min_is_not_a_peak() {
    let s: Vec<_> = times(0.0, 3.0).into_iter().map(|t| {
        sample(t, Limb::RightHand, [0.0, G + 7.0 * (-(t - 1.0) * (t - 1.0) / (2.0 * 0.012 * 0.012)).exp(), 0.0], [0.0; 3])
    }).collect();
    assert!(peaks(&compute_features(&s, 0.0), Limb::RightHand).is_empty());
}

#[test]
fn peak_rel_rejects_peaks_in_high_energy_background() {
    // Fondo de |lin| ≈ 10 m/s² (fuera de la puerta) y un "pico" de 12: no supera 1.5 × mediana.
    let s: Vec<_> = times(0.0, 4.0).into_iter().map(|t| {
        let bump = 2.0 * (-(t - 2.0) * (t - 2.0) / (2.0 * 0.012 * 0.012)).exp();
        sample(t, Limb::RightHand, [10.0 + bump, G, 0.0], [0.0; 3])
    }).collect();
    let p = peaks(&compute_features(&s, 0.0), Limb::RightHand);
    assert!(p.iter().all(|&t| (t - 2.0).abs() > 0.2), "pico aceptado en fondo de alta energía: {:?}", p);
}

#[test]
fn streaming_in_chunks_equals_offline() {
    let mut s = punches(Limb::RightHand, &[1.0, 1.7, 2.9], 0.0, 4.0);
    s.extend(move_signal(Limb::LeftFoot, 0.5, 2.5, 0.0, 1.0, 0.0, 4.0));
    let s = sort_t(s);
    let off = compute_features(&s, 0.0);
    for chunk in [1usize, 3, 16, 100] {
        let mut sp = make_signal_processor();
        let mut fx = make_feature_extractor();
        let mut fr = Vec::new();
        for c in s.chunks(chunk) {
            let mut fs = Vec::new();
            sp.process(c, 0.0, &mut fs);
            fx.push(&fs, &mut fr);
        }
        fx.flush(&mut fr);
        let mut a = off.clone();
        let mut b = fr.clone();
        let key = |f: &FeatureFrame| (f.limb as u8, (f.t_song * 1000.0).round() as i64);
        a.sort_by_key(key);
        b.sort_by_key(key);
        assert_eq!(a, b, "trozos de {}", chunk);
    }
}

#[test]
fn frame_emitted_only_after_window_closes() {
    let s = rest(Limb::RightHand, 0.0, 2.0, 0.0, 1);
    let mut sp = make_signal_processor();
    let mut fx = make_feature_extractor();
    let mut fs = Vec::new();
    sp.process(&s, 0.0, &mut fs);
    for i in 0..fs.len() {
        let mut fr = Vec::new();
        fx.push(&fs[i..i + 1], &mut fr);
        for f in &fr {
            assert!(fs[i].t_song > f.t_song + 0.05, "trama {} emitida con la muestra {}", f.t_song, fs[i].t_song);
        }
    }
}

#[test]
fn gap_produces_no_empty_frames() {
    let mut s = rest(Limb::RightHand, 0.0, 1.0, 0.0, 1);
    s.extend(rest(Limb::RightHand, 3.0, 4.0, 0.0, 1));
    let fr = compute_features(&s, 0.0);
    assert!(fr.iter().all(|f| !(f.t_song > 1.1 && f.t_song < 2.9)), "tramas sin muestras emitidas");
}
