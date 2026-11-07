//! Lógica de la GDExtension SIN dependencia de Godot (agente G).
//!
//! `lib.rs` solo traduce tipos de Godot <-> estos tipos. Así esta parte se puede compilar y probar sin
//! la crate `godot` (ver `scripts/run_tests.sh`, que la incluye en un crate de prueba con `#[path]`).
//!
//! Igual que `jd_capi/src/sources.rs`, la fuente vive en `Arc<Mutex<..>>` para que un grabador pueda
//! envolverla sin invalidar el `JDSource` original de GDScript: ambos objetos siguen siendo válidos,
//! pero solo se debe leer (poll/pump) del grabador.
use jd_core::clock;
use jd_core::engine::Engine;
use jd_core::phone::{PhoneServerHandle, PhoneServerSource, DEFAULT_PORT};
use jd_core::recorder::RecorderSource;
use jd_core::replay::ReplaySource;
use jd_core::types::{DeviceInfo, JudgeConfig, Limb, MotionSample, MotionSource, StepResult};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

pub type SharedSource = Arc<Mutex<dyn MotionSource>>;

fn lock<T: ?Sized>(m: &Mutex<T>) -> MutexGuard<'_, T> { m.lock().unwrap_or_else(|e| e.into_inner()) }

/// Adaptador: `MotionSource` que delega en una fuente compartida (para meterla en el grabador).
struct Delegate(SharedSource);

impl MotionSource for Delegate {
    fn start(&mut self) -> Result<(), String> { lock(&self.0).start() }
    fn stop(&mut self) { lock(&self.0).stop() }
    fn poll(&mut self, out: &mut Vec<MotionSample>) -> usize { lock(&self.0).poll(out) }
    fn devices(&self) -> Vec<DeviceInfo> { lock(&self.0).devices() }
    fn assign_limb(&mut self, source_id: u32, limb: Limb) { lock(&self.0).assign_limb(source_id, limb) }
    fn last_error(&self) -> String { lock(&self.0).last_error() }
}

/// `limb` de GDScript (int) -> `Limb`; fuera de 0..=3 es `Limb::None`.
pub fn limb_from_int(v: i64) -> Limb {
    if (0..=3).contains(&v) { Limb::from_u8(v as u8) } else { Limb::None }
}

/// Puerto de GDScript: 0 o fuera de rango -> 47800 (como la API C).
pub fn port_from_int(v: i64) -> u16 {
    if v <= 0 || v > u16::MAX as i64 { DEFAULT_PORT } else { v as u16 }
}

/// Código de emparejado: fuera de 1000..=9999 -> 0 (= aleatorio).
pub fn code_from_int(v: i64) -> u32 {
    if (1000..=9999).contains(&v) { v as u32 } else { 0 }
}

/// Núcleo de `JDSource`.
pub struct SourceCore {
    src: SharedSource,
    recorder: Option<Arc<Mutex<RecorderSource>>>,
    phone: Option<PhoneServerHandle>,
    /// Muestras ya sacadas de la fuente que no cupieron en el último `poll_samples(max)`.
    backlog: Vec<MotionSample>,
    error: String,
}

impl SourceCore {
    fn from_shared(src: SharedSource) -> SourceCore {
        SourceCore { src, recorder: None, phone: None, backlog: Vec::new(), error: String::new() }
    }

