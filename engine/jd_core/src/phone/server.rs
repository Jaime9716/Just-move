//! PhoneServerSource: servidor UDP de móviles-mando (contracts/phone_protocol.md).
//!
//! Un hilo propio ("jd-phone-server") recibe con `recv_from` y timeout corto (2 ms): no bloquea al
//! juego y despierta en cuanto llega un paquete, lo que da marcas `t_pc_recv` precisas para la
//! sincronización (dormir entre lecturas no bloqueantes cuesta hasta 15 ms en Windows).
//! El hilo de juego solo toma el mutex para vaciar la cola en `poll()`.

use super::clock_sync::ClockSync;
use super::protocol::{self, Message, Packet, PhoneSample, DEFAULT_PORT};
use crate::clock::now_ns;
use crate::types::{DeviceInfo, Limb, MotionSample, MotionSource, SAMPLE_HAS_ACCEL, SAMPLE_HAS_GYRO};
use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

pub const TIMEOUT_NS: i64 = 3_000_000_000;
pub const SYNC_FAST_PERIOD_NS: i64 = 250_000_000;
pub const SYNC_FAST_PHASE_NS: i64 = 5_000_000_000;
pub const SYNC_SLOW_PERIOD_NS: i64 = 1_000_000_000;
/// Aviso de calidad: rtt mínimo por encima de esto.
pub const RTT_WARN_MS: f32 = 30.0;
const QUEUE_MAX: usize = 1 << 16;
const PENDING_MAX: usize = 2000;
/// Orden de extremidades por defecto para móviles nuevos.
pub const DEFAULT_LIMB_ORDER: [Limb; 4] = [Limb::RightHand, Limb::LeftHand, Limb::RightFoot, Limb::LeftFoot];

/// Estadísticas por móvil (para jd_record, depuración y UI).
#[derive(Clone, Debug, PartialEq)]
pub struct PhoneDeviceStats {
    pub source_id: u32,
    pub device_id: u32,
    pub name: String,
    pub addr: String,
    pub limb: Limb,
    pub connected: bool,
    pub battery: f32,
    /// rtt mínimo de la ventana de sincronización (ms), -1 sin datos.
    pub sync_rtt_ms: f32,
    pub drift_ppm: f64,
    pub synced: bool,
    pub samples_total: u64,
    /// Muestras/s medidas en el último segundo.
    pub rate_hz: f32,
    pub packets: u64,
    pub lost_packets: u64,
    /// Contadores que informa el móvil (STATUS).
    pub phone_sensor_rate_hz: u16,
    pub phone_dropped: u16,
    pub reconnects: u32,
}

struct Device {
    device_id: u32,
    source_id: u32,
    name: String,
    addr: SocketAddr,
    limb: Limb,
    connected: bool,
    battery: f32,
    sensor_rate_hz: u16,
    phone_dropped: u16,
    last_rx_ns: i64,
    last_seq: Option<u32>,
    lost: u64,
    packets: u64,
    samples_total: u64,
    rate_window_start: i64,
    rate_window_count: u64,
    rate_hz: f32,
    clock: ClockSync,
    pending: Vec<PhoneSample>,
    next_sync_ns: i64,
    fast_sync_until_ns: i64,
    last_t_phone: i64,
    last_t_out: i64,
    reconnects: u32,
    welcome_dirty: bool,
}

impl Device {
    fn begin_session(&mut self, now: i64, reset_clock: bool) {
        self.connected = true;
        self.last_rx_ns = now;
        self.next_sync_ns = now;
        self.fast_sync_until_ns = now + SYNC_FAST_PHASE_NS;
        self.last_seq = None;
        if reset_clock { self.clock.reset(); self.pending.clear(); }
    }

    fn to_sample(&self, s: &PhoneSample, t_ns: i64) -> MotionSample {
        MotionSample {
            t_ns,
            source_id: self.source_id,
            limb: self.limb as u8,
            flags: s.flags & (SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO),
            _reserved: 0,
            accel: s.accel,
            gyro: s.gyro,
        }
    }

