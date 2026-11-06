//! Sincronización de reloj móvil ↔ PC (NTP simplificado, contracts/phone_protocol.md).
//!
//! Por cada SYNC_RESP:
//!   rtt    = (t_pc_recv − t_pc_send) − (t_phone_send − t_phone_recv)
//!   offset = ((t_phone_recv − t_pc_send) + (t_phone_send − t_pc_recv)) / 2      (móvil − PC)
//! Se guardan las 16 últimas. Ancla = la de menor rtt: offset(t) = o_ancla + pendiente·(t − t_ancla).
//! Deriva (pendiente): con ≥ 8 respuestas. Ajustar la recta solo con la ventana de 16 (4–16 s) da una
//! pendiente demasiado ruidosa con el jitter de Wi-Fi (1 ms / 4 s = 250 ppm, peor que no corregir),
//! así que cada bloque de 8 respuestas consecutivas aporta su punto de menor rtt a un historial
//! (hasta 32 bloques, ~4 min) y la recta se ajusta por mínimos cuadrados sobre ese historial cuando
//! abarca ≥ 15 s. Antes, pendiente 0. Se limita a ±500 ppm (los cuarzos derivan < 100 ppm).

use std::collections::VecDeque;

pub const WINDOW: usize = 16;
pub const MIN_FOR_DRIFT: usize = 8;
pub const MAX_SLOPE: f64 = 500e-6;
/// Error típico máximo de la pendiente para usarla.
pub const MAX_SLOPE_SE: f64 = 30e-6;
/// Respuestas con rtt mayor se descartan (paquete muy retrasado o basura).
pub const MAX_RTT_NS: i64 = 2_000_000_000;
/// Bloques de MIN_FOR_DRIFT respuestas guardados para estimar la deriva.
pub const HISTORY_BLOCKS: usize = 32;
/// Extensión mínima del historial para usar la pendiente.
pub const MIN_DRIFT_SPAN_NS: i64 = 15_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncPoint {
    /// Punto medio del intercambio en el reloj del PC.
    pub t_pc: i64,
    pub rtt: i64,
    pub offset: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Estimate { t_ref: i64, o_ref: i64, slope: f64 }

#[derive(Clone, Debug, Default)]
pub struct ClockSync {
    points: VecDeque<SyncPoint>,
    est: Option<Estimate>,
    block_best: Option<SyncPoint>,
    block_count: usize,
    history: VecDeque<SyncPoint>,
    total: u64,
}

impl ClockSync {
    pub fn new() -> ClockSync { ClockSync::default() }

    pub fn reset(&mut self) { *self = ClockSync::default(); }

    pub fn len(&self) -> usize { self.points.len() }
    pub fn is_empty(&self) -> bool { self.points.is_empty() }
    pub fn has_estimate(&self) -> bool { self.est.is_some() }

    /// Añade una respuesta. Devuelve el punto calculado o None si se descarta (rtt negativo/enorme).
    pub fn add(&mut self, t_pc_send: i64, t_phone_recv: i64, t_phone_send: i64, t_pc_recv: i64) -> Option<SyncPoint> {
        let (a, b, c, d) = (t_pc_send as i128, t_phone_recv as i128, t_phone_send as i128, t_pc_recv as i128);
        let rtt = (d - a) - (c - b);
        let offset = ((b - a) + (c - d)) / 2;
        if rtt < 0 || rtt > MAX_RTT_NS as i128 || c < b { return None; }
        if offset > i64::MAX as i128 / 2 || offset < i64::MIN as i128 / 2 { return None; }
        let p = SyncPoint { t_pc: ((a + d) / 2) as i64, rtt: rtt as i64, offset: offset as i64 };
        if self.points.len() == WINDOW { self.points.pop_front(); }
        self.points.push_back(p);
        self.total += 1;
        if self.block_best.map_or(true, |b| p.rtt < b.rtt) { self.block_best = Some(p); }
        self.block_count += 1;
        if self.block_count >= MIN_FOR_DRIFT {
            if let Some(b) = self.block_best.take() {
                if self.history.len() == HISTORY_BLOCKS { self.history.pop_front(); }
                self.history.push_back(b);
            }
            self.block_count = 0;
        }
        self.recompute();
        Some(p)
    }

    fn recompute(&mut self) {
        let best = match self.points.iter().min_by_key(|p| p.rtt) { Some(b) => *b, None => { self.est = None; return; } };
        let mut slope = 0.0;
        let span = match (self.history.front(), self.history.back()) { (Some(a), Some(b)) => b.t_pc - a.t_pc, _ => 0 };
        if self.points.len() >= MIN_FOR_DRIFT && self.history.len() >= 3 && span >= MIN_DRIFT_SPAN_NS {
            // Mínimos cuadrados ponderados: el error de un punto está acotado por rtt/2; σ ≈ rtt/5.
            // Solo se usa la pendiente si su error típico es < MAX_SLOPE_SE (si no, corregiría peor).
            let r = self.history[0];
            let w: Vec<f64> = self.history.iter().map(|p| { let s = (p.rtt as f64 / 5.0).max(50_000.0); 1.0 / (s * s) }).collect();
            let xs: Vec<f64> = self.history.iter().map(|p| (p.t_pc - r.t_pc) as f64).collect();
            let ys: Vec<f64> = self.history.iter().map(|p| (p.offset - r.offset) as f64).collect();
            let sw: f64 = w.iter().sum();
            let mx = w.iter().zip(&xs).map(|(w, x)| w * x).sum::<f64>() / sw;
            let my = w.iter().zip(&ys).map(|(w, y)| w * y).sum::<f64>() / sw;
            let sxx: f64 = w.iter().zip(&xs).map(|(w, x)| w * (x - mx) * (x - mx)).sum();
            let sxy: f64 = w.iter().zip(xs.iter().zip(&ys)).map(|(w, (x, y))| w * (x - mx) * (y - my)).sum();
            if sxx > 0.0 && (1.0 / sxx).sqrt() < MAX_SLOPE_SE { slope = (sxy / sxx).clamp(-MAX_SLOPE, MAX_SLOPE); }
        }
        self.est = Some(Estimate { t_ref: best.t_pc, o_ref: best.offset, slope });
    }

    /// Offset estimado (móvil − PC, ns) en el instante `t_pc` del PC.
    pub fn offset_at(&self, t_pc: i64) -> Option<i64> {
        let e = self.est?;
        Some(e.o_ref + (e.slope * (t_pc as f64 - e.t_ref as f64)).round() as i64)
    }

    /// Convierte un instante del reloj del móvil al reloj del PC: t_pc = t_phone − offset(t_pc).
    pub fn phone_to_pc(&self, t_phone: i64) -> Option<i64> {
        let e = self.est?;
        let mut t = t_phone.saturating_sub(e.o_ref);
        for _ in 0..2 { t = t_phone.saturating_sub(self.offset_at(t)?); }
        Some(t)
    }

    /// Menor rtt de la ventana (ns).
    pub fn min_rtt_ns(&self) -> Option<i64> { self.points.iter().map(|p| p.rtt).min() }

    /// Deriva estimada (adimensional, p. ej. 50e-6 = 50 ppm).
    pub fn drift(&self) -> f64 { self.est.map(|e| e.slope).unwrap_or(0.0) }

    pub fn points(&self) -> impl Iterator<Item = &SyncPoint> { self.points.iter() }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
        fn unit(&mut self) -> f64 { (self.next() >> 11) as f64 / (1u64 << 53) as f64 }
        /// Retardo de red: base + exponencial (media `mean_ms`) + 5 % de picos grandes.
        fn delay_ns(&mut self, base_ms: f64, mean_ms: f64) -> i64 {
            let mut d = base_ms - mean_ms * (1.0 - self.unit()).ln();
            if self.unit() < 0.05 { d += 40.0 * self.unit(); }
            (d * 1e6) as i64
        }
    }

    /// Reloj del móvil: offset grande + deriva.
    fn phone_clock(t_pc: i64, off: i64, drift: f64) -> i64 { off + t_pc + (t_pc as f64 * drift) as i64 }

    struct RunResult { worst_ms: f64, median_ms: f64, late_worst_ms: f64 }

    /// Simula 120 s de sincronización (SYNC cada 250 ms los 5 primeros s, luego cada 1 s) y mide
    /// el error al convertir marcas del móvil en el segundo siguiente a cada respuesta.
    /// Comprueba la cota teórica: |error| ≤ rtt_min/2 (+ deriva no corregida).
    fn run(drift: f64, seed: u64, mean_ms: f64) -> RunResult {
        let off: i64 = 987_654_321_012_345; // ~11 días
        let mut cs = ClockSync::new();
        let mut rng = Rng(seed);
        let mut t: i64 = 1_000_000_000;
        let (mut worst, mut late_worst) = (0.0f64, 0.0f64);
        let mut errs = Vec::new();
        let mut k = 0;
        while t < 120_000_000_000 {
            let send = t;
            let recv_phone_pc = send + rng.delay_ns(1.0, mean_ms);
            let proc = 200_000 + (rng.unit() * 300_000.0) as i64;
            let send_phone_pc = recv_phone_pc + proc;
            let back = send_phone_pc + rng.delay_ns(1.0, mean_ms);
            cs.add(send, phone_clock(recv_phone_pc, off, drift), phone_clock(send_phone_pc, off, drift), back);
            k += 1;
            if k >= MIN_FOR_DRIFT {
                let half_rtt = cs.min_rtt_ns().unwrap() as f64 / 2.0;
                for j in 0..10 {
                    let tp = back + j * 100_000_000;
                    let est = cs.phone_to_pc(phone_clock(tp, off, drift)).unwrap();
                    let e = (est - tp).abs() as f64;
                    // Sin pendiente aún: la deriva puede sumar hasta 50 ppm · 17 s ≈ 0.85 ms.
                    let margin = if t > 60_000_000_000 { 300_000.0 } else { 1_000_000.0 };
                    assert!(e <= half_rtt + margin, "t={} s error {} ms > cota {} ms", t / 1_000_000_000, e / 1e6, (half_rtt + margin) / 1e6);
                    worst = worst.max(e);
                    if t > 60_000_000_000 { late_worst = late_worst.max(e); }
                    errs.push(e);
                }
            }
            t += if t < 6_000_000_000 { 250_000_000 } else { 1_000_000_000 };
        }
        if drift != 0.0 { assert!((cs.drift() - drift).abs() < 20e-6, "deriva estimada {} ppm", cs.drift() * 1e6); }
        errs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        RunResult { worst_ms: worst / 1e6, median_ms: errs[errs.len() / 2] / 1e6, late_worst_ms: late_worst / 1e6 }
    }

    #[test]
    fn formulas() {
        let mut cs = ClockSync::new();
        // PC envía en 100, móvil (offset +1000) recibe en 1105 (5 de ida), responde en 1107, PC recibe en 112.
        let p = cs.add(100, 1105, 1107, 112).unwrap();
        assert_eq!(p.rtt, 10);
        assert_eq!(p.offset, 1000);
        assert_eq!(cs.phone_to_pc(5000), Some(4000));
        assert_eq!(cs.min_rtt_ns(), Some(10));
        assert!(cs.add(100, 1105, 1107, 101).is_none()); // rtt negativo
        assert!(cs.add(i64::MIN, i64::MAX, i64::MAX, i64::MAX).is_none());
    }

    #[test]
    fn window_and_min_rtt() {
        let mut cs = ClockSync::new();
        for i in 0..40i64 { cs.add(i * 1000, i * 1000 + 50 + 500, i * 1000 + 50 + 500, i * 1000 + 100 + (i % 5) * 10); }
        assert_eq!(cs.len(), WINDOW);
        assert_eq!(cs.min_rtt_ns(), Some(100));
    }

    #[test]
    fn offset_only() {
        for seed in 1..6 {
            let r = run(0.0, seed * 7919, 3.0);
            assert!(r.median_ms < 0.5 && r.worst_ms < 2.0, "mediana {} ms, peor {} ms", r.median_ms, r.worst_ms);
        }
    }

    #[test]
    fn drift_50ppm_with_jitter() {
        for seed in 1..6 {
            for d in [50e-6, -50e-6] {
                let r = run(d, seed * 104729, 3.0);
                assert!(r.median_ms < 0.6 && r.worst_ms < 2.0 && r.late_worst_ms < 1.5,
                    "deriva {} semilla {}: mediana {} peor {} peor tardío {} ms", d, seed, r.median_ms, r.worst_ms, r.late_worst_ms);
            }
        }
    }

    #[test]
    fn drift_heavy_jitter() {
        // Wi-Fi malo: media 10 ms por sentido; la elección de menor rtt mantiene el error bajo rtt/2.
        for seed in 1..4 {
            let r = run(50e-6, seed * 15485863, 10.0);
            assert!(r.median_ms < 2.0 && r.worst_ms < 6.0, "mediana {} peor {} ms", r.median_ms, r.worst_ms);
        }
    }
}
