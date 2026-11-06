//! B5 · Juez (contracts/judge_algorithm_v1.md). AGENTE C.
//!
//! Un resultado por paso y extremidad. Determinista: el resultado solo depende de las tramas
//! recibidas y de la secuencia de `update`.
//!
//! Contrato v1.1 + proto/ALGORITHM.md §1 (normativo):
//! - Ventana de un move: tramas k ∈ [ceil(t/hop − 1e-6), floor((t+dur)/hop + 1e-6)]; hay referencia si
//!   existe alguna trama de referencia de esa extremidad en ella.
//! - Desfases L = m·hop, |m| ≤ floor(LAG_SEARCH_SCALE·gw/hop + 1e-6); orden 0, −1, +1, …, solo sustituye
//!   un score estrictamente mayor. Pares (ref k, jugador k+m) solo donde existen ambas tramas; sin
//!   pares → score 0. Puerta de energía con las medias de los pares de L*. La nota usa pw/gw.
//! - Accent: pico con |err| ≤ gw + 1e-9, el más cercano (empate → el anterior).
//! - Emisión: t + dur + LAG_SEARCH_SCALE·gw + EMIT_MARGIN_S; en cada `update` se emiten, ordenados por
//!   (paso, extremidad), todos los vencidos. EXTENSIÓN de tiempo real (permitida por el contrato): si
//!   esa extremidad ya ha enviado tramas pero aún no han llegado las que cubren la ventana del paso
//!   (salen con hasta ~0.25 s de retraso por la detección de picos), se espera hasta `max_wait_s` más.
//!   Con todas las tramas ya empujadas (uso offline) coincide con el contrato.
use crate::features::EPS_T;
use crate::types::{Choreo, FeatureFrame, Grade, Judge, JudgeConfig, Limb, StepKind, StepResult};

/// Parámetros de B5 (valores por defecto del contrato v1). Las ventanas van en JudgeConfig.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JudgeParams {
    pub w_ncc: f64,
    pub w_dir: f64,
    pub energy_gate: f64,
    pub noref_energy_full: f64,
    pub perfect_sim: f64,
    pub good_sim: f64,
    pub emit_margin_s: f64,
    pub frame_hop_s: f64,
    pub var_eps: f64,
    /// v1.1: la búsqueda de desfase de los move cubre ±lag_search_scale·gw (1.0 = v1).
    pub lag_search_scale: f64,
    /// Espera extra máxima a las tramas del jugador (extensión de tiempo real; 0 = contrato puro).
    pub max_wait_s: f64,
}