    /// Convierte y encola; devuelve false si aún no hay estimación de reloj.
    fn emit(&mut self, s: &PhoneSample, queue: &mut VecDeque<MotionSample>, overflow: &mut u64) -> bool {
        let t_pc = match self.clock.phone_to_pc(s.t_ns) { Some(t) => t, None => return false };
        if s.t_ns <= self.last_t_phone { return true; } // duplicada o desordenada: se descarta
        if !s.accel.iter().chain(s.gyro.iter()).all(|x| x.is_finite()) { return true; }
        self.last_t_phone = s.t_ns;
        // t_ns estrictamente creciente por dispositivo aunque la estimación de offset se mueva.
        let t = if t_pc <= self.last_t_out { self.last_t_out + 1 } else { t_pc };
        self.last_t_out = t;
        if queue.len() >= QUEUE_MAX { queue.pop_front(); *overflow += 1; }
        queue.push_back(self.to_sample(s, t));
        self.samples_total += 1;
        self.rate_window_count += 1;
        true
    }

    fn stats(&self) -> PhoneDeviceStats {
        PhoneDeviceStats {
            source_id: self.source_id,
            device_id: self.device_id,
            name: self.name.clone(),
            addr: self.addr.to_string(),
            limb: self.limb,
            connected: self.connected,
            battery: self.battery,
            sync_rtt_ms: self.clock.min_rtt_ns().map(|r| r as f32 / 1e6).unwrap_or(-1.0),
            drift_ppm: self.clock.drift() * 1e6,
            synced: self.clock.has_estimate(),
            samples_total: self.samples_total,
            rate_hz: self.rate_hz,
            packets: self.packets,
            lost_packets: self.lost,
            phone_sensor_rate_hz: self.sensor_rate_hz,
            phone_dropped: self.phone_dropped,
            reconnects: self.reconnects,
        }
    }
}

struct Shared {
    code: u32,
    port: u16,
    endpoint: String,
    devices: Vec<Device>,
    queue: VecDeque<MotionSample>,
    queue_overflow: u64,
    next_source_id: u32,
    seq: u32,
    last_error: String,
    rejected_hellos: u64,
    bad_packets: u64,
    /// Extremidades para los móviles nuevos por orden de conexión (vacío = DEFAULT_LIMB_ORDER).
    limb_order: Vec<Limb>,
}

impl Shared {
    fn next_seq(&mut self) -> u32 { self.seq = self.seq.wrapping_add(1); self.seq }
    fn pick_default_limb(&self) -> Limb {
        // Orden configurado: el k-ésimo móvil nuevo recibe limb_order[k].
        if let Some(l) = self.limb_order.get(self.devices.len()) { return *l; }
        for l in DEFAULT_LIMB_ORDER {
            if !self.devices.iter().any(|d| d.connected && d.limb == l) { return l; }
        }
        Limb::None
    }
}

fn lock(m: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

enum Out {
    Packet(SocketAddr, Packet),
    /// SYNC_REQ: t_pc_send se toma justo antes de enviar.
    Sync(SocketAddr, u32, u32),
}

/// Acceso compartido (clonable) a endpoint, código y estadísticas, aunque la fuente esté envuelta
/// en un RecorderSource u otra caja.
#[derive(Clone)]
pub struct PhoneServerHandle { shared: Arc<Mutex<Shared>> }

impl PhoneServerHandle {
    pub fn endpoint(&self) -> String { lock(&self.shared).endpoint.clone() }
    pub fn pairing_code(&self) -> u32 { lock(&self.shared).code }
    pub fn stats(&self) -> Vec<PhoneDeviceStats> { lock(&self.shared).devices.iter().map(|d| d.stats()).collect() }
    pub fn rejected_hellos(&self) -> u64 { lock(&self.shared).rejected_hellos }
}

pub struct PhoneServerSource {
    requested_port: u16,
    socket: Option<UdpSocket>,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    local_ip: IpAddr,
}

/// IP LAN principal: "conecta" un socket UDP a 8.8.8.8:80 (no envía nada) y lee la IP local
/// elegida por la tabla de rutas. Sin red → 127.0.0.1.
pub fn detect_lan_ip() -> IpAddr {
    let fallback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let s = match UdpSocket::bind("0.0.0.0:0") { Ok(s) => s, Err(_) => return fallback };
    if s.connect("8.8.8.8:80").is_err() { return fallback; }
    match s.local_addr() {
        Ok(a) if !a.ip().is_unspecified() => a.ip(),
        _ => fallback,
    }
}

/// Código aleatorio de 4 dígitos (1000..=9999) sin dependencias externas.
pub fn random_pairing_code() -> u32 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_i64(now_ns());
    h.write_u32(std::process::id());
    1000 + (h.finish() % 9000) as u32
}

