//! jd_source_* (contracts/engine_api.h). AGENTE A.
//!
//! `JD_Source*` apunta a un `SourceHandle` en el heap. Todas las funciones aceptan punteros nulos
//! (devuelven error/0 sin hacer nada). La fuente vive en `Arc<Mutex<..>>` para que un recorder pueda
//! envolverla sin invalidar el puntero original: tras `jd_source_create_recorder(inner, ..)` ambos
//! punteros siguen siendo válidos y se destruyen por separado, pero solo se debe hacer poll del recorder
//! (las muestras que saque `inner` directamente no pasan por la grabación).
use jd_core::phone::{PhoneServerHandle, PhoneServerSource, DEFAULT_PORT};
use jd_core::recorder::RecorderSource;
use jd_core::replay::ReplaySource;
use jd_core::types::{DeviceInfo, Limb, MotionSample, MotionSource};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

/// Idéntico a JD_DeviceInfoC (contracts/engine_api.h). 96 bytes.
#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct JD_DeviceInfoC {
    pub source_id: u32,
    pub name: [c_char; 64],
    pub kind: [c_char; 16],
    pub limb: u8,
    pub connected: u8,
    pub battery: f32,
    pub sync_rtt_ms: f32,
}

type Shared = Arc<Mutex<dyn MotionSource>>;

/// Opaco para C (`typedef struct JD_Source JD_Source;`).
#[allow(non_camel_case_types)]
pub struct JD_Source {
    src: Shared,
    recorder: Option<Arc<Mutex<RecorderSource>>>,
    phone: Option<PhoneServerHandle>,
    /// Muestras ya sacadas de la fuente que no cupieron en el último poll.
    backlog: Vec<MotionSample>,
    error: String,
    error_c: CString,
}

/// Adaptador: un `MotionSource` que delega en una fuente compartida (para envolver en el recorder).
struct SharedSource(Shared);

impl MotionSource for SharedSource {
    fn start(&mut self) -> Result<(), String> { lock(&self.0).start() }
    fn stop(&mut self) { lock(&self.0).stop() }
    fn poll(&mut self, out: &mut Vec<MotionSample>) -> usize { lock(&self.0).poll(out) }
    fn devices(&self) -> Vec<DeviceInfo> { lock(&self.0).devices() }
    fn assign_limb(&mut self, source_id: u32, limb: Limb) { lock(&self.0).assign_limb(source_id, limb) }
    fn last_error(&self) -> String { lock(&self.0).last_error() }
}

fn lock<T: ?Sized>(m: &Mutex<T>) -> MutexGuard<'_, T> { m.lock().unwrap_or_else(|e| e.into_inner()) }

fn new_handle(src: Shared, recorder: Option<Arc<Mutex<RecorderSource>>>, phone: Option<PhoneServerHandle>) -> *mut JD_Source {
    Box::into_raw(Box::new(JD_Source { src, recorder, phone, backlog: Vec::new(), error: String::new(), error_c: CString::default() }))
}

unsafe fn handle<'a>(s: *mut JD_Source) -> Option<&'a mut JD_Source> { s.as_mut() }

unsafe fn path_arg(p: *const c_char) -> Option<String> {
    if p.is_null() { return None; }
    CStr::from_ptr(p).to_str().ok().map(|s| s.to_string())
}

/// Trunca en un límite de carácter para que el búfer C siga siendo UTF-8 válido.
fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes { return s; }
    let mut n = max_bytes;
    while n > 0 && !s.is_char_boundary(n) { n -= 1; }
    &s[..n]
}

unsafe fn write_fixed(s: &str, out: &mut [c_char]) {
    let t = truncate_utf8(s, out.len() - 1);
    crate::write_c_string(t, out.as_mut_ptr(), out.len() as i32);
}

// ------------------------------------------------------------------ creación

#[no_mangle]
pub extern "C" fn jd_source_create_phone_server(port: u16, pairing_code: u32) -> *mut JD_Source {
    let port = if port == 0 { DEFAULT_PORT } else { port };
    let server = PhoneServerSource::new(port, pairing_code);
    let phone = server.handle();
    let src: Shared = Arc::new(Mutex::new(server));
    new_handle(src, None, Some(phone))
}