impl Default for JudgeParams {
    fn default() -> Self {
        JudgeParams {
            w_ncc: 0.6,
            w_dir: 0.4,
            energy_gate: 0.25,
            noref_energy_full: 6.0,
            perfect_sim: 0.75,
            good_sim: 0.5,
            emit_margin_s: 0.05,
            frame_hop_s: 0.02,
            var_eps: 1e-6,
            lag_search_scale: 2.0,
            max_wait_s: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Lite {
    energy: f64,
    dir: [f64; 3],
    nonzero: bool,
}

#[derive(Clone, Debug, Default)]
struct LimbFrames {
    ks: Vec<i64>,
    fr: Vec<Lite>,
}

impl LimbFrames {
    fn insert(&mut self, k: i64, l: Lite) {
        match self.ks.last() {
            Some(&last) if k <= last => match self.ks.binary_search(&k) {
                Ok(i) => self.fr[i] = l,
                Err(i) => { self.ks.insert(i, k); self.fr.insert(i, l); }
            },
            _ => { self.ks.push(k); self.fr.push(l); }
        }
    }
    fn get(&self, k: i64) -> Option<&Lite> { self.ks.binary_search(&k).ok().map(|i| &self.fr[i]) }
    /// Rango de índices con k en [k0, k1].
    fn range_k(&self, k0: i64, k1: i64) -> std::ops::Range<usize> {
        let lo = self.ks.partition_point(|&k| k < k0);
        let hi = self.ks.partition_point(|&k| k <= k1);
        lo..hi.max(lo)
    }
    fn clear(&mut self) { self.ks.clear(); self.fr.clear(); }
}

#[derive(Clone, Copy, Debug)]
struct Item {
    step: usize,
    limb: Limb,
    t: f64,
    dur: f64,
    kind: StepKind,
    pw: f64,
    gw: f64,
    due: f64,
    needed: f64,
}

pub struct DefaultJudge {
    p: JudgeParams,
    cfg: JudgeConfig,
    items: Vec<Item>,
    emitted: Vec<bool>,
    first: usize,
    reference: [LimbFrames; 4],
    player: [LimbFrames; 4],
    peaks: [Vec<f64>; 4],
    seen: [bool; 4],
    watermark: [f64; 4],
}

fn lite(f: &FeatureFrame) -> Lite {
    let d = [f.dir[0] as f64, f.dir[1] as f64, f.dir[2] as f64];
    Lite { energy: f.energy as f64, dir: d, nonzero: d.iter().any(|v| *v != 0.0) }
}

fn frame_k(t: f64, hop: f64) -> i64 { (t / hop).round() as i64 }

impl DefaultJudge {
    pub fn new(p: JudgeParams) -> Self {
        DefaultJudge {
            p,
            cfg: JudgeConfig::default(),
            items: Vec::new(),
            emitted: Vec::new(),
            first: 0,
            reference: Default::default(),
            player: Default::default(),
            peaks: Default::default(),
            seen: [false; 4],
            watermark: [f64::NEG_INFINITY; 4],
        }
    }

    fn windows(&self, limb: Limb) -> (f64, f64) {
        let scale = if limb.is_foot() { self.cfg.foot_window_scale as f64 } else { 1.0 };
        (self.cfg.perfect_window_ms as f64 / 1000.0 * scale, self.cfg.good_window_ms as f64 / 1000.0 * scale)
    }

    fn judge_accent(&self, it: &Item) -> (Grade, f64, f64) {
        let peaks = &self.peaks[it.limb as usize];
        let lo = peaks.partition_point(|&p| p < it.t - it.gw - EPS_T);
        let mut best: Option<f64> = None;
        for &pt in &peaks[lo..] {
            let e = pt - it.t;
            if e > it.gw + EPS_T { break; }
            if e.abs() > it.gw + EPS_T { continue; }
            match best { Some(b) if b.abs() <= e.abs() => {}, _ => best = Some(e) }
        }
        match best {
            None => (Grade::Miss, 0.0, 0.0),
            Some(err) => {
                if err.abs() <= it.pw + EPS_T {
                    (Grade::Perfect, (1.0 - err.abs() / it.gw).max(0.0), err * 1000.0)
                } else {
                    (Grade::Good, (1.0 - err.abs() / it.gw).max(0.0), err * 1000.0)
                }
            }
        }
    }

    /// Índices de trama de la ventana [t, t+dur]: [ceil(t/hop − 1e-6), floor((t+dur)/hop + 1e-6)].
    fn window_k(&self, it: &Item) -> (i64, i64) {
        let hop = self.p.frame_hop_s;
        ((it.t / hop - 1e-6).ceil() as i64, ((it.t + it.dur) / hop + 1e-6).floor() as i64)
    }

    fn judge_move(&self, it: &Item, er: &mut Vec<f64>, ep: &mut Vec<f64>) -> (Grade, f64, f64) {
        let p = &self.p;
        let li = it.limb as usize;
        let rf = &self.reference[li];
        let pf = &self.player[li];
        let (k0, k1) = self.window_k(it);
        let rr = rf.range_k(k0, k1);
        if rr.is_empty() {
            let r = pf.range_k(k0, k1);
            let n = r.len();
            let mean = if n == 0 { 0.0 } else { pf.fr[r].iter().map(|f| f.energy).sum::<f64>() / n as f64 };
            let sim = (mean / p.noref_energy_full).clamp(0.0, 1.0);
            let g = if sim >= p.good_sim { Grade::Good } else { Grade::Miss };
            return (g, sim, 0.0);
        }
        let hop = p.frame_hop_s;
        let mmax = (p.lag_search_scale * it.gw / hop + 1e-6).floor() as i64;
        let rks = &rf.ks[rr.clone()];
        let rfr = &rf.fr[rr];

        // Orden de búsqueda: 0, −1, +1, −2, +2… y solo se sustituye con score estrictamente mayor.
        let mut best: Option<(f64, i64, f64, f64)> = None; // (score, m, mean_r, mean_p) de los pares
        for step in 0..=(2 * mmax) {
            let m = if step == 0 { 0 } else if step % 2 == 1 { -((step + 1) / 2) } else { step / 2 };
            er.clear();
            ep.clear();
            let mut dsum = 0.0;
            let mut dn = 0usize;
            for (idx, &k) in rks.iter().enumerate() {
                if let Some(pfr) = pf.get(k + m) {
                    let rfrm = &rfr[idx];
                    er.push(rfrm.energy);
                    ep.push(pfr.energy);
                    if rfrm.nonzero && pfr.nonzero {
                        dsum += rfrm.dir[0] * pfr.dir[0] + rfrm.dir[1] * pfr.dir[1] + rfrm.dir[2] * pfr.dir[2];
                        dn += 1;
                    }
                }
            }
            let (score, mean_r, mean_p) = if er.is_empty() { (0.0, 0.0, 0.0) } else {
                let n = er.len() as f64;
                let mean_r = er.iter().sum::<f64>() / n;
                let mean_p = ep.iter().sum::<f64>() / n;
                let ncc = ncc(er, mean_r, ep, mean_p, p.var_eps);
                let score = if dn == 0 { ncc } else {
                    let dcos = (dsum / dn as f64).clamp(0.0, 1.0);
                    p.w_ncc * ncc + p.w_dir * dcos
                };
                (score, mean_r, mean_p)
            };
            match best { Some((s, ..)) if score <= s => {}, _ => best = Some((score, m, mean_r, mean_p)) }
        }
        let (mut score, m, mean_r, mean_p) = best.unwrap();
        let gate = p.energy_gate * mean_r;
        if gate > 0.0 && mean_p < gate { score *= mean_p / gate; }
        let l = m as f64 * hop;
        let g = if score >= p.perfect_sim && l.abs() <= it.pw + EPS_T {
            Grade::Perfect
        } else if score >= p.good_sim && l.abs() <= it.gw + EPS_T {
            Grade::Good
        } else {
            Grade::Miss
        };
        (g, score, l * 1000.0)
    }
}

fn ncc(a: &[f64], ma: f64, b: &[f64], mb: f64, eps: f64) -> f64 {
    let n = a.len() as f64;
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b.iter()) {
        let (dx, dy) = (x - ma, y - mb);
        sab += dx * dy;
        saa += dx * dx;
        sbb += dy * dy;
    }
    if saa / n < eps || sbb / n < eps { return 0.0; }
    (sab / (saa * sbb).sqrt()).clamp(0.0, 1.0)
}

impl Judge for DefaultJudge {
    fn load(&mut self, choreo: &Choreo, reference_frames: &[FeatureFrame], cfg: &JudgeConfig) {
        self.cfg = *cfg;
        let hop = self.p.frame_hop_s;
        for r in self.reference.iter_mut() { r.clear(); }
        for f in reference_frames {
            if f.limb == Limb::None { continue; }
            self.reference[f.limb as usize].insert(frame_k(f.t_song, hop), lite(f));
        }
        self.items.clear();
        for (si, st) in choreo.steps.iter().enumerate() {
            let mut limbs: Vec<Limb> = st.limbs.iter().copied().filter(|l| *l != Limb::None).collect();
            limbs.sort();
            limbs.dedup();
            for limb in limbs {
                let (pw, gw) = self.windows(limb);
                let lag = self.p.lag_search_scale * gw;
                let needed = match st.kind { StepKind::Accent => st.t + gw, StepKind::Move => st.t + st.dur + lag };
                self.items.push(Item {
                    step: si, limb, t: st.t, dur: st.dur, kind: st.kind, pw, gw,
                    due: st.t + st.dur + lag + self.p.emit_margin_s,
                    needed,
                });
            }
        }
        self.reset();
    }

    fn push(&mut self, player_frames: &[FeatureFrame]) {
        let hop = self.p.frame_hop_s;
        for f in player_frames {
            if f.limb == Limb::None { continue; }
            let li = f.limb as usize;
            self.player[li].insert(frame_k(f.t_song, hop), lite(f));
            if f.is_peak {
                let v = &mut self.peaks[li];
                match v.last() {
                    Some(&last) if f.peak_t_song < last => {
                        let i = v.partition_point(|&x| x < f.peak_t_song);
                        v.insert(i, f.peak_t_song);
                    }
                    _ => v.push(f.peak_t_song),
                }
            }
            self.seen[li] = true;
            if f.t_song > self.watermark[li] { self.watermark[li] = f.t_song; }
        }
    }

    fn update(&mut self, song_time_s: f64, out: &mut Vec<StepResult>) {
        let mut er = Vec::new();
        let mut ep = Vec::new();
        let half_hop = self.p.frame_hop_s / 2.0;
        let mut i = self.first;
        while i < self.items.len() {
            let it = self.items[i];
            if it.t > song_time_s { break; } // due > t: ningún paso posterior puede estar listo
            if !self.emitted[i] && song_time_s >= it.due {
                let li = it.limb as usize;
                let ready = !self.seen[li]
                    || self.watermark[li] >= it.needed - half_hop
                    || song_time_s >= it.due + self.p.max_wait_s;
                if ready {
                    let (g, sim, terr) = match it.kind {
                        StepKind::Accent => self.judge_accent(&it),
                        StepKind::Move => self.judge_move(&it, &mut er, &mut ep),
                    };
                    out.push(StepResult {
                        step_index: it.step as i32,
                        limb: it.limb as u8,
                        grade: g as u8,
                        _reserved: 0,
                        similarity: sim as f32,
                        timing_error_ms: terr as f32,
                    });
                    self.emitted[i] = true;
                }
            }
            i += 1;
        }
        while self.first < self.items.len() && self.emitted[self.first] { self.first += 1; }
    }

    fn reset(&mut self) {
        self.emitted = vec![false; self.items.len()];
        self.first = 0;
        for p in self.player.iter_mut() { p.clear(); }
        for p in self.peaks.iter_mut() { p.clear(); }
        self.seen = [false; 4];
        self.watermark = [f64::NEG_INFINITY; 4];
    }
}

pub fn make_judge() -> Box<dyn Judge> { Box::new(DefaultJudge::new(JudgeParams::default())) }

pub fn make_judge_with(p: JudgeParams) -> Box<dyn Judge> { Box::new(DefaultJudge::new(p)) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Step;

    fn frame(t: f64, limb: Limb, e: f32, dir: [f32; 3]) -> FeatureFrame {
        FeatureFrame { t_song: t, limb, energy: e, peak: e, is_peak: false, peak_t_song: t, dir, quat: [1.0, 0.0, 0.0, 0.0] }
    }
    fn peak_frame(tp: f64, limb: Limb) -> FeatureFrame {
        let mut f = frame((tp / 0.02).round() * 0.02, limb, 10.0, [0.0, 1.0, 0.0]);
        f.is_peak = true;
        f.peak_t_song = tp;
        f
    }
    fn choreo(steps: Vec<Step>) -> Choreo { Choreo { steps, ..Default::default() } }
    fn step(t: f64, dur: f64, limbs: &[Limb], kind: StepKind) -> Step {
        Step { t, dur, limbs: limbs.to_vec(), icon: String::new(), kind }
    }

    fn run(c: &Choreo, r: &[FeatureFrame], p: &[FeatureFrame]) -> Vec<StepResult> {
        let mut j = make_judge();
        j.load(c, r, &JudgeConfig::default());
        j.push(p);
        let mut out = Vec::new();
        j.update(1e9, &mut out);
        out
    }

    #[test]
    fn accent_grades() {
        let c = choreo(vec![step(2.0, 0.2, &[Limb::RightHand], StepKind::Accent)]);
        let cases = [(0.0, Grade::Perfect), (0.04, Grade::Perfect), (-0.075, Grade::Good), (0.15, Grade::Miss)];
        for (off, g) in cases {
            let out = run(&c, &[], &[peak_frame(2.0 + off, Limb::RightHand)]);
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].grade, g as u8, "desfase {}", off);
            if g != Grade::Miss { assert!((out[0].timing_error_ms as f64 - off * 1000.0).abs() < 1e-3); }
        }
        let out = run(&c, &[], &[]);
        assert_eq!(out[0].grade, Grade::Miss as u8);
    }

