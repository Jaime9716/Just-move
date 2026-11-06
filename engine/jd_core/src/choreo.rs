//! B6 · Coreografía: carga y validación de choreo.json (contracts/song_package.md). AGENTE F.
//!
//! Reglas (idénticas a `tools/choreo_validate.py`):
//! - `format == "jd-choreo"`, `version == 1`.
//! - `song`: `title` (texto), `artist` (texto, opcional), `audio` (ruta relativa no vacía),
//!   `video` (ruta relativa o null, opcional), `bpm` (0 < bpm ≤ 400), `offset_s` (≥ 0, < duration_s),
//!   `duration_s` (> 0).
//! - `steps`: lista; cada paso con `t` (≥ 0), `dur` (≥ 0), `t + dur ≤ duration_s`, `limbs` (lista no
//!   vacía de nombres conocidos sin repetir), `icon` (texto, opcional → ""), `kind` ("accent"|"move");
//!   ordenados por `t` (no decreciente).
//! - `references` (opcional): objeto; `imu` ruta relativa o null.
//! - Los campos desconocidos se ignoran (compatibilidad hacia delante).
//!
//! Los errores llevan la ruta JSON del campo (`steps[3].t: debe ser >= steps[2].t`). Se informan
//! todos (hasta `MAX_ERRORS`), uno por línea.

use crate::json::{self, Value};
use crate::types::{Choreo, Limb, Step, StepKind};
use std::path::Path;

pub const FORMAT: &str = "jd-choreo";
pub const VERSION: f64 = 1.0;
pub const MAX_BPM: f64 = 400.0;
/// Tolerancia para `t + dur <= duration_s` (redondeos de editores).
pub const DURATION_EPS: f64 = 1e-6;
const MAX_ERRORS: usize = 20;

/// Carga y valida `choreo.json`. Errores con ruta JSON, uno por línea, precedidos por el archivo:
/// `songs/x/choreo.json: steps[3].t: debe ser >= steps[2].t`.
pub fn load_choreo(path: &Path) -> Result<Choreo, String> {
    let v = json::parse_file(path)?; // ya incluye archivo:línea:columna
    let c = choreo_from_value(&v).map_err(|e| prefix_lines(&path.display().to_string(), &e))?;
    validate_choreo(&c).map_err(|e| prefix_lines(&path.display().to_string(), &e))?;
    Ok(c)
}

/// Igual que `load_choreo` pero desde texto (sin prefijo de archivo).
pub fn parse_choreo(text: &str) -> Result<Choreo, String> {
    let v = json::parse(text)?;
    let c = choreo_from_value(&v)?;
    validate_choreo(&c)?;
    Ok(c)
}

/// Reglas semánticas sobre una coreografía ya construida (rangos, orden, rutas).
pub fn validate_choreo(c: &Choreo) -> Result<(), String> {
    let mut errs = Errors::default();
    if c.title.trim().is_empty() { errs.push("song.title", "no puede estar vacío"); }
    check_rel_path(&mut errs, "song.audio", &c.audio, false);
    check_rel_path(&mut errs, "song.video", &c.video, true);
    check_rel_path(&mut errs, "references.imu", &c.ref_imu, true);
    if !c.bpm.is_finite() || c.bpm <= 0.0 || c.bpm > MAX_BPM {
        errs.push("song.bpm", &format!("debe estar en (0, {}] (es {})", MAX_BPM, c.bpm));
    }
    let dur_ok = c.duration_s.is_finite() && c.duration_s > 0.0;
    if !dur_ok { errs.push("song.duration_s", &format!("debe ser > 0 (es {})", c.duration_s)); }
    if !c.offset_s.is_finite() || c.offset_s < 0.0 {
        errs.push("song.offset_s", &format!("debe ser >= 0 (es {})", c.offset_s));
    } else if dur_ok && c.offset_s >= c.duration_s {
        errs.push("song.offset_s", &format!("debe ser < song.duration_s ({})", c.duration_s));
    }
    for (i, s) in c.steps.iter().enumerate() {
        let p = format!("steps[{}]", i);
        let t_ok = s.t.is_finite() && s.t >= 0.0;
        if !t_ok { errs.push(&format!("{}.t", p), &format!("debe ser >= 0 (es {})", s.t)); }
        let d_ok = s.dur.is_finite() && s.dur >= 0.0;
        if !d_ok { errs.push(&format!("{}.dur", p), &format!("debe ser >= 0 (es {})", s.dur)); }
        if t_ok && d_ok && dur_ok && s.t + s.dur > c.duration_s + DURATION_EPS {
            errs.push(&format!("{}.t", p), &format!("t + dur ({}) supera song.duration_s ({})", s.t + s.dur, c.duration_s));
        }
        if i > 0 {
            let prev = c.steps[i - 1].t;
            if t_ok && prev.is_finite() && s.t < prev {
                errs.push(&format!("{}.t", p), &format!("debe ser >= steps[{}].t ({} < {})", i - 1, s.t, prev));
            }
        }
        if s.limbs.is_empty() { errs.push(&format!("{}.limbs", p), "no puede estar vacío"); }
        for (j, l) in s.limbs.iter().enumerate() {
            if *l == Limb::None {
                errs.push(&format!("{}.limbs[{}]", p, j), "extremidad desconocida");
            } else if s.limbs[..j].contains(l) {
                errs.push(&format!("{}.limbs[{}]", p, j), &format!("extremidad repetida ({})", l.name()));
            }
        }
    }
    errs.finish()
}

