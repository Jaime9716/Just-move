//! CONTRATO de tipos y traits entre bloques del motor (solo lo cambia el arquitecto).
//! Equivale a contracts/motion_sample.h + engine_api.h para el lado Rust.

use std::fmt;

/// Extremidad (valores = JD_Limb).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Limb {
    LeftHand = 0,
    RightHand = 1,
    LeftFoot = 2,
    RightFoot = 3,
    None = 255,
}

impl Limb {
    pub const ALL: [Limb; 4] = [Limb::LeftHand, Limb::RightHand, Limb::LeftFoot, Limb::RightFoot];
    pub fn from_u8(v: u8) -> Limb {
        match v { 0 => Limb::LeftHand, 1 => Limb::RightHand, 2 => Limb::LeftFoot, 3 => Limb::RightFoot, _ => Limb::None }
    }
    /// Nombre en archivos JSON.
    pub fn name(self) -> &'static str {
        match self {
            Limb::LeftHand => "left_hand", Limb::RightHand => "right_hand",
            Limb::LeftFoot => "left_foot", Limb::RightFoot => "right_foot", Limb::None => "none",
        }
    }
    pub fn from_name(s: &str) -> Limb {
        match s {
            "left_hand" => Limb::LeftHand, "right_hand" => Limb::RightHand,
            "left_foot" => Limb::LeftFoot, "right_foot" => Limb::RightFoot, _ => Limb::None,
        }
    }
    pub fn is_foot(self) -> bool { matches!(self, Limb::LeftFoot | Limb::RightFoot) }
}

impl fmt::Display for Limb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.name()) }
}

pub const SAMPLE_HAS_ACCEL: u8 = 1 << 0;
pub const SAMPLE_HAS_GYRO: u8 = 1 << 1;

/// Idéntico a JD_MotionSample (contracts/motion_sample.h). 40 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct MotionSample {
    pub t_ns: i64,
    pub source_id: u32,
    pub limb: u8,
    pub flags: u8,
    pub _reserved: u16,
    pub accel: [f32; 3],
    pub gyro: [f32; 3],
}

impl MotionSample {
    pub fn limb(&self) -> Limb { Limb::from_u8(self.limb) }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceInfo {
    pub source_id: u32,
    pub name: String,
    pub kind: String,      // "phone", "replay", "fake"...
    pub limb: Limb,
    pub connected: bool,
    pub battery: f32,      // 0..1, -1 desconocido
    pub sync_rtt_ms: f32,  // -1 si no aplica
}

/// Interfaz común de fuentes (B1) y reproducción (B7).
pub trait MotionSource: Send {
    fn start(&mut self) -> Result<(), String>;
    fn stop(&mut self);
    /// Añade a `out` las muestras nuevas (t_ns creciente por dispositivo). Devuelve cuántas añadió.
    fn poll(&mut self, out: &mut Vec<MotionSample>) -> usize;
    fn devices(&self) -> Vec<DeviceInfo>;
    fn assign_limb(&mut self, source_id: u32, limb: Limb);
    fn last_error(&self) -> String { String::new() }
}

// ------------------------------------------------------------------ B3 · Señal
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilteredSample {
    pub t_song: f64,
    pub limb: Limb,
    pub accel: [f32; 3],
    pub lin_accel: [f32; 3],
    pub gyro: [f32; 3],
    pub quat: [f32; 4], // w, x, y, z (mundo <- sensor), mundo Y arriba
}

pub trait SignalProcessor: Send {
    fn process(&mut self, input: &[MotionSample], mono_to_song_s: f64, out: &mut Vec<FilteredSample>);
    fn reset(&mut self);
}

// ------------------------------------------------------------------ B4 · Rasgos
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureFrame {
    pub t_song: f64,
    pub limb: Limb,
    pub energy: f32,
    pub peak: f32,
    pub is_peak: bool,
    pub peak_t_song: f64,
    pub dir: [f32; 3],
    pub quat: [f32; 4],
}

pub trait FeatureExtractor: Send {
    fn push(&mut self, input: &[FilteredSample], out: &mut Vec<FeatureFrame>);
    /// Emite las tramas pendientes (fin de datos).
    fn flush(&mut self, out: &mut Vec<FeatureFrame>);
    fn reset(&mut self);
}

// ------------------------------------------------------------------ B6 · Coreografía
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind { Accent, Move }

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub t: f64,
    pub dur: f64,
    pub limbs: Vec<Limb>,
    pub icon: String,
    pub kind: StepKind,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Choreo {
    pub title: String,
    pub artist: String,
    pub audio: String,
    pub video: String,   // vacío si no hay
    pub bpm: f64,
    pub offset_s: f64,
    pub duration_s: f64,
    pub steps: Vec<Step>,
    pub ref_imu: String, // ruta relativa, vacía si no hay
}

// ------------------------------------------------------------------ B7 · Sesiones
#[derive(Clone, Debug, PartialEq)]
pub struct SessionDevice { pub source_id: u32, pub name: String, pub kind: String, pub limb: Limb }

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Session {
    pub time_base: String, // "song" | "mono"
    pub song_id: String,
    pub recorded_at: String,
    pub note: String,
    pub devices: Vec<SessionDevice>,
    pub samples: Vec<MotionSample>, // t_ns = round(t * 1e9)
}

// ------------------------------------------------------------------ B5 · Juez
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grade { Miss = 0, Good = 1, Perfect = 2 }

/// Idéntico a JD_JudgeConfig.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JudgeConfig {
    pub perfect_window_ms: f32,
    pub good_window_ms: f32,
    pub foot_window_scale: f32,
}
impl Default for JudgeConfig {
    fn default() -> Self { JudgeConfig { perfect_window_ms: 50.0, good_window_ms: 100.0, foot_window_scale: 1.5 } }
}

/// Idéntico a JD_StepResult.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepResult {
    pub step_index: i32,
    pub limb: u8,
    pub grade: u8,
    pub _reserved: u16,
    pub similarity: f32,
    pub timing_error_ms: f32,
}

pub trait Judge: Send {
    fn load(&mut self, choreo: &Choreo, reference_frames: &[FeatureFrame], cfg: &JudgeConfig);
    fn push(&mut self, player_frames: &[FeatureFrame]);
    /// Resultados de pasos cuya ventana ya se cerró en song_time_s (orden del contrato).
    fn update(&mut self, song_time_s: f64, out: &mut Vec<StepResult>);
    fn reset(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_sizes_match_c_header() {
        assert_eq!(std::mem::size_of::<MotionSample>(), 40);
        assert_eq!(std::mem::size_of::<StepResult>(), 16);
        assert_eq!(std::mem::size_of::<JudgeConfig>(), 12);
    }
    #[test]
    fn limb_names() {
        for l in Limb::ALL { assert_eq!(Limb::from_name(l.name()), l); }
        assert_eq!(Limb::from_name("x"), Limb::None);
    }
}