    #[test]
    fn accent_feet_window_scaled() {
        let c = choreo(vec![step(2.0, 0.2, &[Limb::LeftFoot, Limb::LeftHand], StepKind::Accent)]);
        let out = run(&c, &[], &[peak_frame(2.07, Limb::LeftFoot), peak_frame(2.07, Limb::LeftHand)]);
        // Orden: extremidad left_hand (0) antes que left_foot (2).
        assert_eq!(out[0].limb, Limb::LeftHand as u8);
        assert_eq!(out[0].grade, Grade::Good as u8);
        assert_eq!(out[1].limb, Limb::LeftFoot as u8);
        assert_eq!(out[1].grade, Grade::Perfect as u8); // 70 ms ≤ 75 ms
        let out = run(&c, &[], &[peak_frame(2.13, Limb::LeftFoot), peak_frame(2.13, Limb::LeftHand)]);
        assert_eq!(out[0].grade, Grade::Miss as u8);
        assert_eq!(out[1].grade, Grade::Good as u8); // 130 ms ≤ 150 ms
    }

    fn wave(t0: f64, t1: f64, limb: Limb, shift: f64, scale: f32) -> Vec<FeatureFrame> {
        let k0 = (t0 / 0.02).round() as i64;
        let k1 = (t1 / 0.02).round() as i64;
        (k0..=k1).map(|k| {
            let t = k as f64 * 0.02;
            let x = t - shift;
            let e = (4.0 + 3.0 * (x * 7.0).sin()) as f32 * scale;
            let d = [(x * 3.0).cos() as f32, (x * 3.0).sin() as f32, 0.0];
            frame(t, limb, e, d)
        }).collect()
    }

