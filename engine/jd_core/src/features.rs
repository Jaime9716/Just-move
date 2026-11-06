//! B4 · Rasgos (contracts/judge_algorithm_v1.md). AGENTE C.
//!
//! Por extremidad, tramas centradas en k·FRAME_HOP_S que usan las muestras con
//! |t − centro| ≤ FRAME_WIN_S/2. Picos sobre la serie de muestras |lin_accel|.
//!
//! Funcionamiento en flujo (el resultado es idéntico al cálculo "offline" sobre toda la serie):
//! 1. Una trama se CALCULA cuando llega una muestra con t > centro + WIN/2.
//! 2. La muestra i se EVALÚA como candidata a pico cuando existe la muestra i+1 y ya se han
//!    calculado todas las tramas con centro ≤ t_i (para la mediana de energía del último 1 s).
//! 3. Regla secuencial de separación: un candidato a < PEAK_MIN_SEP_S del último pico aceptado
//!    lo sustituye si es mayor (si no, se descarta). Un pico es definitivo cuando la siguiente
//!    muestra sin evaluar está a ≥ PEAK_MIN_SEP_S de él.
//! 4. Una trama se EMITE cuando ningún pico aún no decidido puede caer en ella. Por eso las tramas
//!    salen con un retraso de ~0.06 s (sin picos) a ~0.25 s (tras un pico) respecto a su centro.
//!
//! Sigue proto/ALGORITHM.md §1 (normativo desde v1.1): ventana |t − t_c| ≤ WIN/2 + 1e-9;
//! máximo local = m[i] > m[i−1] y m[i] ≥ m[i+1] (la primera y la última muestra nunca son pico);
//! mediana de las tramas con centro en (t_i − 1, t_i] (si no hay ninguna, solo cuenta PEAK_MIN);
//! el pico va a la trama floor(t/hop + 0.5); en tramas sin pico `peak_t_song` = 0.0.
use crate::types::{FeatureExtractor, FeatureFrame, FilteredSample, Limb};
use std::collections::VecDeque;

/// Parámetros de B4 (valores por defecto del contrato v1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureParams {
    pub frame_win_s: f64,
    pub frame_hop_s: f64,
    pub peak_min: f64,
    pub peak_rel: f64,
    pub peak_rel_window_s: f64,
    pub peak_min_sep_s: f64,
    pub dir_min_norm: f64,
}

