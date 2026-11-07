//! jd_godot — GDExtension del Juego de Baile (agente G, bloque B10). contracts/gdextension_api.md.
//!
//! Clases `JDSource` y `JDEngine` (RefCounted) para GDScript, hechas con godot-rust (gdext 0.3, API de
//! Godot 4.4). Usa `jd_core` directamente; toda la lógica está en `bridge.rs` (probada sin Godot), aquí
//! solo se convierten tipos.
//!
//! NO VERIFICADO EN EL ENTORNO DE DESARROLLO (sin red, sin la crate `godot`): esto compila por primera
//! vez en GitHub Actions. Ver docs/agents/G-report.md, sección "No verificado".
//!
//! Notas de API:
//! - Las funciones `#[func]` sin `self` se registran como métodos estáticos de la clase.
//! - Los valores por defecto del contrato (`port = 47800`, `speed = 1.0`, `max = 512`) NO se declaran en
//!   Godot (gdext 0.3 no tiene parámetros opcionales en `#[func]`): GDScript debe pasar todos los
//!   argumentos. 0 / valores no válidos se traducen al valor por defecto (port 0 -> 47800,
//!   speed <= 0 -> 1.0, max <= 0 -> 512).
//! - Las rutas `res://` y `user://` se convierten a rutas del sistema con `ProjectSettings.globalize_path`.

use godot::prelude::*;
use godot::classes::ProjectSettings;

#[allow(dead_code)]
mod bridge;

use bridge::{EngineCore, SourceCore};

struct JdExtension;

/// Punto de entrada: símbolo `gdext_rust_init` (game/jd_engine.gdextension).
#[gdextension]
unsafe impl ExtensionLibrary for JdExtension {}

const DEFAULT_POLL_MAX: i64 = 512;

/// `res://...` / `user://...` -> ruta absoluta; el resto se deja igual.
fn os_path(path: &GString) -> String {
    let s = path.to_string();
    if s.starts_with("res://") || s.starts_with("user://") {
        ProjectSettings::singleton().globalize_path(path).to_string()
    } else {
        s
    }
}

fn key(k: &str) -> Variant { GString::from(k).to_variant() }

fn put<V: ToGodot>(d: &mut Dictionary, k: &str, v: V) { d.set(key(k), v.to_variant()); }

fn vec3(v: [f32; 3]) -> Vector3 { Vector3::new(v[0], v[1], v[2]) }

// ====================================================================== JDSource

/// Fuente de movimiento: servidor de móviles-mando, reproducción de sesión o grabador.
/// Se crea con los métodos estáticos `create_*`; `JDSource.new()` da una fuente vacía que no hace nada.
#[derive(GodotClass)]
#[class(base = RefCounted, init)]
pub struct JDSource {
    core: Option<SourceCore>,
    /// Error de la creación o de una llamada sobre una fuente vacía.
    error: String,
}

impl JDSource {
    fn wrap(core: SourceCore) -> Gd<JDSource> {
        Gd::from_object(JDSource { core: Some(core), error: String::new() })
    }
}

#[godot_api]
impl JDSource {
    /// Servidor de móviles-mando (contracts/phone_protocol.md). `pairing_code` 0 = aleatorio.
    #[func]
    fn create_phone_server(port: i64, pairing_code: i64) -> Gd<JDSource> {
        JDSource::wrap(SourceCore::phone_server(port, pairing_code))
    }

    /// Reproduce una sesión grabada; null si el archivo no existe o no se puede leer.
    #[func]
    fn create_replay(path: GString, speed: f64) -> Option<Gd<JDSource>> {
        match SourceCore::replay(&os_path(&path), speed) {
            Ok(core) => Some(JDSource::wrap(core)),
            Err(e) => {
                godot_warn!("JDSource.create_replay: {}", e);
                None
            }
        }
    }

    /// Envuelve `inner` y graba en `out_path` (JSONL) todo lo que pasa por poll/pump.
    /// Después, leer SOLO del grabador. null si `inner` no es válido o no se puede crear el archivo.
    #[func]
    fn create_recorder(inner: Option<Gd<JDSource>>, out_path: GString) -> Option<Gd<JDSource>> {
        let inner = match inner {
            Some(g) => g,
            None => {
                godot_warn!("JDSource.create_recorder: inner es null");
                return None;
            }
        };
        let path = os_path(&out_path);
        let result = {
            let b = inner.bind();
            match &b.core {
                Some(core) => SourceCore::recorder(core, &path),
                None => Err("inner es una fuente vacía".to_string()),
            }
        };
        match result {
            Ok(core) => Some(JDSource::wrap(core)),
            Err(e) => {
                godot_warn!("JDSource.create_recorder: {}", e);
                None
            }
        }
    }

    /// "IP:puerto" de la LAN ("" si no es un servidor de móviles).
    #[func]
    fn endpoint(&self) -> GString {
        GString::from(self.core.as_ref().map(|c| c.endpoint()).unwrap_or_default().as_str())
    }

    /// Código de emparejado de 4 dígitos (0 si no es un servidor de móviles).
    #[func]
    fn pairing_code(&self) -> i64 {
        self.core.as_ref().map(|c| c.pairing_code() as i64).unwrap_or(0)
    }

    #[func]
    fn start(&mut self) -> bool {
        match self.core.as_mut() {
            Some(c) => c.start(),
            None => {
                self.error = "fuente vacía (usa JDSource.create_*)".to_string();
                false
            }
        }
    }