#[no_mangle]
pub unsafe extern "C" fn jd_source_phone_endpoint(s: *mut JD_Source, out: *mut c_char, out_len: c_int) -> u32 {
    let h = match handle(s) { Some(h) => h, None => { crate::write_c_string("", out, out_len); return 0; } };
    match &h.phone {
        Some(p) => {
            let ep = p.endpoint();
            if out_len > 0 { crate::write_c_string(truncate_utf8(&ep, out_len as usize - 1), out, out_len); }
            p.pairing_code()
        }
        None => { crate::write_c_string("", out, out_len); 0 }
    }
}

#[no_mangle]
pub unsafe extern "C" fn jd_source_create_replay(session_path: *const c_char, speed: f64) -> *mut JD_Source {
    let p = match path_arg(session_path) { Some(p) => p, None => return std::ptr::null_mut() };
    match ReplaySource::from_path(Path::new(&p), speed) {
        Ok(r) => new_handle(Arc::new(Mutex::new(r)), None, None),
        Err(e) => {
            // Sin objeto donde dejar el error: se devuelve una fuente vacía con last_error relleno,
            // para que el juego pueda mostrar el motivo (jd_source_start devolverá error).
            let h = new_handle(Arc::new(Mutex::new(FailedSource(e.clone()))), None, None);
            (*h).error = e;
            h
        }
    }
}

/// Fuente que solo informa de un error de creación.
struct FailedSource(String);
impl MotionSource for FailedSource {
    fn start(&mut self) -> Result<(), String> { Err(self.0.clone()) }
    fn stop(&mut self) {}
    fn poll(&mut self, _out: &mut Vec<MotionSample>) -> usize { 0 }
    fn devices(&self) -> Vec<DeviceInfo> { Vec::new() }
    fn assign_limb(&mut self, _source_id: u32, _limb: Limb) {}
    fn last_error(&self) -> String { self.0.clone() }
}

#[no_mangle]
pub unsafe extern "C" fn jd_source_create_recorder(inner: *mut JD_Source, out_path: *const c_char) -> *mut JD_Source {
    let inner_h = match handle(inner) { Some(h) => h, None => return std::ptr::null_mut() };
    let p = match path_arg(out_path) { Some(p) => p, None => { inner_h.error = "ruta de grabación nula o no UTF-8".into(); return std::ptr::null_mut(); } };
    let wrapped = Box::new(SharedSource(inner_h.src.clone()));
    match RecorderSource::new(wrapped, Path::new(&p)) {
        Ok(r) => {
            let rec = Arc::new(Mutex::new(r));
            let src: Shared = rec.clone();
            new_handle(src, Some(rec), inner_h.phone.clone())
        }
        Err(e) => { inner_h.error = e; std::ptr::null_mut() }
    }
}

#[no_mangle]
pub unsafe extern "C" fn jd_source_recorder_set_mapping(rec: *mut JD_Source, mono_to_song_s: f64) {
    if let Some(h) = handle(rec) {
        if let Some(r) = &h.recorder { lock(r).set_mapping(mono_to_song_s); }
    }
}

// ------------------------------------------------------------------ uso común

#[no_mangle]
pub unsafe extern "C" fn jd_source_start(s: *mut JD_Source) -> c_int {
    let h = match handle(s) { Some(h) => h, None => return -1 };
    h.backlog.clear();
    match lock(&h.src).start() {
        Ok(()) => { h.error.clear(); 0 }
        Err(e) => { h.error = e; -2 }
    }
}

#[no_mangle]
pub unsafe extern "C" fn jd_source_stop(s: *mut JD_Source) {
    if let Some(h) = handle(s) { lock(&h.src).stop(); }
}

#[no_mangle]
pub unsafe extern "C" fn jd_source_poll(s: *mut JD_Source, out: *mut MotionSample, max: c_int) -> c_int {
    let h = match handle(s) { Some(h) => h, None => return -1 };
    if max <= 0 { return 0; }
    if out.is_null() { h.error = "jd_source_poll: búfer nulo".into(); return -1; }
    let max = max as usize;
    if h.backlog.len() < max { lock(&h.src).poll(&mut h.backlog); }
    let n = h.backlog.len().min(max);
    std::ptr::copy_nonoverlapping(h.backlog.as_ptr(), out, n);
    h.backlog.drain(..n);
    n as c_int
}