impl PhoneServerSource {
    /// `port` literal (0 = puerto efímero del sistema, útil en tests; la API C traduce 0 → 47800).
    /// `pairing_code` 0 = aleatorio. Intenta abrir el puerto ya; si falla, `start()` lo reintenta.
    pub fn new(port: u16, pairing_code: u32) -> PhoneServerSource {
        let code = if pairing_code == 0 { random_pairing_code() } else { pairing_code };
        let local_ip = detect_lan_ip();
        let shared = Arc::new(Mutex::new(Shared {
            code, port, endpoint: format!("{}:{}", local_ip, port), devices: Vec::new(),
            queue: VecDeque::new(), queue_overflow: 0, next_source_id: 1, seq: 0,
            last_error: String::new(), rejected_hellos: 0, bad_packets: 0, limb_order: Vec::new(),
        }));
        let mut s = PhoneServerSource { requested_port: port, socket: None, shared, stop: Arc::new(AtomicBool::new(false)), thread: None, local_ip };
        if let Err(e) = s.bind() { lock(&s.shared).last_error = e; }
        s
    }

    /// Puerto por defecto del contrato.
    pub fn with_default_port(pairing_code: u32) -> PhoneServerSource { PhoneServerSource::new(DEFAULT_PORT, pairing_code) }

    fn bind(&mut self) -> Result<(), String> {
        if self.socket.is_some() { return Ok(()); }
        let sock = UdpSocket::bind(("0.0.0.0", self.requested_port))
            .map_err(|e| format!("no se puede abrir el puerto UDP {}: {}", self.requested_port, e))?;
        let port = sock.local_addr().map(|a| a.port()).unwrap_or(self.requested_port);
        {
            let mut sh = lock(&self.shared);
            sh.port = port;
            sh.endpoint = format!("{}:{}", self.local_ip, port);
            sh.last_error.clear();
        }
        self.socket = Some(sock);
        Ok(())
    }

    pub fn handle(&self) -> PhoneServerHandle { PhoneServerHandle { shared: self.shared.clone() } }
    /// "IP:puerto" de la interfaz LAN principal.
    pub fn endpoint(&self) -> String { lock(&self.shared).endpoint.clone() }
    pub fn pairing_code(&self) -> u32 { lock(&self.shared).code }
    pub fn local_port(&self) -> u16 { lock(&self.shared).port }
    pub fn stats(&self) -> Vec<PhoneDeviceStats> { self.handle().stats() }
    pub fn is_running(&self) -> bool { self.thread.is_some() }
    /// Extremidades para los móviles nuevos, por orden de conexión (el 1.º recibe `order[0]`...).
    /// Los que pasen de la lista siguen el orden por defecto. Se aplica en el HELLO, antes de la
    /// primera muestra, así que las muestras y la cabecera de una grabación ya salen con ella.
    pub fn set_limb_order(&mut self, order: &[Limb]) { lock(&self.shared).limb_order = order.to_vec(); }
}

impl MotionSource for PhoneServerSource {
    fn start(&mut self) -> Result<(), String> {
        if self.thread.is_some() { return Ok(()); }
        if let Err(e) = self.bind() { lock(&self.shared).last_error = e.clone(); return Err(e); }
        let sock = self.socket.as_ref().unwrap().try_clone().map_err(|e| format!("socket: {}", e))?;
        sock.set_read_timeout(Some(Duration::from_millis(2))).map_err(|e| format!("socket: {}", e))?;
        self.stop.store(false, Ordering::SeqCst);
        let (shared, stop) = (self.shared.clone(), self.stop.clone());
        let th = std::thread::Builder::new().name("jd-phone-server".into())
            .spawn(move || server_loop(sock, shared, stop))
            .map_err(|e| format!("no se puede crear el hilo del servidor: {}", e))?;
        self.thread = Some(th);
        Ok(())
    }