    #[func]
    fn stop(&mut self) {
        if let Some(c) = self.core.as_mut() { c.stop(); }
    }

    /// `{id, name, kind, limb, connected, battery, sync_rtt_ms}` por dispositivo.
    #[func]
    fn devices(&self) -> Array<Dictionary> {
        let list = self.core.as_ref().map(|c| c.devices()).unwrap_or_default();
        list.into_iter()
            .map(|d| {
                let mut dict = Dictionary::new();
                put(&mut dict, "id", d.source_id as i64);
                put(&mut dict, "name", GString::from(d.name.as_str()));
                put(&mut dict, "kind", GString::from(d.kind.as_str()));
                put(&mut dict, "limb", d.limb as u8 as i64);
                put(&mut dict, "connected", d.connected);
                put(&mut dict, "battery", d.battery as f64);
                put(&mut dict, "sync_rtt_ms", d.sync_rtt_ms as f64);
                dict
            })
            .collect()
    }

    /// limb = JD_Limb (0 mano izq, 1 mano der, 2 pie izq, 3 pie der; otro = ninguna).
    #[func]
    fn assign_limb(&mut self, id: i64, limb: i64) {
        if let Some(c) = self.core.as_mut() { c.assign_limb(id, limb); }
    }

    /// Solo grabadores: `song_time = t_ns·1e-9 + mono_to_song_s` (llamar cada fotograma).
    #[func]
    fn set_recorder_mapping(&mut self, mono_to_song_s: f64) {
        let ok = self.core.as_mut().map(|c| c.set_recorder_mapping(mono_to_song_s)).unwrap_or(false);
        if !ok { self.error = "set_recorder_mapping: la fuente no es un grabador".to_string(); }
    }

    /// Consume hasta `max` muestras: `{t_ns, id, limb, a: Vector3, g: Vector3}`.
    /// Para calibración y depuración; no usar a la vez que `JDEngine.pump`.
    #[func]
    fn poll_samples(&mut self, max: i64) -> Array<Dictionary> {
        let max = if max <= 0 { DEFAULT_POLL_MAX } else { max } as usize;
        let mut buf = Vec::new();
        if let Some(c) = self.core.as_mut() { c.poll_samples(max, &mut buf); }
        buf.into_iter()
            .map(|s| {
                let mut dict = Dictionary::new();
                put(&mut dict, "t_ns", s.t_ns);
                put(&mut dict, "id", s.source_id as i64);
                put(&mut dict, "limb", s.limb as i64);
                put(&mut dict, "a", vec3(s.accel));
                put(&mut dict, "g", vec3(s.gyro));
                dict
            })
            .collect()
    }

    #[func]
    fn last_error(&self) -> GString {
        let e = match self.core.as_ref() {
            Some(c) => {
                let e = c.last_error();
                if e.is_empty() { self.error.clone() } else { e }
            }
            None => self.error.clone(),
        };
        GString::from(e.as_str())
    }
}

// ====================================================================== JDEngine

/// Motor de puntuación: señal -> rasgos -> juez (jd_core::engine::Engine).
#[derive(GodotClass)]
#[class(base = RefCounted, init)]
pub struct JDEngine {
    core: EngineCore,
}

#[godot_api]
impl JDEngine {
    /// `jd_now_ns()`: el reloj monotónico oficial (el mismo que marca las muestras).
    #[func]
    fn now_ns() -> i64 { EngineCore::now_ns() }

    /// "" si OK; si no, el mensaje de error.
    #[func]
    fn load_song(&mut self, song_dir: GString) -> GString {
        let e = self.core.load_song(&os_path(&song_dir));
        GString::from(e.as_str())
    }

    #[func]
    fn set_config(&mut self, perfect_ms: f64, good_ms: f64, foot_scale: f64) {
        self.core.set_config(perfect_ms, good_ms, foot_scale);
    }

    /// Cada fotograma, desde SongClock.
    #[func]
    fn set_time_mapping(&mut self, mono_to_song_s: f64) {
        self.core.set_time_mapping(mono_to_song_s);
    }

    /// Lee todas las muestras de `source` y las pasa al motor (sin pasar por GDScript).
    #[func]
    fn pump(&mut self, source: Option<Gd<JDSource>>) -> i64 {
        let mut source = match source {
            Some(s) => s,
            None => return 0,
        };
        let mut b = source.bind_mut();
        match b.core.as_mut() {
            Some(c) => self.core.pump(c) as i64,
            None => 0,
        }
    }

    /// `{step, limb, grade, similarity, timing_error_ms}` de los pasos cuya ventana se cerró.
    /// grade 0 Miss, 1 Good, 2 Perfect.
    #[func]
    fn update(&mut self, song_time_s: f64) -> Array<Dictionary> {
        self.core
            .update(song_time_s)
            .iter()
            .map(|r| {
                let mut dict = Dictionary::new();
                put(&mut dict, "step", r.step_index as i64);
                put(&mut dict, "limb", r.limb as i64);
                put(&mut dict, "grade", r.grade as i64);
                put(&mut dict, "similarity", r.similarity as f64);
                put(&mut dict, "timing_error_ms", r.timing_error_ms as f64);
                dict
            })
            .collect()
    }

    #[func]
    fn reset(&mut self) { self.core.reset(); }
}
