//! B1 · Fuente móvil: protocolo UDP, sincronización de reloj y servidor
//! (contracts/phone_protocol.md). AGENTE A.
//!
//! - `protocol`: codificación/decodificación de mensajes (sin pánicos con datos basura).
//! - `clock_sync`: estimador de offset/deriva móvil ↔ PC.
//! - `server`: `PhoneServerSource` (MotionSource) con hilo UDP propio.
//! - `sim`: simulador de móvil (bin `phone_sim` y tests de integración).

pub mod clock_sync;
pub mod protocol;
pub mod server;
pub mod sim;

pub use clock_sync::ClockSync;
pub use protocol::DEFAULT_PORT;
pub use server::{detect_lan_ip, PhoneDeviceStats, PhoneServerHandle, PhoneServerSource};
pub use sim::{PhoneSim, SimConfig};