impl Default for FeatureParams {
    fn default() -> Self {
        FeatureParams {
            frame_win_s: 0.100,
            frame_hop_s: 0.020,
            peak_min: 8.0,
            peak_rel: 1.5,
            peak_rel_window_s: 1.0,
            peak_min_sep_s: 0.150,
            dir_min_norm: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct S {
    t: f64,
    m: f64,
    lin: [f64; 3],
    quat: [f32; 4],
}

/// Tolerancia de las comparaciones de ventanas de tiempo (proto/ALGORITHM.md §1).
pub const EPS_T: f64 = 1e-9;

/// Trama que recibe un pico en t: k = floor(t/hop + 0.5) (proto/ALGORITHM.md §1).
pub fn nearest_frame_index(t: f64, hop: f64) -> i64 {
    (t / hop + 0.5).floor() as i64
}

#[derive(Default)]
struct LimbFx {
    samples: VecDeque<S>,
    base: usize,   // índice absoluto de samples[0]
    total: usize,  // muestras recibidas
    next_k: Option<i64>,
    eval_i: usize, // siguiente muestra a evaluar como candidata
    energy_hist: VecDeque<(f64, f64)>, // (centro, energía) de tramas calculadas
    provisional: Option<(f64, f64)>,   // (t, m) del último pico aceptado aún no definitivo
    final_peaks: VecDeque<f64>,
    pending: VecDeque<(i64, FeatureFrame)>,
    latest_t: f64,
}

impl LimbFx {
    fn new() -> Self { LimbFx { eval_i: 1, ..Default::default() } }

    fn s(&self, abs: usize) -> &S { &self.samples[abs - self.base] }

    fn center(p: &FeatureParams, k: i64) -> f64 { k as f64 * p.frame_hop_s }

    /// Primera trama que contiene t.
    fn first_k_containing(p: &FeatureParams, t: f64) -> i64 {
        let half = p.frame_win_s / 2.0 + EPS_T;
        let mut k = ((t - half) / p.frame_hop_s).ceil() as i64;
        while (t - Self::center(p, k - 1)).abs() <= half { k -= 1; }
        while !((t - Self::center(p, k)).abs() <= half) && Self::center(p, k) < t { k += 1; }
        k
    }

    fn compute_frame(&self, p: &FeatureParams, limb: Limb, k: i64) -> Option<FeatureFrame> {
        let half = p.frame_win_s / 2.0 + EPS_T;
        let c = Self::center(p, k);
        let mut n = 0usize;
        let mut sum2 = 0.0;
        let mut peak = -1.0;
        let mut sl = [0.0f64; 3];
        let mut quat = [1.0f32, 0.0, 0.0, 0.0];
        for s in &self.samples {
            if (s.t - c).abs() <= half {
                n += 1;
                sum2 += s.m * s.m;
                if s.m > peak { peak = s.m; }
                for i in 0..3 { sl[i] += s.lin[i]; }
                quat = s.quat;
            }
        }
        if n == 0 { return None; }
        let nf = n as f64;
        let mean = [sl[0] / nf, sl[1] / nf, sl[2] / nf];
        let mn = (mean[0] * mean[0] + mean[1] * mean[1] + mean[2] * mean[2]).sqrt();
        let dir = if mn < p.dir_min_norm { [0.0f32; 3] } else {
            [(mean[0] / mn) as f32, (mean[1] / mn) as f32, (mean[2] / mn) as f32]
        };
        Some(FeatureFrame {
            t_song: c,
            limb,
            energy: (sum2 / nf).sqrt() as f32,
            peak: peak as f32,
            is_peak: false,
            peak_t_song: 0.0,
            dir,
            quat,
        })
    }

    /// Calcula tramas cuyo final ya pasó (o todas, si `all`).
    fn compute_frames(&mut self, p: &FeatureParams, limb: Limb, all: bool) {
        if self.samples.is_empty() { return; }
        let half = p.frame_win_s / 2.0 + EPS_T;
        if self.next_k.is_none() {
            let t0 = self.samples[0].t;
            self.next_k = Some(Self::first_k_containing(p, t0));
        }
        loop {
            let k = self.next_k.unwrap();
            let c = Self::center(p, k);
            let ready = if all { c - half <= self.latest_t } else { self.latest_t > c + half };
            if !ready { break; }
            match self.compute_frame(p, limb, k) {
                Some(f) => {
                    self.energy_hist.push_back((c, f.energy as f64));
                    self.pending.push_back((k, f));
                    self.next_k = Some(k + 1);
                }
                None => {
                    // Hueco: saltar a la primera trama que contenga la siguiente muestra posterior.
                    let next = self.samples.iter().map(|s| s.t).filter(|&t| t > c + half)
                        .fold(f64::INFINITY, f64::min);
                    let nk = if next.is_finite() { Self::first_k_containing(p, next) } else { k + 1 };
                    self.next_k = Some(nk.max(k + 1));
                }
            }
        }
    }

    fn median_energy(&self, p: &FeatureParams, t: f64, scratch: &mut Vec<f64>) -> Option<f64> {
        scratch.clear();
        let lo = t - p.peak_rel_window_s;
        for &(c, e) in &self.energy_hist {
            if c > lo && c <= t { scratch.push(e); }
        }
        if scratch.is_empty() { return None; }
        scratch.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = scratch.len();
        Some(if n % 2 == 1 { scratch[n / 2] } else { 0.5 * (scratch[n / 2 - 1] + scratch[n / 2]) })
    }

    fn evaluate(&mut self, p: &FeatureParams, all_frames_done: bool, scratch: &mut Vec<f64>) {
        while self.eval_i + 1 < self.total {
            let i = self.eval_i;
            let si = *self.s(i);
            if !all_frames_done {
                match self.next_k {
                    Some(k) if Self::center(p, k) > si.t => {}
                    _ => break,
                }
            }
            let prev = self.s(i - 1).m;
            let next = self.s(i + 1).m;
            if si.m > prev && si.m >= next && si.m >= p.peak_min {
                let ok = match self.median_energy(p, si.t, scratch) {
                    Some(med) => si.m >= p.peak_rel * med,
                    None => true,
                };
                if ok {
                    match self.provisional {
                        Some((pt, pm)) if si.t - pt < p.peak_min_sep_s => {
                            if si.m > pm { self.provisional = Some((si.t, si.m)); }
                        }
                        Some((pt, _)) => {
                            self.final_peaks.push_back(pt);
                            self.provisional = Some((si.t, si.m));
                        }
                        None => self.provisional = Some((si.t, si.m)),
                    }
                }
            }
            self.eval_i += 1;
            // ¿El pico provisional ya no puede ser sustituido?
            if let Some((pt, _)) = self.provisional {
                let tn = self.s(self.eval_i).t;
                if tn - pt >= p.peak_min_sep_s {
                    self.final_peaks.push_back(pt);
                    self.provisional = None;
                }
            }
            // La mediana solo mira 1 s hacia atrás.
            let lo = si.t - p.peak_rel_window_s;
            while let Some(&(c, _)) = self.energy_hist.front() {
                if c <= lo { self.energy_hist.pop_front(); } else { break; }
            }
        }
    }

    fn emit(&mut self, p: &FeatureParams, flush: bool, out: &mut Vec<FeatureFrame>) {
        let limit_k = if flush { i64::MAX } else {
            let idx = self.eval_i.min(self.total.saturating_sub(1));
            let mut u = if self.total > 0 { self.s(idx).t.min(self.latest_t) } else { return };
            if let Some((pt, _)) = self.provisional { u = u.min(pt); }
            nearest_frame_index(u, p.frame_hop_s)
        };
        while let Some(&(k, _)) = self.pending.front() {
            if k >= limit_k { break; }
            let (k, mut f) = self.pending.pop_front().unwrap();
            while let Some(&pt) = self.final_peaks.front() {
                let pk = nearest_frame_index(pt, p.frame_hop_s);
                if pk < k { self.final_peaks.pop_front(); continue; }
                if pk == k {
                    f.is_peak = true;
                    f.peak_t_song = pt;
                    self.final_peaks.pop_front();
                }
                break;
            }
            out.push(f);
        }
    }

    fn prune(&mut self, p: &FeatureParams) {
        let half = p.frame_win_s / 2.0 + EPS_T;
        let cmin = match self.next_k { Some(k) => Self::center(p, k) - half, None => return };
        let keep_from = self.eval_i.saturating_sub(1);
        while let Some(s) = self.samples.front() {
            if self.base < keep_from && s.t < cmin {
                self.samples.pop_front();
                self.base += 1;
            } else { break; }
        }
    }

    fn push(&mut self, p: &FeatureParams, fs: &FilteredSample, out: &mut Vec<FeatureFrame>, scratch: &mut Vec<f64>) {
        let lin = [fs.lin_accel[0] as f64, fs.lin_accel[1] as f64, fs.lin_accel[2] as f64];
        let m = (lin[0] * lin[0] + lin[1] * lin[1] + lin[2] * lin[2]).sqrt();
        self.samples.push_back(S { t: fs.t_song, m, lin, quat: fs.quat });
        self.total += 1;
        if self.total == 1 || fs.t_song > self.latest_t { self.latest_t = fs.t_song; }
        self.compute_frames(p, fs.limb, false);
        self.evaluate(p, false, scratch);
        self.emit(p, false, out);
        self.prune(p);
    }

    fn flush(&mut self, p: &FeatureParams, limb: Limb, out: &mut Vec<FeatureFrame>, scratch: &mut Vec<f64>) {
        if self.total == 0 { return; }
        self.compute_frames(p, limb, true);
        self.evaluate(p, true, scratch);
        if let Some((pt, _)) = self.provisional.take() { self.final_peaks.push_back(pt); }
        self.emit(p, true, out);
        *self = LimbFx::new();
    }
}

pub struct DefaultFeatureExtractor {
    p: FeatureParams,
    limbs: [LimbFx; 4],
    scratch: Vec<f64>,
}

impl DefaultFeatureExtractor {
    pub fn new(p: FeatureParams) -> Self {
        DefaultFeatureExtractor {
            p,
            limbs: [LimbFx::new(), LimbFx::new(), LimbFx::new(), LimbFx::new()],
            scratch: Vec::with_capacity(64),
        }
    }
}

impl FeatureExtractor for DefaultFeatureExtractor {
    fn push(&mut self, input: &[FilteredSample], out: &mut Vec<FeatureFrame>) {
        for fs in input {
            if fs.limb == Limb::None { continue; }
            let p = self.p;
            self.limbs[fs.limb as usize].push(&p, fs, out, &mut self.scratch);
        }
    }
    fn flush(&mut self, out: &mut Vec<FeatureFrame>) {
        let p = self.p;
        for l in Limb::ALL {
            self.limbs[l as usize].flush(&p, l, out, &mut self.scratch);
        }
    }
    fn reset(&mut self) {
        self.limbs = [LimbFx::new(), LimbFx::new(), LimbFx::new(), LimbFx::new()];
    }
}

pub fn make_feature_extractor() -> Box<dyn FeatureExtractor> {
    Box::new(DefaultFeatureExtractor::new(FeatureParams::default()))
}

pub fn make_feature_extractor_with(p: FeatureParams) -> Box<dyn FeatureExtractor> {
    Box::new(DefaultFeatureExtractor::new(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs(t: f64, limb: Limb, lin: [f32; 3]) -> FilteredSample {
        FilteredSample { t_song: t, limb, accel: [0.0, 9.8, 0.0], lin_accel: lin, gyro: [0.0; 3], quat: [1.0, 0.0, 0.0, 0.0] }
    }

    /// Serie con golpes gaussianos (σ = 15 ms) de amplitud `amp` en `hits`.
    fn series(t0: f64, t1: f64, limb: Limb, hits: &[f64], amp: f64) -> Vec<FilteredSample> {
        let n = ((t1 - t0) / 0.005).round() as usize;
        (0..n).map(|i| {
            let t = t0 + i as f64 * 0.005;
            let mut v = 0.2;
            for &h in hits { v += amp * (-(t - h) * (t - h) / (2.0 * 0.015 * 0.015)).exp(); }
            fs(t, limb, [0.0, v as f32, 0.0])
        }).collect()
    }

    fn run(input: &[FilteredSample], chunk: usize) -> Vec<FeatureFrame> {
        let mut fx = make_feature_extractor();
        let mut out = Vec::new();
        for c in input.chunks(chunk) { fx.push(c, &mut out); }
        fx.flush(&mut out);
        out
    }

    #[test]
    fn frames_aligned_and_energy() {
        let input: Vec<_> = (0..200).map(|i| fs(1.003 + i as f64 * 0.005, Limb::LeftHand, [3.0, 4.0, 0.0])).collect();
        let out = run(&input, 7);
        assert!(!out.is_empty());
        for w in out.windows(2) { assert!(w[1].t_song > w[0].t_song); }
        for f in &out {
            let k = (f.t_song / 0.02).round();
            assert!((f.t_song - k * 0.02).abs() < 1e-12);
            assert!((f.energy - 5.0).abs() < 1e-5);
            assert!((f.peak - 5.0).abs() < 1e-5);
            assert!((f.dir[0] - 0.6).abs() < 1e-5 && (f.dir[1] - 0.8).abs() < 1e-5);
            assert!(!f.is_peak);
        }
        // Primera trama: centro 0.96 (|1.003−0.96| = 0.043 ≤ 0.05); la de 0.94 no tiene muestras.
        assert!((out[0].t_song - 0.96).abs() < 1e-9);
        let last_t = input.last().unwrap().t_song;
        assert!(out.last().unwrap().t_song <= last_t + 0.05);
    }

    #[test]
    fn small_dir_is_zero() {
        let input: Vec<_> = (0..50).map(|i| fs(i as f64 * 0.005, Limb::RightHand, [0.3, 0.0, 0.0])).collect();
        for f in run(&input, 50) { assert_eq!(f.dir, [0.0; 3]); }
    }

    #[test]
    fn peaks_at_known_instants() {
        let hits = [1.0, 1.5, 2.012, 2.6];
        let input = series(0.0, 4.0, Limb::LeftFoot, &hits, 20.0);
        for chunk in [1, 13, 1000] {
            let out = run(&input, chunk);
            let peaks: Vec<f64> = out.iter().filter(|f| f.is_peak).map(|f| f.peak_t_song).collect();
            assert_eq!(peaks.len(), hits.len(), "chunk {} picos {:?}", chunk, peaks);
            for (p, h) in peaks.iter().zip(hits.iter()) {
                assert!((p - h).abs() <= 0.005 + 1e-9, "pico {} esperado {}", p, h);
            }
            for f in out.iter().filter(|f| f.is_peak) {
                assert!((f.t_song - f.peak_t_song).abs() <= 0.01 + 1e-9);
            }
        }
    }

    #[test]
    fn min_separation_keeps_larger() {
        // Dos golpes a 100 ms: gana el mayor.
        let mut input = series(0.0, 3.0, Limb::LeftHand, &[1.0], 12.0);
        let big = series(0.0, 3.0, Limb::LeftHand, &[1.1], 25.0);
        for (a, b) in input.iter_mut().zip(big.iter()) { a.lin_accel[1] += b.lin_accel[1] - 0.2; }
        let out = run(&input, 5);
        let peaks: Vec<f64> = out.iter().filter(|f| f.is_peak).map(|f| f.peak_t_song).collect();
        assert_eq!(peaks.len(), 1, "{:?}", peaks);
        assert!((peaks[0] - 1.1).abs() <= 0.005 + 1e-9);
    }

    #[test]
    fn below_threshold_is_not_peak() {
        let input = series(0.0, 3.0, Limb::LeftHand, &[1.0], 7.0);
        assert!(run(&input, 9).iter().all(|f| !f.is_peak));
    }

    #[test]
    fn relative_threshold_with_loud_background() {
        // Fondo de 7 m/s² de energía: un pico de 9.5 (< 1.5·7) no cuenta aunque supere PEAK_MIN.
        let mut input = series(0.0, 3.0, Limb::LeftHand, &[2.0], 2.5);
        for (i, s) in input.iter_mut().enumerate() {
            let osc = if i % 2 == 0 { 7.0 } else { 6.9 };
            s.lin_accel[1] += osc as f32;
        }
        assert!(run(&input, 3).iter().all(|f| !f.is_peak));
    }

    #[test]
    fn streaming_equals_offline() {
        let hits = [0.5, 0.8, 1.3, 1.33, 2.2];
        let mut input = series(0.0, 3.0, Limb::RightHand, &hits, 15.0);
        input.extend(series(0.0, 3.0, Limb::LeftFoot, &hits[1..], 11.0));
        input.sort_by(|a, b| a.t_song.partial_cmp(&b.t_song).unwrap());
        let a = run(&input, input.len());
        let b = run(&input, 1);
        let key = |v: &Vec<FeatureFrame>, l: Limb| v.iter().filter(|f| f.limb == l).cloned().collect::<Vec<_>>();
        for l in [Limb::RightHand, Limb::LeftFoot] { assert_eq!(key(&a, l), key(&b, l)); }
    }

    #[test]
    fn gap_skips_empty_frames() {
        let mut input: Vec<_> = (0..40).map(|i| fs(i as f64 * 0.005, Limb::LeftHand, [1.0, 0.0, 0.0])).collect();
        input.extend((0..40).map(|i| fs(2.0 + i as f64 * 0.005, Limb::LeftHand, [1.0, 0.0, 0.0])));
        let out = run(&input, 4);
        assert!(out.iter().all(|f| f.t_song < 0.25 || f.t_song > 1.94));
        assert!(out.iter().any(|f| f.t_song > 2.0));
    }

    #[test]
    fn peak_frame_is_floor_plus_half() {
        assert_eq!(nearest_frame_index(1.0, 0.02), 50);
        assert_eq!(nearest_frame_index(1.009, 0.02), 50);
        assert_eq!(nearest_frame_index(1.011, 0.02), 51);
        assert_eq!(nearest_frame_index(1.01, 0.02), (1.01f64 / 0.02 + 0.5).floor() as i64);
    }
}
