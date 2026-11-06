//! jd_core — motor de movimiento del Juego de Baile (sin dependencias externas).
//!
//! Propiedad de módulos (ver AGENTS.md):
//!   types, json, clock ............ arquitecto (contratos, solo lectura)
//!   session, replay, recorder, phone  agente A (B1 fuente móvil, B7 sesiones)
//!   signal, features, judge, engine . agente C (B3, B4, B5 y motor de alto nivel)
//!   choreo ........................ agente F (B6)

pub mod clock;
pub mod json;
pub mod types;

pub mod choreo;
pub mod engine;
pub mod features;
pub mod judge;
pub mod phone;
pub mod recorder;
pub mod replay;
pub mod session;
pub mod signal;

pub use types::*;