    /// Envuelve cualquier fuente (pruebas, fuentes futuras).
    pub fn from_source<S: MotionSource + 'static>(src: S) -> SourceCore {
        SourceCore::from_shared(Arc::new(Mutex::new(src)))
    }

    pub fn phone_server(port: i64, pairing_code: i64) -> SourceCore {
        let server = PhoneServerSource::new(port_from_int(port), code_from_int(pairing_code));
        let phone = server.handle();
        let mut core = SourceCore::from_shared(Arc::new(Mutex::new(server)));
        core.phone = Some(phone);
        core
    }

    /// `None` si el archivo no existe o no se puede leer (contrato: `create_replay` devuelve null).
    pub fn replay(path: &str, speed: f64) -> Result<SourceCore, String> {
        let p = Path::new(path);
        if !p.is_file() { return Err(format!("{}: no existe", path)); }
        let speed = if speed.is_finite() && speed > 0.0 { speed } else { 1.0 };
        let r = ReplaySource::from_path(p, speed)?;
        Ok(SourceCore::from_shared(Arc::new(Mutex::new(r))))
    }

    pub fn recorder(inner: &SourceCore, out_path: &str) -> Result<SourceCore, String> {
        let rec = RecorderSource::new(Box::new(Delegate(inner.src.clone())), Path::new(out_path))?;
        let rec = Arc::new(Mutex::new(rec));
        let shared: SharedSource = rec.clone();
        let mut core = SourceCore::from_shared(shared);
        core.recorder = Some(rec);
        core.phone = inner.phone.clone();
        Ok(core)
    }

    pub fn endpoint(&self) -> String { self.phone.as_ref().map(|p| p.endpoint()).unwrap_or_default() }
    pub fn pairing_code(&self) -> u32 { self.phone.as_ref().map(|p| p.pairing_code()).unwrap_or(0) }

    pub fn start(&mut self) -> bool {
        match lock(&self.src).start() {
            Ok(()) => { self.error.clear(); true }
            Err(e) => { self.error = e; false }
        }
    }

    pub fn stop(&mut self) {
        lock(&self.src).stop();
        self.backlog.clear();
    }

    pub fn devices(&self) -> Vec<DeviceInfo> { lock(&self.src).devices() }

    pub fn assign_limb(&mut self, id: i64, limb: i64) {
        if id < 0 || id > u32::MAX as i64 { return; }
        lock(&self.src).assign_limb(id as u32, limb_from_int(limb));
    }

    /// `false` si esta fuente no es un grabador.
    pub fn set_recorder_mapping(&mut self, mono_to_song_s: f64) -> bool {
        match &self.recorder {
            Some(r) => { lock(r).set_mapping(mono_to_song_s); true }
            None => false,
        }
    }

    /// Hasta `max` muestras (primero las que quedaron pendientes).
    pub fn poll_samples(&mut self, max: usize, out: &mut Vec<MotionSample>) -> usize {
        lock(&self.src).poll(&mut self.backlog);
        let n = max.min(self.backlog.len());
        out.extend(self.backlog.drain(..n));
        n
    }

    /// Todas las muestras disponibles (pendientes + nuevas), para `JDEngine.pump`.
    pub fn drain_all(&mut self, out: &mut Vec<MotionSample>) -> usize {
        let from = out.len();
        out.append(&mut self.backlog);
        lock(&self.src).poll(out);
        out.len() - from
    }

    pub fn last_error(&self) -> String {
        let inner = lock(&self.src).last_error();
        if !inner.is_empty() { inner } else { self.error.clone() }
    }

    pub fn is_recorder(&self) -> bool { self.recorder.is_some() }
}

/// Núcleo de `JDEngine`.
pub struct EngineCore {
    engine: Engine,
    samples: Vec<MotionSample>,
    results: Vec<StepResult>,
}

impl Default for EngineCore {
    fn default() -> Self { EngineCore { engine: Engine::new(), samples: Vec::new(), results: Vec::new() } }
}

impl EngineCore {
    pub fn now_ns() -> i64 { clock::now_ns() }

    /// "" si OK; si no, el mensaje de error.
    pub fn load_song(&mut self, song_dir: &str) -> String {
        match self.engine.load_song(Path::new(song_dir)) {
            Ok(()) => String::new(),
            Err(e) => e,
        }
    }

    pub fn set_config(&mut self, perfect_ms: f64, good_ms: f64, foot_scale: f64) {
        let d = JudgeConfig::default();
        let pick = |v: f64, def: f32| if v.is_finite() && v > 0.0 { v as f32 } else { def };
        self.engine.set_config(JudgeConfig {
            perfect_window_ms: pick(perfect_ms, d.perfect_window_ms),
            good_window_ms: pick(good_ms, d.good_window_ms),
            foot_window_scale: pick(foot_scale, d.foot_window_scale),
        });
    }

    pub fn config(&self) -> JudgeConfig { self.engine.config() }

    pub fn set_time_mapping(&mut self, mono_to_song_s: f64) {
        if mono_to_song_s.is_finite() { self.engine.set_time_mapping(mono_to_song_s); }
    }

