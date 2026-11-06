//! jd_engine_* (contracts/engine_api.h). AGENTE C.
//!
//! Todas las funciones aceptan punteros nulos sin fallar (no hacen nada / devuelven error).
//! Los pánicos internos no cruzan la frontera C (se capturan y se informan como error).
use crate::write_c_string;
use jd_core::engine::Engine;
use jd_core::types::{JudgeConfig, MotionSample, StepResult};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

/// Tipo opaco JD_Engine.
pub struct JdEngine {
    engine: Engine,
    results: Vec<StepResult>, // resultados ya calculados que no cupieron en `out`
}

pub const JD_GRADE_MISS: c_int = 0;
pub const JD_GRADE_GOOD: c_int = 1;
pub const JD_GRADE_PERFECT: c_int = 2;

#[no_mangle]
pub extern "C" fn jd_engine_create() -> *mut JdEngine {
    match catch_unwind(|| Box::new(JdEngine { engine: Engine::new(), results: Vec::new() })) {
        Ok(b) => Box::into_raw(b),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn jd_engine_destroy(e: *mut JdEngine) {
    if e.is_null() { return; }
    drop(Box::from_raw(e));
}

#[no_mangle]
pub unsafe extern "C" fn jd_engine_default_config(out: *mut JudgeConfig) {
    if out.is_null() { return; }
    *out = JudgeConfig::default();
}

#[no_mangle]
pub unsafe extern "C" fn jd_engine_set_config(e: *mut JdEngine, cfg: *const JudgeConfig) {
    let (Some(e), Some(cfg)) = (e.as_mut(), cfg.as_ref()) else { return };
    let cfg = *cfg;
    let _ = catch_unwind(AssertUnwindSafe(|| {
        e.engine.set_config(cfg);
        e.results.clear();
    }));
}

#[no_mangle]
pub unsafe extern "C" fn jd_engine_load_song(e: *mut JdEngine, song_dir: *const c_char, err: *mut c_char, err_len: c_int) -> c_int {
    let Some(e) = e.as_mut() else {
        write_c_string("motor nulo", err, err_len);
        return -1;
    };
    if song_dir.is_null() {
        write_c_string("song_dir nulo", err, err_len);
        return -1;
    }
    let dir = match CStr::from_ptr(song_dir).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            write_c_string("song_dir no es UTF-8 válido", err, err_len);
            return -1;
        }
    };
    let r = catch_unwind(AssertUnwindSafe(|| e.engine.load_song(Path::new(&dir))));
    match r {
        Ok(Ok(())) => {
            e.results.clear();
            write_c_string("", err, err_len);
            0
        }
        Ok(Err(msg)) => {
            write_c_string(&msg, err, err_len);
            -2
        }
        Err(_) => {
            write_c_string("error interno del motor al cargar la canción", err, err_len);
            -3
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn jd_engine_set_time_mapping(e: *mut JdEngine, mono_to_song_s: f64) {
    if let Some(e) = e.as_mut() { e.engine.set_time_mapping(mono_to_song_s); }
}

#[no_mangle]
pub unsafe extern "C" fn jd_engine_push_samples(e: *mut JdEngine, s: *const MotionSample, count: c_int) {
    let Some(e) = e.as_mut() else { return };
    if s.is_null() || count <= 0 { return; }
    let slice = std::slice::from_raw_parts(s, count as usize);
    let _ = catch_unwind(AssertUnwindSafe(|| e.engine.push_samples(slice)));
}

#[no_mangle]
pub unsafe extern "C" fn jd_engine_update(e: *mut JdEngine, song_time_s: f64, out: *mut StepResult, max_out: c_int) -> c_int {
    let Some(e) = e.as_mut() else { return 0 };
    let r = catch_unwind(AssertUnwindSafe(|| {
        let mut fresh = Vec::new();
        e.engine.update(song_time_s, &mut fresh);
        e.results.extend(fresh);
    }));
    if r.is_err() { return -1; }
    if out.is_null() || max_out <= 0 { return 0; }
    // Lo que no cabe se guarda para la siguiente llamada (no se pierde ningún resultado).
    let n = e.results.len().min(max_out as usize);
    std::ptr::copy_nonoverlapping(e.results.as_ptr(), out, n);
    e.results.drain(..n);
    n as c_int
}

#[no_mangle]
pub unsafe extern "C" fn jd_engine_reset(e: *mut JdEngine) {
    let Some(e) = e.as_mut() else { return };
    let _ = catch_unwind(AssertUnwindSafe(|| e.engine.reset()));
    e.results.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_pointers_are_safe() {
        unsafe {
            jd_engine_destroy(std::ptr::null_mut());
            jd_engine_default_config(std::ptr::null_mut());
            jd_engine_set_config(std::ptr::null_mut(), std::ptr::null());
            jd_engine_set_time_mapping(std::ptr::null_mut(), 1.0);
            jd_engine_push_samples(std::ptr::null_mut(), std::ptr::null(), 5);
            assert_eq!(jd_engine_update(std::ptr::null_mut(), 1.0, std::ptr::null_mut(), 4), 0);
            jd_engine_reset(std::ptr::null_mut());
            let mut buf = [0 as c_char; 32];
            assert!(jd_engine_load_song(std::ptr::null_mut(), std::ptr::null(), buf.as_mut_ptr(), 32) < 0);
            let e = jd_engine_create();
            assert!(!e.is_null());
            assert!(jd_engine_load_song(e, std::ptr::null(), std::ptr::null_mut(), 0) < 0);
            let dir = std::ffi::CString::new("/no/existe").unwrap();
            assert!(jd_engine_load_song(e, dir.as_ptr(), buf.as_mut_ptr(), 32) < 0);
            assert!(buf[0] != 0, "mensaje de error escrito");
            jd_engine_push_samples(e, std::ptr::null(), 3);
            assert_eq!(jd_engine_update(e, 10.0, std::ptr::null_mut(), 0), 0);
            jd_engine_reset(e);
            jd_engine_destroy(e);
        }
    }

    #[test]
    fn config_roundtrip() {
        unsafe {
            let mut c = JudgeConfig { perfect_window_ms: 0.0, good_window_ms: 0.0, foot_window_scale: 0.0 };
            jd_engine_default_config(&mut c);
            assert_eq!(c, JudgeConfig::default());
            let e = jd_engine_create();
            c.good_window_ms = 120.0;
            jd_engine_set_config(e, &c);
            assert_eq!((*e).engine.config().good_window_ms, 120.0);
            jd_engine_destroy(e);
        }
    }

    #[test]
    fn end_to_end_demo_song() {
        let syn = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/sessions/synthetic");
        if !syn.join("song_demo/choreo.json").is_file() { return; }
        let sess = match jd_core::session::load_session(&syn.join("perfect.jsonl")) { Ok(s) => s, Err(_) => return };
        let mut samples = sess.samples.clone();
        samples.sort_by_key(|s| s.t_ns);
        unsafe {
            let e = jd_engine_create();
            let dir = std::ffi::CString::new(syn.join("song_demo").to_string_lossy().as_bytes()).unwrap();
            let mut err = [0 as c_char; 256];
            assert_eq!(jd_engine_load_song(e, dir.as_ptr(), err.as_mut_ptr(), 256), 0);
            jd_engine_set_time_mapping(e, 0.0);
            let mut buf = [StepResult { step_index: 0, limb: 0, grade: 0, _reserved: 0, similarity: 0.0, timing_error_ms: 0.0 }; 2];
            let mut got = Vec::new();
            let (mut idx, mut t) = (0usize, 0.0f64);
            while t < 40.0 {
                t += 0.016;
                let s0 = idx;
                while idx < samples.len() && samples[idx].t_ns as f64 * 1e-9 <= t { idx += 1; }
                jd_engine_push_samples(e, samples[s0..idx].as_ptr(), (idx - s0) as c_int);
                // max_out = 2: lo que no cabe sale en la llamada siguiente.
                let n = jd_engine_update(e, t, buf.as_mut_ptr(), 2);
                got.extend_from_slice(&buf[..n as usize]);
            }
            let n_steps: usize = (*e).engine.choreo().unwrap().steps.iter().map(|s| s.limbs.len()).sum();
            assert_eq!(got.len(), n_steps);
            assert!(got.iter().filter(|r| r.grade == JD_GRADE_PERFECT as u8).count() * 10 >= got.len() * 8);
            jd_engine_reset(e);
            assert_eq!(jd_engine_update(e, 1000.0, buf.as_mut_ptr(), 2), 2);
            jd_engine_destroy(e);
        }
    }
}
