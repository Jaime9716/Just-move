//! API C del motor (contracts/engine_api.h es la fuente de verdad). Arquitecto.
//! sources.rs -> agente A · engine.rs -> agente C.
#![allow(clippy::missing_safety_doc)]

pub mod engine;
pub mod sources;

pub const JD_ENGINE_API_VERSION: i32 = 1;

#[no_mangle]
pub extern "C" fn jd_api_version() -> i32 { JD_ENGINE_API_VERSION }

#[no_mangle]
pub extern "C" fn jd_now_ns() -> i64 { jd_core::clock::now_ns() }

/// Copia una cadena UTF-8 a un búfer C terminado en 0 (trunca). Utilidad común.
pub(crate) unsafe fn write_c_string(s: &str, out: *mut std::os::raw::c_char, len: i32) {
    if out.is_null() || len <= 0 { return; }
    let bytes = s.as_bytes();
    let n = bytes.len().min(len as usize - 1);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, n);
    *out.add(n) = 0;
}