#[no_mangle]
pub unsafe extern "C" fn jd_source_device_count(s: *mut JD_Source) -> c_int {
    match handle(s) { Some(h) => lock(&h.src).devices().len() as c_int, None => 0 }
}

#[no_mangle]
pub unsafe extern "C" fn jd_source_device_info(s: *mut JD_Source, index: c_int, out: *mut JD_DeviceInfoC) -> c_int {
    let h = match handle(s) { Some(h) => h, None => return -1 };
    if out.is_null() { return -1; }
    let devs = lock(&h.src).devices();
    let d = match usize::try_from(index).ok().and_then(|i| devs.get(i)) { Some(d) => d, None => return -2 };
    let mut c = JD_DeviceInfoC {
        source_id: d.source_id, name: [0; 64], kind: [0; 16], limb: d.limb as u8,
        connected: d.connected as u8, battery: d.battery, sync_rtt_ms: d.sync_rtt_ms,
    };
    write_fixed(&d.name, &mut c.name);
    write_fixed(&d.kind, &mut c.kind);
    out.write(c);
    0
}

#[no_mangle]
pub unsafe extern "C" fn jd_source_assign_limb(s: *mut JD_Source, source_id: u32, limb: u8) {
    if let Some(h) = handle(s) { lock(&h.src).assign_limb(source_id, Limb::from_u8(limb)); }
}

