//! Motor de alto nivel: señal -> rasgos -> juez para una canción. AGENTE C.
//! Lo usan jd_capi (API C), jd_tools y la GDExtension.
use crate::types::{JudgeConfig, MotionSample, StepResult};
use std::path::Path;

pub struct Engine {
    _private: (),
}

impl Engine {
    pub fn new() -> Engine { Engine { _private: () } }
    pub fn set_config(&mut self, _cfg: JudgeConfig) {}
    pub fn config(&self) -> JudgeConfig { JudgeConfig::default() }
    /// Carga <dir>/choreo.json y, si existe, la referencia IMU (rasgos precalculados).
    pub fn load_song(&mut self, _song_dir: &Path) -> Result<(), String> { Err("sin implementar (agente C)".into()) }
    pub fn set_time_mapping(&mut self, _mono_to_song_s: f64) {}
    pub fn push_samples(&mut self, _s: &[MotionSample]) {}
    /// Resultados de pasos cuya ventana se cerró hasta song_time_s.
    pub fn update(&mut self, _song_time_s: f64, _out: &mut Vec<StepResult>) {}
    pub fn reset(&mut self) {}
}

impl Default for Engine { fn default() -> Self { Self::new() } }
