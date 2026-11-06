//! B7 · ReplaySource: reproduce una sesión en tiempo real. AGENTE A.
//!
//! Desde `start()` (instante `t0` del reloj), cada muestra se emite cuando ha pasado
//! `(t − t_primera) / speed` y lleva `t_ns = t0 + (t − t_primera) / speed`: el instante equivalente
//! en el reloj del host, no el instante (con retraso) en que se hizo `poll()`.
//! El reloj es inyectable (`with_clock`) para tests deterministas; por defecto `clock::now_ns`.
use crate::clock;
use crate::session::load_session;
use crate::types::{DeviceInfo, Limb, MotionSample, MotionSource, Session};
use std::path::Path;

pub type ClockFn = Box<dyn Fn() -> i64 + Send>;

pub struct ReplaySource {
    session: Session,
    samples: Vec<MotionSample>, // ordenadas por t_ns (estable)
    speed: f64,
    clock: ClockFn,
    t0: i64,
    first_t: i64,
    next: usize,
    running: bool,
    limb_override: Vec<(u32, Limb)>,
}

impl ReplaySource {
    /// `speed` = 1.0 tiempo real; valores no positivos o no finitos se tratan como 1.0.
    pub fn new(session: Session, speed: f64) -> ReplaySource {
        let mut samples = session.samples.clone();
        samples.sort_by_key(|s| s.t_ns);
        let first_t = samples.first().map(|s| s.t_ns).unwrap_or(0);
        ReplaySource {
            session,
            samples,
            speed: if speed.is_finite() && speed > 0.0 { speed } else { 1.0 },
            clock: Box::new(clock::now_ns),
            t0: 0,
            first_t,
            next: 0,
            running: false,
            limb_override: Vec::new(),
        }
    }

    pub fn from_path(path: &Path, speed: f64) -> Result<ReplaySource, String> {
        Ok(ReplaySource::new(load_session(path)?, speed))
    }

    /// Sustituye el reloj (nanosegundos monotónicos).
    pub fn with_clock(mut self, clock: ClockFn) -> ReplaySource { self.clock = clock; self }

    pub fn session(&self) -> &Session { &self.session }
    pub fn speed(&self) -> f64 { self.speed }
    /// true cuando ya se emitieron todas las muestras.
    pub fn finished(&self) -> bool { self.next >= self.samples.len() }
    /// Duración de la sesión en tiempo de reproducción (ns).
    pub fn duration_ns(&self) -> i64 {
        match (self.samples.first(), self.samples.last()) {
            (Some(a), Some(b)) => ((b.t_ns - a.t_ns) as f64 / self.speed).round() as i64,
            _ => 0,
        }
    }

    fn limb_for(&self, s: &MotionSample) -> u8 {
        self.limb_override.iter().find(|(id, _)| *id == s.source_id).map(|(_, l)| *l as u8).unwrap_or(s.limb)
    }
}

impl MotionSource for ReplaySource {
    fn start(&mut self) -> Result<(), String> {
        self.t0 = (self.clock)();
        self.next = 0;
        self.running = true;
        Ok(())
    }

    fn stop(&mut self) { self.running = false; }

    fn poll(&mut self, out: &mut Vec<MotionSample>) -> usize {
        if !self.running { return 0; }
        let elapsed = ((self.clock)() - self.t0) as f64 * self.speed;
        let mut n = 0;
        while self.next < self.samples.len() {
            let s = self.samples[self.next];
            let rel = (s.t_ns - self.first_t) as f64;
            if rel > elapsed { break; }
            let mut o = s;
            o.t_ns = self.t0 + (rel / self.speed).round() as i64;
            o.limb = self.limb_for(&s);
            out.push(o);
            self.next += 1;
            n += 1;
        }
        n
    }

