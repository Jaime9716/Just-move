//! Simulador de móvil-mando (para `phone_sim`, tests de integración y pruebas sin móvil real).
//! Se comporta como la app Android del contrato: HELLO cada 500 ms hasta WELCOME, responde SYNC_REQ
//! al instante con su propio reloj (desplazado y con deriva), envía SAMPLES a 200 Hz en paquetes
//! cada ~10 ms, STATUS cada segundo y vuelve a emparejarse si el PC calla 3 s.

use super::protocol::{self, Message, Packet, PhoneSample};
use crate::clock::now_ns;
use crate::types::{SAMPLE_HAS_ACCEL, SAMPLE_HAS_GYRO};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SimConfig {
    pub device_id: u32,
    pub pairing_code: u32,
    pub name: String,
    pub rate_hz: u32,
    pub batch_ms: u32,
    /// Desfase del reloj del móvil respecto al del PC (ns).
    pub clock_offset_ns: i64,
    pub drift_ppm: f64,
    /// Periodo de las sacudidas sintéticas (s).
    pub shake_period_s: f64,
    pub battery: f32,
    /// Si es true, accel[2] lleva el índice de la muestra (para que los tests casen envío/recepción).
    pub tag_index: bool,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            device_id: 0x51A0_0001, pairing_code: 0, name: "phone_sim".into(), rate_hz: 200, batch_ms: 10,
            clock_offset_ns: 123_456_789_000, drift_ppm: 0.0, shake_period_s: 1.0, battery: 0.8, tag_index: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SimStats {
    pub samples_sent: u64,
    pub packets_sent: u64,
    pub sync_answered: u64,
    pub hellos_sent: u64,
    pub welcomes: u64,
    pub rejected: u64,
}

pub struct PhoneSim {
    sock: UdpSocket,
    server: SocketAddr,
    pub cfg: SimConfig,
    seq: u32,
    t0_pc: i64,
    pub accepted: bool,
    pub source_id: u32,
    pub limb: u8,
    pub stats: SimStats,
    /// t_ns (reloj del móvil) de cada muestra enviada, en orden.
    pub sent_log: Vec<i64>,
    next_sample_phone: i64,
    batch: Vec<PhoneSample>,
    last_batch_pc: i64,
    last_status_pc: i64,
    last_hello_pc: i64,
    last_rx_pc: i64,
    index: u64,
}

impl PhoneSim {
    pub fn new(server: SocketAddr, cfg: SimConfig) -> std::io::Result<PhoneSim> {
        let bind = if server.ip().is_loopback() { "127.0.0.1:0" } else { "0.0.0.0:0" };
        let sock = UdpSocket::bind(bind)?;
        sock.set_read_timeout(Some(Duration::from_millis(1)))?;
        let _ = sock.set_broadcast(true);
        let t = now_ns();
        let mut s = PhoneSim {
            sock, server, cfg, seq: 0, t0_pc: t, accepted: false, source_id: 0, limb: 255,
            stats: SimStats::default(), sent_log: Vec::new(), next_sample_phone: 0, batch: Vec::new(),
            last_batch_pc: t, last_status_pc: t, last_hello_pc: i64::MIN / 2, last_rx_pc: t, index: 0,
        };
        s.next_sample_phone = s.phone_now();
        Ok(s)
    }

    /// Reloj del móvil: offset + deriva respecto a `now_ns()`.
    pub fn phone_time(&self, t_pc: i64) -> i64 {
        self.cfg.clock_offset_ns + t_pc + ((t_pc - self.t0_pc) as f64 * self.cfg.drift_ppm * 1e-6).round() as i64
    }
    pub fn phone_now(&self) -> i64 { self.phone_time(now_ns()) }
    /// Inverso exacto (salvo redondeo) de `phone_time`: instante real del PC de una marca del móvil.
    pub fn true_pc_time(&self, t_phone: i64) -> i64 {
        let x = (t_phone - self.cfg.clock_offset_ns - self.t0_pc) as f64;
        self.t0_pc + (x / (1.0 + self.cfg.drift_ppm * 1e-6)).round() as i64
    }

    fn send(&mut self, msg: Message) {
        self.seq = self.seq.wrapping_add(1);
        let b = protocol::encode(&Packet { device_id: self.cfg.device_id, seq: self.seq, msg });
        let _ = self.sock.send_to(&b, self.server);
    }

    fn send_hello(&mut self) {
        let m = Message::Hello { pairing_code: self.cfg.pairing_code, name: self.cfg.name.clone() };
        self.send(m);
        self.stats.hellos_sent += 1;
        self.last_hello_pc = now_ns();
    }

    pub fn send_discover(&mut self) { self.send(Message::Discover); }
    pub fn send_bye(&mut self) { self.send(Message::Bye); self.accepted = false; }

    /// Procesa lo recibido (como mucho ~1 ms de espera). Devuelve el WELCOME si llegó uno.
    fn receive(&mut self) -> Option<(u32, u8, bool)> {
        let mut buf = [0u8; 2048];
        let mut welcome = None;
        loop {
            let n = match self.sock.recv_from(&mut buf) { Ok((n, _from)) => n, Err(_) => break };
            let t_recv_phone = self.phone_now();
            let p = match protocol::decode(&buf[..n]) { Ok(p) => p, Err(_) => continue };
            self.last_rx_pc = now_ns();
            match p.msg {
                Message::SyncReq { t_pc_send } => {
                    let t_send = self.phone_now();
                    self.send(Message::SyncResp { t_pc_send, t_phone_recv: t_recv_phone, t_phone_send: t_send });
                    self.stats.sync_answered += 1;
                }
                Message::Welcome { source_id, limb, accepted } => {
                    self.stats.welcomes += 1;
                    if accepted { self.accepted = true; self.source_id = source_id; self.limb = limb; }
                    else if source_id != 0 || !self.accepted { self.stats.rejected += 1; }
                    welcome = Some((source_id, limb, accepted));
                }
                Message::Bye => { self.accepted = false; }
                _ => {}
            }
            if welcome.is_some() { break; }
        }
        welcome
    }

    /// Envía HELLO cada 500 ms hasta recibir WELCOME. Ok(true) aceptado, Ok(false) código rechazado.
    pub fn pair(&mut self, timeout: Duration) -> Result<bool, String> {
        let end = now_ns() + timeout.as_nanos() as i64;
        while now_ns() < end {
            if now_ns() - self.last_hello_pc >= 500_000_000 { self.send_hello(); }
            if let Some((_, _, acc)) = self.receive() { return Ok(acc); }
        }
        Err(format!("sin respuesta de {} tras {:?}", self.server, timeout))
    }

    fn synth(&self, t_phone: i64) -> PhoneSample {
        let t = (t_phone - self.cfg.clock_offset_ns) as f64 * 1e-9;
        let period = if self.cfg.shake_period_s > 0.0 { self.cfg.shake_period_s } else { 1.0 };
        let phase = t.rem_euclid(period);
        let g = (-((phase - 0.1) / 0.03).powi(2)).exp() as f32;
        let accel_z = if self.cfg.tag_index { self.index as f32 } else { 0.3 };
        PhoneSample {
            t_ns: t_phone,
            flags: SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO,
            accel: [18.0 * g, 9.81 - 4.0 * g, accel_z],
            gyro: [0.0, 1.5 * g, 6.0 * g],
        }
    }

    /// Un paso del bucle (≈ 1 ms): recibe, genera muestras, envía lotes y STATUS, re-empareja.
    pub fn step(&mut self) {
        let _ = self.receive();
        let now = now_ns();
        if self.accepted && now - self.last_rx_pc > 3_000_000_000 { self.accepted = false; }
        if !self.accepted {
            if now - self.last_hello_pc >= 500_000_000 { self.send_hello(); }
            self.batch.clear();
            self.next_sample_phone = self.phone_now();
            return;
        }
        let period = 1_000_000_000i64 / self.cfg.rate_hz.max(1) as i64;
        let pnow = self.phone_now();
        if pnow - self.next_sample_phone > 100_000_000 { self.next_sample_phone = pnow; } // tras una pausa
        while self.next_sample_phone <= pnow {
            let s = self.synth(self.next_sample_phone);
            self.batch.push(s);
            self.sent_log.push(s.t_ns);
            self.index += 1;
            self.next_sample_phone += period;
            if self.batch.len() >= 8 { self.flush_batch(); }
        }
        if !self.batch.is_empty() && now - self.last_batch_pc >= self.cfg.batch_ms as i64 * 1_000_000 { self.flush_batch(); }
        if now - self.last_status_pc >= 1_000_000_000 {
            self.last_status_pc = now;
            let b = self.cfg.battery;
            self.send(Message::Status { battery: b, sensor_rate_hz: self.cfg.rate_hz as u16, dropped: 0 });
        }
    }

    fn flush_batch(&mut self) {
        let v = std::mem::take(&mut self.batch);
        self.stats.samples_sent += v.len() as u64;
        self.stats.packets_sent += 1;
        self.send(Message::Samples(v));
        self.last_batch_pc = now_ns();
    }

    pub fn run_for(&mut self, d: Duration) {
        let end = now_ns() + d.as_nanos() as i64;
        while now_ns() < end { self.step(); }
        if !self.batch.is_empty() && self.accepted { self.flush_batch(); }
    }

    /// Corre hasta que `stop` sea true (o `max` si se da).
    pub fn run_until(&mut self, stop: &AtomicBool, max: Option<Duration>) {
        let end = max.map(|m| now_ns() + m.as_nanos() as i64).unwrap_or(i64::MAX);
        while !stop.load(Ordering::SeqCst) && now_ns() < end { self.step(); }
    }
}
