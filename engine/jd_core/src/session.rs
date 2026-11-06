//! B7 · Sesiones: lectura/escritura JSONL (contracts/session_format.md). AGENTE A.
use crate::types::Session;
use std::path::Path;

/// Error legible con número de línea ("sesion.jsonl:12: ...").
pub fn load_session(path: &Path) -> Result<Session, String> {
    let _ = path;
    Err("load_session: sin implementar (agente A)".into())
}

pub fn save_session(path: &Path, s: &Session) -> Result<(), String> {
    let _ = (path, s);
    Err("save_session: sin implementar (agente A)".into())
}