    fn devices(&self) -> Vec<DeviceInfo> {
        let mut ids: Vec<u32> = self.session.devices.iter().map(|d| d.source_id).collect();
        for s in &self.samples { if !ids.contains(&s.source_id) { ids.push(s.source_id); } }
        ids.iter().map(|&id| {
            let d = self.session.devices.iter().find(|d| d.source_id == id);
            let base_limb = d.map(|d| d.limb).unwrap_or_else(|| {
                self.samples.iter().find(|s| s.source_id == id).map(|s| s.limb()).unwrap_or(Limb::None)
            });
            let limb = self.limb_override.iter().find(|(i, _)| *i == id).map(|(_, l)| *l).unwrap_or(base_limb);
            DeviceInfo {
                source_id: id,
                name: d.map(|d| d.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("replay {}", id)),
                kind: "replay".into(),
                limb,
                connected: self.running,
                battery: -1.0,
                sync_rtt_ms: -1.0,
            }
        }).collect()
    }

    fn assign_limb(&mut self, source_id: u32, limb: Limb) {
        if let Some(e) = self.limb_override.iter_mut().find(|(id, _)| *id == source_id) { e.1 = limb; }
        else { self.limb_override.push((source_id, limb)); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SessionDevice, SAMPLE_HAS_ACCEL};
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;

    fn session() -> Session {
        let mut s = Session { time_base: "song".into(), ..Default::default() };
        s.devices.push(SessionDevice { source_id: 1, name: "A".into(), kind: "phone".into(), limb: Limb::LeftHand });
        for k in 0..10 {
            s.samples.push(MotionSample { t_ns: 2_000_000_000 + k * 10_000_000, source_id: 1, limb: 0,
                flags: SAMPLE_HAS_ACCEL, _reserved: 0, accel: [k as f32, 0.0, 0.0], gyro: [0.0; 3] });
        }
        s
    }

    fn fake_clock() -> (Arc<AtomicI64>, ClockFn) {
        let c = Arc::new(AtomicI64::new(1_000_000));
        let c2 = c.clone();
        (c, Box::new(move || c2.load(Ordering::SeqCst)))
    }

    #[test]
    fn emits_in_real_time_with_equivalent_t() {
        let (c, f) = fake_clock();
        let mut r = ReplaySource::new(session(), 1.0).with_clock(f);
        let mut out = Vec::new();
        assert_eq!(r.poll(&mut out), 0); // sin start
        r.start().unwrap();
        assert_eq!(r.poll(&mut out), 1); // la primera sale en t0
        assert_eq!(out[0].t_ns, 1_000_000);
        c.fetch_add(25_000_000, Ordering::SeqCst); // +25 ms -> muestras de 10 y 20 ms
        assert_eq!(r.poll(&mut out), 2);
        assert_eq!(out[1].t_ns, 11_000_000);
        assert_eq!(out[2].t_ns, 21_000_000);
        c.fetch_add(1_000_000_000, Ordering::SeqCst);
        assert_eq!(r.poll(&mut out), 7);
        assert!(r.finished());
        assert_eq!(out.last().unwrap().accel[0], 9.0);
        assert_eq!(out.last().unwrap().t_ns, 1_000_000 + 90_000_000);
    }

    #[test]
    fn speed_and_limb_override() {
        let (c, f) = fake_clock();
        let mut r = ReplaySource::new(session(), 2.0).with_clock(f);
        r.assign_limb(1, Limb::RightFoot);
        r.start().unwrap();
        c.fetch_add(20_000_000, Ordering::SeqCst); // 20 ms a 2x = 40 ms de sesión
        let mut out = Vec::new();
        assert_eq!(r.poll(&mut out), 5);
        assert_eq!(out[4].t_ns, 1_000_000 + 20_000_000);
        assert!(out.iter().all(|s| s.limb == Limb::RightFoot as u8));
        assert_eq!(r.devices()[0].limb, Limb::RightFoot);
        assert_eq!(r.devices()[0].kind, "replay");
        assert_eq!(r.duration_ns(), 45_000_000);
        // start() de nuevo reinicia.
        r.start().unwrap();
        out.clear();
        assert_eq!(r.poll(&mut out), 1);
    }
}