// ------------------------------------------------------------------ construcción desde JSON

fn choreo_from_value(v: &Value) -> Result<Choreo, String> {
    let mut errs = Errors::default();
    if v.as_object().is_none() {
        return Err("$: la raíz debe ser un objeto".into());
    }
    match v.get("format") {
        Value::Null if !v.has("format") => errs.push("format", "falta (debe ser \"jd-choreo\")"),
        Value::String(s) if s == FORMAT => {}
        other => errs.push("format", &format!("debe ser \"{}\" (es {})", FORMAT, short(other))),
    }
    match v.get("version") {
        Value::Null if !v.has("version") => errs.push("version", "falta (debe ser 1)"),
        Value::Number(n) if *n == VERSION => {}
        Value::Number(n) => errs.push("version", &format!("versión no soportada {} (se admite 1)", json::fmt_number(*n))),
        other => errs.push("version", &format!("debe ser un número (es {})", short(other))),
    }

    let mut c = Choreo::default();
    let song = v.get("song");
    if !v.has("song") {
        errs.push("song", "falta");
    } else if song.as_object().is_none() {
        errs.push("song", &format!("debe ser un objeto (es {})", short(song)));
    } else {
        c.title = req_str(&mut errs, song, "song", "title");
        c.artist = opt_str(&mut errs, song, "song", "artist", false);
        c.audio = req_str(&mut errs, song, "song", "audio");
        c.video = opt_str(&mut errs, song, "song", "video", true);
        c.bpm = req_num(&mut errs, song, "song", "bpm");
        c.offset_s = req_num(&mut errs, song, "song", "offset_s");
        c.duration_s = req_num(&mut errs, song, "song", "duration_s");
    }

    let steps = v.get("steps");
    if !v.has("steps") {
        errs.push("steps", "falta");
    } else if let Some(arr) = steps.as_array() {
        for (i, sv) in arr.iter().enumerate() {
            let p = format!("steps[{}]", i);
            if sv.as_object().is_none() {
                errs.push(&p, &format!("debe ser un objeto (es {})", short(sv)));
                continue;
            }
            let t = req_num(&mut errs, sv, &p, "t");
            let dur = req_num(&mut errs, sv, &p, "dur");
            let icon = opt_str(&mut errs, sv, &p, "icon", false);
            let mut limbs = Vec::new();
            match sv.get("limbs") {
                Value::Null if !sv.has("limbs") => errs.push(&format!("{}.limbs", p), "falta"),
                Value::Array(la) => {
                    for (j, lv) in la.iter().enumerate() {
                        match lv.as_str() {
                            Some(name) => {
                                let l = Limb::from_name(name);
                                if l == Limb::None {
                                    errs.push(&format!("{}.limbs[{}]", p, j), &format!(
                                        "extremidad desconocida \"{}\" (válidas: left_hand, right_hand, left_foot, right_foot)", name));
                                } else {
                                    limbs.push(l);
                                }
                            }
                            None => errs.push(&format!("{}.limbs[{}]", p, j), &format!("debe ser texto (es {})", short(lv))),
                        }
                    }
                    if la.is_empty() { errs.push(&format!("{}.limbs", p), "no puede estar vacío"); }
                }
                other => errs.push(&format!("{}.limbs", p), &format!("debe ser una lista (es {})", short(other))),
            }
            let kind = match sv.get("kind") {
                Value::Null if !sv.has("kind") => { errs.push(&format!("{}.kind", p), "falta (\"accent\" o \"move\")"); StepKind::Accent }
                Value::String(s) if s == "accent" => StepKind::Accent,
                Value::String(s) if s == "move" => StepKind::Move,
                other => { errs.push(&format!("{}.kind", p), &format!("debe ser \"accent\" o \"move\" (es {})", short(other))); StepKind::Accent }
            };
            c.steps.push(Step { t, dur, limbs, icon, kind });
        }
    } else {
        errs.push("steps", &format!("debe ser una lista (es {})", short(steps)));
    }

    if v.has("references") {
        let r = v.get("references");
        if r.as_object().is_none() {
            errs.push("references", &format!("debe ser un objeto (es {})", short(r)));
        } else {
            c.ref_imu = opt_str(&mut errs, r, "references", "imu", true);
        }
    }

    // Errores estructurales primero; si los hay, no seguimos (evita mensajes en cascada).
    errs.finish()?;
    Ok(c)
}