    fn stop(&mut self) {
        let th = match self.thread.take() { Some(t) => t, None => return };
        self.stop.store(true, Ordering::SeqCst);
        let _ = th.join();
        // Despedida a los móviles conectados.
        let mut out = Vec::new();
        {
            let mut sh = lock(&self.shared);
            let targets: Vec<(SocketAddr, u32)> = sh.devices.iter().filter(|d| d.connected).map(|d| (d.addr, d.device_id)).collect();
            for (addr, id) in targets {
                let seq = sh.next_seq();
                out.push((addr, protocol::encode(&Packet { device_id: id, seq, msg: Message::Bye })));
            }
            for d in sh.devices.iter_mut() { d.connected = false; d.pending.clear(); }
        }
        if let Some(sock) = &self.socket { for (a, b) in out { let _ = sock.send_to(&b, a); } }
    }

    fn poll(&mut self, out: &mut Vec<MotionSample>) -> usize {
        let mut sh = lock(&self.shared);
        let n = sh.queue.len();
        out.extend(sh.queue.drain(..));
        n
    }

    fn devices(&self) -> Vec<DeviceInfo> {
        let sh = lock(&self.shared);
        sh.devices.iter().map(|d| DeviceInfo {
            source_id: d.source_id,
            name: d.name.clone(),
            kind: "phone".into(),
            limb: d.limb,
            connected: d.connected,
            battery: d.battery,
            sync_rtt_ms: d.clock.min_rtt_ns().map(|r| r as f32 / 1e6).unwrap_or(-1.0),
        }).collect()
    }

    fn assign_limb(&mut self, source_id: u32, limb: Limb) {
        let mut sh = lock(&self.shared);
        if let Some(d) = sh.devices.iter_mut().find(|d| d.source_id == source_id) {
            d.limb = limb;
            d.welcome_dirty = true; // el hilo reenvía WELCOME para que el móvil muestre la extremidad
        }
    }

    fn last_error(&self) -> String { lock(&self.shared).last_error.clone() }
}

impl Drop for PhoneServerSource {
    fn drop(&mut self) { self.stop(); }
}

// ------------------------------------------------------------------ hilo del servidor

fn server_loop(sock: UdpSocket, shared: Arc<Mutex<Shared>>, stop: Arc<AtomicBool>) {
    let mut buf = [0u8; 4096];
    let mut out: Vec<Out> = Vec::new();
    let mut last_housekeeping = 0i64;
    while !stop.load(Ordering::SeqCst) {
        match sock.recv_from(&mut buf) {
            Ok((n, addr)) => {
                let t_recv = now_ns();
                let mut sh = lock(&shared);
                handle_datagram(&mut sh, &buf[..n], addr, t_recv, &mut out);
            }
            Err(e) => match e.kind() {
                // ConnectionReset: Windows lo devuelve tras un ICMP "puerto inalcanzable"; no es fatal.
                ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted | ErrorKind::ConnectionReset => {}
                _ => {
                    lock(&shared).last_error = format!("error de recepción UDP: {}", e);
                    std::thread::sleep(Duration::from_millis(5));
                }
            },
        }
        let now = now_ns();
        if now - last_housekeeping >= 1_000_000 {
            last_housekeeping = now;
            let mut sh = lock(&shared);
            housekeeping(&mut sh, now, &mut out);
        }
        if !out.is_empty() { flush_out(&sock, &shared, &mut out); }
    }
}

fn flush_out(sock: &UdpSocket, shared: &Arc<Mutex<Shared>>, out: &mut Vec<Out>) {
    for o in out.drain(..) {
        let (addr, bytes) = match o {
            Out::Packet(a, p) => (a, protocol::encode(&p)),
            Out::Sync(a, device_id, seq) => {
                (a, protocol::encode(&Packet { device_id, seq, msg: Message::SyncReq { t_pc_send: now_ns() } }))
            }
        };
        if let Err(e) = sock.send_to(&bytes, addr) {
            if e.kind() != ErrorKind::WouldBlock { lock(shared).last_error = format!("error de envío UDP a {}: {}", addr, e); }
        }
    }
}

fn welcome(sh: &mut Shared, addr: SocketAddr, device_id: u32, source_id: u32, limb: u8, accepted: bool) -> Out {
    let seq = sh.next_seq();
    Out::Packet(addr, Packet { device_id, seq, msg: Message::Welcome { source_id, limb, accepted } })
}