/// Válido hasta la siguiente llamada sobre la misma fuente. Nunca nulo.
#[no_mangle]
pub unsafe extern "C" fn jd_source_last_error(s: *mut JD_Source) -> *const c_char {
    static EMPTY: &[u8] = b"\0";
    let h = match handle(s) { Some(h) => h, None => return EMPTY.as_ptr() as *const c_char };
    let msg = if h.error.is_empty() { lock(&h.src).last_error() } else { h.error.clone() };
    h.error_c = CString::new(msg.replace('\0', " ")).unwrap_or_default();
    h.error_c.as_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn jd_source_destroy(s: *mut JD_Source) {
    if s.is_null() { return; }
    // Al soltar el último Arc, Drop de cada fuente hace la limpieza: el servidor para su hilo y
    // despide a los móviles; el recorder cierra el archivo. Un recorder mantiene viva su fuente interior.
    drop(Box::from_raw(s));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn abi_layout_matches_header() {
        assert_eq!(size_of::<JD_DeviceInfoC>(), 96);
        assert_eq!(offset_of!(JD_DeviceInfoC, name), 4);
        assert_eq!(offset_of!(JD_DeviceInfoC, kind), 68);
        assert_eq!(offset_of!(JD_DeviceInfoC, limb), 84);
        assert_eq!(offset_of!(JD_DeviceInfoC, connected), 85);
        assert_eq!(offset_of!(JD_DeviceInfoC, battery), 88);
        assert_eq!(offset_of!(JD_DeviceInfoC, sync_rtt_ms), 92);
    }

    #[test]
    fn null_pointers_are_safe() {
        unsafe {
            let n = std::ptr::null_mut();
            assert_eq!(jd_source_start(n), -1);
            jd_source_stop(n);
            assert_eq!(jd_source_poll(n, std::ptr::null_mut(), 10), -1);
            assert_eq!(jd_source_device_count(n), 0);
            assert_eq!(jd_source_device_info(n, 0, std::ptr::null_mut()), -1);
            jd_source_assign_limb(n, 1, 1);
            assert_eq!(*jd_source_last_error(n), 0);
            jd_source_recorder_set_mapping(n, 1.0);
            assert_eq!(jd_source_phone_endpoint(n, std::ptr::null_mut(), 0), 0);
            assert!(jd_source_create_replay(std::ptr::null(), 1.0).is_null());
            assert!(jd_source_create_recorder(n, std::ptr::null()).is_null());
            jd_source_destroy(n);
        }
    }

    fn cstr(s: &str) -> CString { CString::new(s).unwrap() }

    #[test]
    fn replay_record_roundtrip_through_c_api() {
        let dir = std::env::temp_dir();
        let src_path = dir.join(format!("jd_capi_a_src_{}.jsonl", std::process::id()));
        let rec_path = dir.join(format!("jd_capi_a_rec_{}.jsonl", std::process::id()));
        let mut text = String::from("{\"format\":\"jd-session\",\"version\":1,\"time_base\":\"song\",\"devices\":[{\"source_id\":4,\"name\":\"Móvil con nombre larguísimo que no cabe en sesenta y cuatro bytes ñññ\",\"kind\":\"phone\",\"limb\":\"right_hand\"}]}\n");
        for k in 0..20 { text.push_str(&format!("{{\"t\":{},\"src\":4,\"limb\":\"right_hand\",\"a\":[{},9.8,0],\"g\":[0,0,1]}}\n", 1.0 + k as f64 * 0.005, k)); }
        std::fs::write(&src_path, text).unwrap();
        unsafe {
            let bad = jd_source_create_replay(cstr("/no/existe.jsonl").as_ptr(), 1.0);
            assert!(!bad.is_null());
            assert_eq!(jd_source_start(bad), -2);
            let e = CStr::from_ptr(jd_source_last_error(bad)).to_str().unwrap().to_string();
            assert!(e.contains("/no/existe.jsonl"), "{}", e);
            jd_source_destroy(bad);

            let rp = jd_source_create_replay(cstr(src_path.to_str().unwrap()).as_ptr(), 100.0);
            assert!(!rp.is_null());
            let rec = jd_source_create_recorder(rp, cstr(rec_path.to_str().unwrap()).as_ptr());
            assert!(!rec.is_null());
            jd_source_recorder_set_mapping(rec, 0.0);
            assert_eq!(jd_source_device_count(rec), 1);
            let mut info: JD_DeviceInfoC = std::mem::zeroed();
            assert_eq!(jd_source_device_info(rec, 0, &mut info), 0);
            assert_eq!(jd_source_device_info(rec, 1, &mut info), -2);
            let name = CStr::from_ptr(info.name.as_ptr()).to_str().expect("UTF-8 válido tras truncar");
            assert!(name.len() <= 63 && name.starts_with("Móvil"));
            assert_eq!(CStr::from_ptr(info.kind.as_ptr()).to_str().unwrap(), "replay");
            assert_eq!(info.sync_rtt_ms, -1.0);
            jd_source_destroy(rp); // el recorder la mantiene viva
            assert_eq!(jd_source_start(rec), 0);
            let mut buf = [MotionSample::default(); 7];
            let mut total = Vec::new();
            let end = jd_core::clock::now_ns() + 2_000_000_000;
            while total.len() < 20 && jd_core::clock::now_ns() < end {
                let n = jd_source_poll(rec, buf.as_mut_ptr(), 7);
                assert!((0..=7).contains(&n));
                total.extend_from_slice(&buf[..n as usize]);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert_eq!(total.len(), 20);
            jd_source_stop(rec);
            jd_source_destroy(rec);
        }
        let back = jd_core::session::load_session(&rec_path).unwrap();
        assert_eq!(back.samples.len(), 20);
        assert_eq!(back.time_base, "song");
        let _ = std::fs::remove_file(src_path);
        let _ = std::fs::remove_file(rec_path);
    }

    #[test]
    fn phone_server_endpoint() {
        unsafe {
            let s = jd_source_create_phone_server(0, 0);
            assert!(!s.is_null());
            let mut buf = [0 as c_char; 64];
            let code = jd_source_phone_endpoint(s, buf.as_mut_ptr(), 64);
            assert!((1000..=9999).contains(&code));
            let ep = CStr::from_ptr(buf.as_ptr()).to_str().unwrap();
            assert!(ep.ends_with(":47800"), "{}", ep);
            let mut small = [0 as c_char; 4];
            jd_source_phone_endpoint(s, small.as_mut_ptr(), 4);
            assert_eq!(CStr::from_ptr(small.as_ptr()).to_bytes().len(), 3);
            assert_eq!(jd_source_device_count(s), 0);
            jd_source_destroy(s);
        }
    }
}