// ------------------------------------------------------------------ utilidades

#[derive(Default)]
struct Errors { list: Vec<String>, extra: usize }

impl Errors {
    fn push(&mut self, path: &str, msg: &str) {
        if self.list.len() < MAX_ERRORS { self.list.push(format!("{}: {}", path, msg)); } else { self.extra += 1; }
    }
    fn finish(self) -> Result<(), String> {
        if self.list.is_empty() { return Ok(()); }
        let mut s = self.list.join("\n");
        if self.extra > 0 { s.push_str(&format!("\n... y {} errores más", self.extra)); }
        Err(s)
    }
}

fn prefix_lines(prefix: &str, s: &str) -> String {
    s.lines().map(|l| format!("{}: {}", prefix, l)).collect::<Vec<_>>().join("\n")
}

/// Descripción corta de un valor para mensajes.
fn short(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => json::fmt_number(*n),
        Value::String(s) => {
            let t: String = s.chars().take(40).collect();
            format!("\"{}\"", t)
        }
        Value::Array(_) => "una lista".into(),
        Value::Object(_) => "un objeto".into(),
    }
}

fn req_str(errs: &mut Errors, obj: &Value, parent: &str, key: &str) -> String {
    let path = format!("{}.{}", parent, key);
    match obj.get(key) {
        Value::String(s) => s.clone(),
        Value::Null if !obj.has(key) => { errs.push(&path, "falta"); String::new() }
        other => { errs.push(&path, &format!("debe ser texto (es {})", short(other))); String::new() }
    }
}

/// Texto opcional; `nullable` permite null explícito. Ausente/null → "".
fn opt_str(errs: &mut Errors, obj: &Value, parent: &str, key: &str, nullable: bool) -> String {
    let path = format!("{}.{}", parent, key);
    match obj.get(key) {
        Value::String(s) => s.clone(),
        Value::Null if !obj.has(key) || nullable => String::new(),
        other => {
            let want = if nullable { "texto o null" } else { "texto" };
            errs.push(&path, &format!("debe ser {} (es {})", want, short(other)));
            String::new()
        }
    }
}

fn req_num(errs: &mut Errors, obj: &Value, parent: &str, key: &str) -> f64 {
    let path = format!("{}.{}", parent, key);
    match obj.get(key) {
        Value::Number(n) => *n,
        Value::Null if !obj.has(key) => { errs.push(&path, "falta"); f64::NAN }
        other => { errs.push(&path, &format!("debe ser un número (es {})", short(other))); f64::NAN }
    }
}

/// Ruta relativa dentro del paquete: sin absolutas, sin "..", sin barras invertidas.
/// `optional`: vacío permitido (= no hay archivo).
fn check_rel_path(errs: &mut Errors, path: &str, p: &str, optional: bool) {
    if p.is_empty() {
        if !optional { errs.push(path, "no puede estar vacío"); }
        return;
    }
    if p.starts_with('/') || p.contains('\\') || p.contains(':') {
        errs.push(path, &format!("debe ser una ruta relativa dentro del paquete (es \"{}\")", p));
    } else if p.split('/').any(|seg| seg == "..") {
        errs.push(path, &format!("no puede salir del paquete con \"..\" (es \"{}\")", p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &str = r#"{"format":"jd-choreo","version":1,
      "song":{"title":"T","artist":"A","audio":"audio.ogg","video":null,"bpm":120,"offset_s":0.5,"duration_s":30},
      "steps":[{"t":1,"dur":0.25,"limbs":["right_hand"],"icon":"punch","kind":"accent"},
               {"t":1,"dur":1,"limbs":["left_hand","left_foot"],"kind":"move"}],
      "references":{}}"#;

    #[test]
    fn parses_valid() {
        let c = parse_choreo(OK).unwrap();
        assert_eq!(c.steps.len(), 2);
        assert_eq!(c.video, "");
        assert_eq!(c.steps[1].icon, "");
        assert_eq!(c.steps[1].kind, StepKind::Move);
        assert_eq!(c.steps[1].limbs, vec![Limb::LeftHand, Limb::LeftFoot]);
    }

    #[test]
    fn order_error_message() {
        let mut c = parse_choreo(OK).unwrap();
        c.steps[1].t = 0.5;
        let e = validate_choreo(&c).unwrap_err();
        assert!(e.contains("steps[1].t: debe ser >= steps[0].t"), "{}", e);
    }

    #[test]
    fn rel_paths() {
        let mut c = parse_choreo(OK).unwrap();
        c.audio = "../x.ogg".into();
        c.ref_imu = "/etc/passwd".into();
        let e = validate_choreo(&c).unwrap_err();
        assert!(e.contains("song.audio:") && e.contains("references.imu:"), "{}", e);
    }
}