fn handle_datagram(sh: &mut Shared, data: &[u8], addr: SocketAddr, now: i64, out: &mut Vec<Out>) {
    let p = match protocol::decode(data) {
        Ok(p) => p,
        Err(_) => { sh.bad_packets += 1; return; }
    };
    let idx = sh.devices.iter().position(|d| d.device_id == p.device_id);
    match p.msg {
        Message::Discover => {
            let o = welcome(sh, addr, p.device_id, 0, Limb::None as u8, false);
            out.push(o);
        }
        Message::Hello { pairing_code, name } => {
            if pairing_code != sh.code {
                sh.rejected_hellos += 1;
                sh.last_error = format!("código de emparejado incorrecto desde {} ({})", addr, name);
                let o = welcome(sh, addr, p.device_id, 0, Limb::None as u8, false);
                out.push(o);
                return;
            }
            let i = match idx {
                Some(i) => {
                    let d = &mut sh.devices[i];
                    d.addr = addr;
                    if !name.is_empty() { d.name = name; }
                    if d.connected {
                        // Reintento de HELLO: solo repetir WELCOME.
                        d.last_rx_ns = now;
                        d.last_seq = Some(p.seq);
                    } else {
                        d.reconnects += 1;
                        d.begin_session(now, true);
                        d.last_seq = Some(p.seq);
                    }
                    i
                }
                None => {
                    let limb = sh.pick_default_limb();
                    let source_id = sh.next_source_id;
                    sh.next_source_id += 1;
                    let mut d = Device {
                        device_id: p.device_id, source_id,
                        name: if name.is_empty() { format!("móvil {}", source_id) } else { name },
                        addr, limb, connected: true, battery: -1.0, sensor_rate_hz: 0, phone_dropped: 0,
                        last_rx_ns: now, last_seq: None, lost: 0, packets: 0, samples_total: 0,
                        rate_window_start: now, rate_window_count: 0, rate_hz: 0.0,
                        clock: ClockSync::new(), pending: Vec::new(), next_sync_ns: now, fast_sync_until_ns: now,
                        last_t_phone: i64::MIN, last_t_out: i64::MIN, reconnects: 0, welcome_dirty: false,
                    };
                    d.begin_session(now, true);
                    d.last_seq = Some(p.seq);
                    sh.devices.push(d);
                    sh.devices.len() - 1
                }
            };
            sh.devices[i].packets += 1;
            if sh.last_error.starts_with("código de emparejado") { sh.last_error.clear(); }
            let (sid, limb) = (sh.devices[i].source_id, sh.devices[i].limb as u8);
            let o = welcome(sh, addr, p.device_id, sid, limb, true);
            out.push(o);
        }
        msg => {
            // Resto de mensajes: solo de móviles ya emparejados.
            let i = match idx { Some(i) => i, None => return };
            let Shared { devices, queue, queue_overflow, .. } = sh;
            let d = &mut devices[i];
            if !d.connected {
                if matches!(msg, Message::Samples(_)) && d.addr == addr {
                    // El móvil siguió enviando tras un corte (> 3 s): ya estaba emparejado, se reanuda
                    // sin reiniciar el reloj (el móvil no ha cambiado de base).
                    d.reconnects += 1;
                    d.begin_session(now, false);
                    d.welcome_dirty = true;
                } else {
                    return;
                }
            }
            d.addr = addr;
            d.last_rx_ns = now;
            d.packets += 1;
            if let Some(last) = d.last_seq {
                let diff = p.seq.wrapping_sub(last) as i32;
                if diff > 1 { d.lost += (diff - 1) as u64; }
                if diff > 0 { d.last_seq = Some(p.seq); }
            } else {
                d.last_seq = Some(p.seq);
            }
            match msg {
                Message::Samples(v) => {
                    for s in &v {
                        if !d.emit(s, queue, queue_overflow) {
                            if d.pending.len() < PENDING_MAX { d.pending.push(*s); }
                        }
                    }
                }
                Message::SyncResp { t_pc_send, t_phone_recv, t_phone_send } => {
                    // Solo respuestas a peticiones plausibles (últimos 10 s, no del futuro).
                    if t_pc_send <= now && now - t_pc_send < 10_000_000_000 {
                        d.clock.add(t_pc_send, t_phone_recv, t_phone_send, now);
                        if d.clock.has_estimate() && !d.pending.is_empty() {
                            let pend = std::mem::take(&mut d.pending);
                            for s in &pend { d.emit(s, queue, queue_overflow); }
                        }
                    }
                }
                Message::Status { battery, sensor_rate_hz, dropped } => {
                    d.battery = if battery.is_finite() && battery >= 0.0 { battery.min(1.0) } else { -1.0 };
                    d.sensor_rate_hz = sensor_rate_hz;
                    d.phone_dropped = dropped;
                }
                Message::Bye => { d.connected = false; d.pending.clear(); }
                _ => {}
            }
        }
    }
}