    #[test]
    fn move_with_reference() {
        let c = choreo(vec![step(2.0, 1.0, &[Limb::LeftHand], StepKind::Move)]);
        let r = wave(0.0, 5.0, Limb::LeftHand, 0.0, 1.0);
        let same = run(&c, &r, &r);
        assert_eq!(same[0].grade, Grade::Perfect as u8);
        assert!(same[0].similarity > 0.99 && same[0].timing_error_ms == 0.0);
        let late = run(&c, &r, &wave(0.0, 5.0, Limb::LeftHand, 0.08, 1.0));
        assert_eq!(late[0].grade, Grade::Good as u8, "{:?}", late);
        assert!((late[0].timing_error_ms - 80.0).abs() < 1e-3);
        let still: Vec<_> = r.iter().map(|f| frame(f.t_song, f.limb, 0.05, [0.0; 3])).collect();
        assert_eq!(run(&c, &r, &still)[0].grade, Grade::Miss as u8);
        assert_eq!(run(&c, &r, &[])[0].grade, Grade::Miss as u8);
        // Misma forma con poca energía: la puerta de energía la penaliza.
        let weak = run(&c, &r, &wave(0.0, 5.0, Limb::LeftHand, 0.0, 0.1));
        assert!(weak[0].similarity < 0.5, "{:?}", weak);
    }

