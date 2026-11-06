//! B6 · Coreografía: carga y validación de choreo.json (contracts/song_package.md). AGENTE F.
use crate::types::Choreo;
use std::path::Path;

/// Errores con ruta JSON: "steps[3].t: debe ser >= steps[2].t".
pub fn load_choreo(path: &Path) -> Result<Choreo, String> {
    let _ = path;
    Err("load_choreo: sin implementar (agente F)".into())
}

pub fn validate_choreo(c: &Choreo) -> Result<(), String> {
    let _ = c;
    Err("validate_choreo: sin implementar (agente F)".into())
}