fn housekeeping(sh: &mut Shared, now: i64, out: &mut Vec<Out>) {
    let mut msgs: Vec<(SocketAddr, u32, Option<(u32, u8)>)> = Vec::new(); // (addr, device_id, welcome?)
    for d in sh.devices.iter_mut() {
        if now - d.rate_window_start >= 1_000_000_000 {
            d.rate_hz = d.rate_window_count as f32 * 1e9 / (now - d.rate_window_start) as f32;
            d.rate_window_start = now;
            d.rate_window_count = 0;
        }
        if !d.connected { continue; }
        if now - d.last_rx_ns > TIMEOUT_NS {
            d.connected = false;
            d.pending.clear();
            continue;
        }
        if d.welcome_dirty {
            d.welcome_dirty = false;
            msgs.push((d.addr, d.device_id, Some((d.source_id, d.limb as u8))));
        }
        if now >= d.next_sync_ns {
            let period = if now < d.fast_sync_until_ns { SYNC_FAST_PERIOD_NS } else { SYNC_SLOW_PERIOD_NS };
            d.next_sync_ns = now + period;
            msgs.push((d.addr, d.device_id, None));
        }
    }
    for (addr, id, w) in msgs {
        match w {
            Some((sid, limb)) => { let o = welcome(sh, addr, id, sid, limb, true); out.push(o); }
            None => { let seq = sh.next_seq(); out.push(Out::Sync(addr, id, seq)); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared(code: u32) -> Shared {
        Shared { code, port: 1, endpoint: String::new(), devices: Vec::new(), queue: VecDeque::new(), queue_overflow: 0,
            next_source_id: 1, seq: 0, last_error: String::new(), rejected_hellos: 0, bad_packets: 0, limb_order: Vec::new() }
    }
    fn addr(p: u16) -> SocketAddr { SocketAddr::from(([127, 0, 0, 1], p)) }
    fn pkt(id: u32, seq: u32, msg: Message) -> Vec<u8> { protocol::encode(&Packet { device_id: id, seq, msg }) }
    fn welcome_of(o: &Out) -> Option<(u32, u8, bool)> {
        match o { Out::Packet(_, Packet { msg: Message::Welcome { source_id, limb, accepted }, .. }) => Some((*source_id, *limb, *accepted)), _ => None }
    }

    #[test]
    fn pairing_limbs_and_garbage() {
        let mut sh = shared(4321);
        let mut out = Vec::new();
        handle_datagram(&mut sh, b"basura", addr(1), 10, &mut out);
        assert!(out.is_empty());
        assert_eq!(sh.bad_packets, 1);
        handle_datagram(&mut sh, &pkt(9, 1, Message::Hello { pairing_code: 1111, name: "malo".into() }), addr(1), 10, &mut out);
        assert_eq!(welcome_of(&out[0]), Some((0, 255, false)));
        assert!(sh.devices.is_empty());
        out.clear();
        let order = [Limb::RightHand, Limb::LeftHand, Limb::RightFoot, Limb::LeftFoot, Limb::None];
        for (k, l) in order.iter().enumerate() {
            handle_datagram(&mut sh, &pkt(100 + k as u32, 1, Message::Hello { pairing_code: 4321, name: format!("m{}", k) }), addr(10 + k as u16), 20, &mut out);
            assert_eq!(welcome_of(out.last().unwrap()), Some((k as u32 + 1, *l as u8, true)));
        }
        // HELLO repetido: mismo source_id, sin reconexión.
        handle_datagram(&mut sh, &pkt(100, 2, Message::Hello { pairing_code: 4321, name: "m0".into() }), addr(10), 30, &mut out);
        assert_eq!(welcome_of(out.last().unwrap()), Some((1, Limb::RightHand as u8, true)));
        assert_eq!(sh.devices[0].reconnects, 0);
        // DISCOVER
        handle_datagram(&mut sh, &pkt(555, 1, Message::Discover), addr(99), 40, &mut out);
        assert_eq!(welcome_of(out.last().unwrap()), Some((0, 255, false)));
        // SAMPLES de un desconocido: ignoradas.
        handle_datagram(&mut sh, &pkt(777, 1, Message::Samples(vec![PhoneSample::default()])), addr(98), 40, &mut out);
        assert!(sh.queue.is_empty());
    }

    #[test]
    fn configured_limb_order() {
        let mut sh = shared(1);
        sh.limb_order = vec![Limb::LeftFoot, Limb::RightFoot];
        let mut out = Vec::new();
        for k in 0..3u32 {
            handle_datagram(&mut sh, &pkt(10 + k, 1, Message::Hello { pairing_code: 1, name: "p".into() }), addr(1 + k as u16), 0, &mut out);
        }
        let limbs: Vec<Limb> = sh.devices.iter().map(|d| d.limb).collect();
        assert_eq!(limbs, vec![Limb::LeftFoot, Limb::RightFoot, Limb::RightHand]);
    }

    #[test]
    fn samples_wait_for_sync_then_convert_and_count_losses() {
        let mut sh = shared(1);
        let mut out = Vec::new();
        handle_datagram(&mut sh, &pkt(5, 1, Message::Hello { pairing_code: 1, name: "p".into() }), addr(1), 1_000, &mut out);
        let s = |t| PhoneSample { t_ns: t, flags: 3, accel: [1.0, 2.0, 3.0], gyro: [0.0; 3] };
        handle_datagram(&mut sh, &pkt(5, 2, Message::Samples(vec![s(50_000), s(55_000)])), addr(1), 2_000, &mut out);
        assert!(sh.queue.is_empty());
        assert_eq!(sh.devices[0].pending.len(), 2);
        // offset del móvil = +40_000 ns, rtt 100.
        handle_datagram(&mut sh, &pkt(5, 3, Message::SyncResp { t_pc_send: 2_900, t_phone_recv: 42_950, t_phone_send: 42_950 }), addr(1), 3_000, &mut out);
        assert_eq!(sh.queue.len(), 2);
        assert_eq!(sh.queue[0].t_ns, 10_000);
        assert_eq!(sh.queue[1].t_ns, 15_000);
        assert_eq!(sh.queue[0].limb, Limb::RightHand as u8);
        // seq 4 y 5 perdidos; repetida (t <= último) descartada.
        handle_datagram(&mut sh, &pkt(5, 6, Message::Samples(vec![s(55_000), s(60_000)])), addr(1), 4_000, &mut out);
        assert_eq!(sh.devices[0].lost, 2);
        assert_eq!(sh.queue.len(), 3);
        handle_datagram(&mut sh, &pkt(5, 7, Message::Status { battery: 0.5, sensor_rate_hz: 200, dropped: 1 }), addr(1), 5_000, &mut out);
        assert_eq!(sh.devices[0].battery, 0.5);
        handle_datagram(&mut sh, &pkt(5, 8, Message::Status { battery: f32::NAN, sensor_rate_hz: 200, dropped: 1 }), addr(1), 5_000, &mut out);
        assert_eq!(sh.devices[0].battery, -1.0);
    }

    #[test]
    fn timeout_sync_schedule_and_reconnect() {
        let mut sh = shared(1);
        let mut out = Vec::new();
        handle_datagram(&mut sh, &pkt(5, 1, Message::Hello { pairing_code: 1, name: "p".into() }), addr(1), 0, &mut out);
        out.clear();
        let mut syncs = 0;
        let mut t = 0;
        while t <= 2_000_000_000 {
            housekeeping(&mut sh, t, &mut out);
            syncs += out.iter().filter(|o| matches!(o, Out::Sync(..))).count();
            out.clear();
            t += 1_000_000;
        }
        assert_eq!(syncs, 9); // 0, 250, ..., 2000 ms
        housekeeping(&mut sh, 3_100_000_000, &mut out);
        assert!(!sh.devices[0].connected);
        handle_datagram(&mut sh, &pkt(5, 1, Message::Hello { pairing_code: 1, name: "p".into() }), addr(2), 4_000_000_000, &mut out);
        assert!(sh.devices[0].connected);
        assert_eq!(sh.devices[0].reconnects, 1);
        assert_eq!(sh.devices[0].source_id, 1);
        assert_eq!(sh.devices[0].addr, addr(2));
    }
}