    #[test]
    fn move_without_reference_never_perfect() {
        let c = choreo(vec![step(2.0, 1.0, &[Limb::RightFoot], StepKind::Move)]);
        let strong: Vec<_> = (0..300).map(|k| frame(k as f64 * 0.02, Limb::RightFoot, 20.0, [1.0, 0.0, 0.0])).collect();
        let out = run(&c, &[], &strong);
        assert_eq!(out[0].grade, Grade::Good as u8);
        assert_eq!(out[0].similarity, 1.0);
        assert_eq!(out[0].timing_error_ms, 0.0);
        let mid: Vec<_> = strong.iter().map(|f| frame(f.t_song, f.limb, 3.6, f.dir)).collect();
        let out = run(&c, &[], &mid);
        assert_eq!(out[0].grade, Grade::Good as u8);
        assert!((out[0].similarity - 0.6).abs() < 1e-6);
        let low: Vec<_> = strong.iter().map(|f| frame(f.t_song, f.limb, 2.0, f.dir)).collect();
        assert_eq!(run(&c, &[], &low)[0].grade, Grade::Miss as u8);
    }

    #[test]
    fn emission_time_and_order() {
        let c = choreo(vec![
            step(1.0, 0.5, &[Limb::LeftFoot, Limb::RightHand], StepKind::Accent), // mano 1.75, pie 1.85
            step(1.2, 0.4, &[Limb::LeftHand], StepKind::Accent),                  // mano 1.85
        ]);
        let mut j = make_judge();
        j.load(&c, &[], &JudgeConfig::default());
        let mut out = Vec::new();
        // Sin tramas del jugador: se emite en el instante del contrato.
        j.update(1.7499, &mut out);
        assert!(out.is_empty());
        j.update(1.7501, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].step_index, out[0].limb), (0, Limb::RightHand as u8));
        j.update(1.8499, &mut out);
        assert_eq!(out.len(), 1);
        j.update(1.8501, &mut out);
        assert_eq!(out.len(), 3);
        assert_eq!((out[1].step_index, out[1].limb), (0, Limb::LeftFoot as u8));
        assert_eq!((out[2].step_index, out[2].limb), (1, Limb::LeftHand as u8));
        j.update(5.0, &mut out);
        assert_eq!(out.len(), 3);
        j.reset();
        out.clear();
        j.update(100.0, &mut out);
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn waits_for_late_frames_when_limb_active() {
        let c = choreo(vec![step(1.0, 0.1, &[Limb::RightHand], StepKind::Accent)]);
        let mut j = make_judge();
        j.load(&c, &[], &JudgeConfig::default());
        j.push(&[frame(0.9, Limb::RightHand, 0.1, [0.0; 3])]);
        let mut out = Vec::new();
        j.update(1.25, &mut out);
        assert!(out.is_empty(), "debe esperar a las tramas que cubren la ventana");
        j.push(&[peak_frame(1.01, Limb::RightHand), frame(1.12, Limb::RightHand, 0.1, [0.0; 3])]);
        j.update(1.36, &mut out);
        assert_eq!(out[0].grade, Grade::Perfect as u8);
        // Sin tramas nuevas: como mucho max_wait_s de espera extra.
        let c = choreo(vec![step(2.0, 0.1, &[Limb::RightHand], StepKind::Accent)]);
        j.load(&c, &[], &JudgeConfig::default());
        j.push(&[frame(1.0, Limb::RightHand, 0.1, [0.0; 3])]);
        out.clear();
        j.update(2.83, &mut out);
        assert!(out.is_empty());
        j.update(2.86, &mut out);
        assert_eq!(out.len(), 1);
    }
}