    /// Mueve todas las muestras de la fuente al motor (sin pasar por GDScript). Devuelve cuántas.
    pub fn pump(&mut self, source: &mut SourceCore) -> usize {
        self.samples.clear();
        let n = source.drain_all(&mut self.samples);
        if n > 0 {
            // El procesado de señal espera t_ns creciente; con varios móviles pueden llegar mezclados.
            self.samples.sort_by_key(|s| s.t_ns);
            self.engine.push_samples(&self.samples);
        }
        n
    }

    pub fn push_samples(&mut self, s: &[MotionSample]) { self.engine.push_samples(s); }

    pub fn update(&mut self, song_time_s: f64) -> &[StepResult] {
        self.results.clear();
        if song_time_s.is_finite() { self.engine.update(song_time_s, &mut self.results); }
        &self.results
    }

    pub fn reset(&mut self) {
        self.engine.reset();
        self.samples.clear();
        self.results.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jd_core::types::{SAMPLE_HAS_ACCEL, SAMPLE_HAS_GYRO};
    use std::io::Write;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("jd_godot_test_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    /// Sesión "song" de 2 s con golpes de mano derecha en 0.5 s y 1.5 s, a 200 Hz.
    fn write_session(path: &Path) {
        let mut f = std::fs::File::create(path).unwrap();
        writeln!(f, "{{\"format\":\"jd-session\",\"version\":1,\"time_base\":\"song\",\"song_id\":\"t\",\"recorded_at\":\"2026-01-01T00:00:00Z\",\"devices\":[{{\"source_id\":1,\"name\":\"m\",\"kind\":\"phone\",\"limb\":\"right_hand\"}}]}}").unwrap();
        for i in 0..400 {
            let t = i as f64 * 0.005;
            let mut ay = 9.80665;
            for h in [0.5, 1.5] { ay += 20.0 * (-(t - h) * (t - h) / (2.0 * 0.012f64 * 0.012)).exp(); }
            writeln!(f, "{{\"t\":{:.6},\"src\":1,\"limb\":\"right_hand\",\"a\":[0,{:.5},0],\"g\":[0,0,0]}}", t, ay).unwrap();
        }
    }

    #[test]
    fn int_conversions() {
        assert_eq!(limb_from_int(0), Limb::LeftHand);
        assert_eq!(limb_from_int(3), Limb::RightFoot);
        assert_eq!(limb_from_int(4), Limb::None);
        assert_eq!(limb_from_int(-1), Limb::None);
        assert_eq!(port_from_int(0), 47800);
        assert_eq!(port_from_int(70000), 47800);
        assert_eq!(port_from_int(5000), 5000);
        assert_eq!(code_from_int(1234), 1234);
        assert_eq!(code_from_int(0), 0);
        assert_eq!(code_from_int(99999), 0);
    }

    #[test]
    fn replay_missing_file_is_error() {
        assert!(SourceCore::replay("/no/existe.jsonl", 1.0).is_err());
    }

    #[test]
    fn replay_poll_and_backlog() {
        let p = tmp("poll.jsonl");
        write_session(&p);
        let mut s = SourceCore::replay(p.to_str().unwrap(), 1000.0).unwrap();
        assert!(s.start());
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut got = Vec::new();
        let mut guard = 0;
        while got.len() < 400 && guard < 200 {
            let n = s.poll_samples(64, &mut got);
            assert!(n <= 64);
            guard += 1;
            if n == 0 { std::thread::sleep(std::time::Duration::from_millis(5)); }
        }
        assert_eq!(got.len(), 400);
        assert!(got.windows(2).all(|w| w[0].t_ns <= w[1].t_ns));
        assert!(got.iter().all(|m| m.flags & (SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO) != 0));
        assert_eq!(s.devices().len(), 1);
        s.assign_limb(1, 0);
        s.stop();
        assert_eq!(s.endpoint(), "");
        assert_eq!(s.pairing_code(), 0);
        assert!(!s.set_recorder_mapping(0.0));
    }

    #[test]
    fn recorder_wraps_replay_and_pump_feeds_engine() {
        let p = tmp("rec_in.jsonl");
        let out = tmp("rec_out.jsonl");
        write_session(&p);
        let inner = SourceCore::replay(p.to_str().unwrap(), 1000.0).unwrap();
        let mut rec = SourceCore::recorder(&inner, out.to_str().unwrap()).unwrap();
        assert!(rec.is_recorder());
        assert!(rec.set_recorder_mapping(0.0));
        assert!(rec.start());
        let mut eng = EngineCore::default();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut total = 0;
        for _ in 0..200 {
            total += eng.pump(&mut rec);
            if total >= 400 { break; }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(total, 400);
        rec.stop();
        drop(rec);
        let text = std::fs::read_to_string(&out).unwrap();
        assert_eq!(text.lines().count(), 401, "cabecera + 400 muestras");
        // Sin canción cargada, update no da resultados.
        assert!(eng.update(10.0).is_empty());
        assert!(!eng.load_song("/no/existe").is_empty());
        eng.set_config(30.0, -1.0, f64::NAN);
        let c = eng.config();
        assert_eq!(c.perfect_window_ms, 30.0);
        assert_eq!(c.good_window_ms, 100.0);
        assert_eq!(c.foot_window_scale, 1.5);
        eng.reset();
    }

    /// Fuente que entrega una sesión en tiempo de canción hasta `until_ns` (como fotogramas).
    struct Chunked { samples: Vec<MotionSample>, next: usize, until_ns: Arc<std::sync::atomic::AtomicI64> }
    impl MotionSource for Chunked {
        fn start(&mut self) -> Result<(), String> { Ok(()) }
        fn stop(&mut self) {}
        fn poll(&mut self, out: &mut Vec<MotionSample>) -> usize {
            let until = self.until_ns.load(std::sync::atomic::Ordering::SeqCst);
            let from = self.next;
            while self.next < self.samples.len() && self.samples[self.next].t_ns <= until { self.next += 1; }
            out.extend_from_slice(&self.samples[from..self.next]);
            self.next - from
        }
        fn devices(&self) -> Vec<DeviceInfo> { Vec::new() }
        fn assign_limb(&mut self, _: u32, _: Limb) {}
    }

    #[test]
    fn pump_scores_synthetic_perfect_session() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/sessions/synthetic");
        let song = root.join("song_demo");
        if !song.join("choreo.json").is_file() { eprintln!("sin datos sintéticos; se omite"); return; }
        let session = jd_core::session::load_session(&root.join("perfect.jsonl")).unwrap();
        let mut samples = session.samples.clone();
        samples.sort_by_key(|s| s.t_ns);
        let until = Arc::new(std::sync::atomic::AtomicI64::new(0));
        let mut core = SourceCore::from_source(Chunked { samples, next: 0, until_ns: until.clone() });
        let mut eng = EngineCore::default();
        assert_eq!(eng.load_song(song.to_str().unwrap()), "");
        eng.set_time_mapping(0.0);
        let mut grades = [0usize; 3];
        let mut pumped = 0;
        let mut t = 0.0;
        while t < 32.0 {
            t += 0.016;
            until.store((t * 1e9) as i64, std::sync::atomic::Ordering::SeqCst);
            pumped += eng.pump(&mut core);
            for r in eng.update(t) { grades[r.grade as usize] += 1; }
        }
        assert_eq!(pumped, session.samples.len());
        let total: usize = grades.iter().sum();
        assert_eq!(total, 27, "{:?}", grades);
        assert!(grades[2] >= 24, "Miss/Good/Perfect = {:?}", grades);
    }

    #[test]
    fn phone_server_endpoint_and_code() {
        // Puerto alto poco probable de estar ocupado; si lo está, start() falla con mensaje.
        let mut s = SourceCore::phone_server(47999, 4321);
        assert_eq!(s.pairing_code(), 4321);
        assert!(s.endpoint().ends_with(":47999"), "{}", s.endpoint());
        if s.start() {
            assert!(s.devices().is_empty());
            let mut v = Vec::new();
            assert_eq!(s.poll_samples(10, &mut v), 0);
        } else {
            assert!(!s.last_error().is_empty());
        }
        s.stop();
        assert!(EngineCore::now_ns() > 0);
    }
}
