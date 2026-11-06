//! B7 · Sesiones: lectura/escritura JSONL (contracts/session_format.md). AGENTE A.
//!
//! - Línea de cabecera = primera línea no vacía que no empieza por `#`.
//! - Muestras: `{"t":..,"src":..,"limb":..,"a":[..],"g":[..]}`; `a`/`g` ausentes = sin ese sensor.
//! - `t_ns = round(t * 1e9)` (sea cual sea `time_base`).
//! - Errores legibles: `"ruta:línea: mensaje"`.
use crate::json::{self, Value};
use crate::types::{Limb, MotionSample, Session, SessionDevice, SAMPLE_HAS_ACCEL, SAMPLE_HAS_GYRO};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

/// Error legible con número de línea ("sesion.jsonl:12: ...").
pub fn load_session(path: &Path) -> Result<Session, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: no se puede abrir: {}", path.display(), e))?;
    parse_session_reader(BufReader::new(f), &path.display().to_string())
}

/// Igual que `load_session` pero desde texto en memoria (`origin` se usa en los mensajes de error).
pub fn parse_session_str(text: &str, origin: &str) -> Result<Session, String> {
    parse_session_reader(BufReader::new(text.as_bytes()), origin)
}

fn parse_session_reader<R: BufRead>(r: R, origin: &str) -> Result<Session, String> {
    let mut session = Session::default();
    let mut header_seen = false;
    for (i, line) in r.split(b'\n').enumerate() {
        let lineno = i + 1;
        let raw = line.map_err(|e| format!("{}:{}: error de lectura: {}", origin, lineno, e))?;
        let text = String::from_utf8(raw).map_err(|_| format!("{}:{}: UTF-8 no válido", origin, lineno))?;
        let text = text.trim_start_matches('\u{feff}').trim();
        if text.is_empty() || text.starts_with('#') { continue; }
        let err = |m: String| format!("{}:{}: {}", origin, lineno, m);
        let v = json::parse(text).map_err(|e| err(format!("JSON no válido ({})", strip_json_line(&e))))?;
        if !header_seen {
            parse_header(&v, &mut session).map_err(err)?;
            header_seen = true;
        } else {
            let s = parse_sample(&v, &session.devices).map_err(err)?;
            session.samples.push(s);
        }
    }
    if !header_seen { return Err(format!("{}:1: sesión vacía (falta la cabecera)", origin)); }
    Ok(session)
}

/// json::parse devuelve "línea:columna: msg"; dentro de una línea JSONL solo interesa la columna.
fn strip_json_line(e: &str) -> String {
    let mut it = e.splitn(3, ':');
    match (it.next(), it.next(), it.next()) {
        (Some(_), Some(col), Some(msg)) => format!("columna {}:{}", col.trim(), msg),
        _ => e.to_string(),
    }
}

fn parse_header(v: &Value, s: &mut Session) -> Result<(), String> {
    if v.as_object().is_none() { return Err("la cabecera debe ser un objeto JSON".into()); }
    match v.get("format").as_str() {
        Some("jd-session") => {}
        Some(f) => return Err(format!("formato desconocido \"{}\" (se esperaba \"jd-session\")", f)),
        None => return Err("falta \"format\":\"jd-session\" en la cabecera".into()),
    }
    match v.get("version").as_f64() {
        Some(x) if x == 1.0 => {}
        Some(x) => return Err(format!("versión de sesión no soportada: {}", x)),
        None => return Err("falta \"version\" en la cabecera".into()),
    }
    s.time_base = match v.get("time_base") {
        Value::Null => "song".into(),
        Value::String(tb) if tb == "song" || tb == "mono" => tb.clone(),
        other => return Err(format!("time_base no válido: {} (\"song\" o \"mono\")", other.dump())),
    };
    s.song_id = opt_str(v, "song_id")?;
    s.recorded_at = opt_str(v, "recorded_at")?;
    s.note = opt_str(v, "note")?;
    s.devices.clear();
    match v.get("devices") {
        Value::Null => {}
        Value::Array(a) => {
            for (k, d) in a.iter().enumerate() {
                let id = d.get("source_id").as_f64().ok_or_else(|| format!("devices[{}]: falta source_id", k))?;
                let source_id = to_u32(id).ok_or_else(|| format!("devices[{}]: source_id no válido", k))?;
                s.devices.push(SessionDevice {
                    source_id,
                    name: d.get("name").as_str().unwrap_or("").to_string(),
                    kind: d.get("kind").as_str().unwrap_or("").to_string(),
                    limb: Limb::from_name(d.get("limb").as_str().unwrap_or("")),
                });
            }
        }
        _ => return Err("\"devices\" debe ser una lista".into()),
    }
    Ok(())
}

fn opt_str(v: &Value, key: &str) -> Result<String, String> {
    match v.get(key) {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s.clone()),
        _ => Err(format!("\"{}\" debe ser texto o null", key)),
    }
}

fn to_u32(x: f64) -> Option<u32> {
    if x.is_finite() && x >= 0.0 && x <= u32::MAX as f64 && x.fract() == 0.0 { Some(x as u32) } else { None }
}

fn vec3(v: &Value, key: &str) -> Result<Option<[f32; 3]>, String> {
    match v.get(key) {
        Value::Null => Ok(None),
        Value::Array(a) if a.len() == 3 => {
            let mut out = [0f32; 3];
            for (i, x) in a.iter().enumerate() {
                out[i] = x.as_f64().filter(|f| f.is_finite())
                    .ok_or_else(|| format!("\"{}\"[{}] no es un número", key, i))? as f32;
            }
            Ok(Some(out))
        }
        _ => Err(format!("\"{}\" debe ser una lista de 3 números", key)),
    }
}

fn parse_sample(v: &Value, devices: &[SessionDevice]) -> Result<MotionSample, String> {
    if v.as_object().is_none() { return Err("la muestra debe ser un objeto JSON".into()); }
    let t = v.get("t").as_f64().ok_or("falta \"t\" (segundos) en la muestra")?;
    if !t.is_finite() || t.abs() > 9.2e9 { return Err(format!("\"t\" fuera de rango: {}", t)); }
    let src = match v.get("src") {
        Value::Null => 0,
        Value::Number(n) => to_u32(*n).ok_or_else(|| format!("\"src\" no válido: {}", n))?,
        _ => return Err("\"src\" debe ser un número".into()),
    };
    let limb = match v.get("limb") {
        Value::String(name) => {
            let l = Limb::from_name(name);
            if l == Limb::None && name != "none" { return Err(format!("extremidad desconocida \"{}\"", name)); }
            l
        }
        Value::Null => devices.iter().find(|d| d.source_id == src).map(|d| d.limb).unwrap_or(Limb::None),
        _ => return Err("\"limb\" debe ser texto".into()),
    };
    let a = vec3(v, "a")?;
    let g = vec3(v, "g")?;
    let mut flags = 0u8;
    if a.is_some() { flags |= SAMPLE_HAS_ACCEL; }
    if g.is_some() { flags |= SAMPLE_HAS_GYRO; }
    Ok(MotionSample {
        t_ns: (t * 1e9).round() as i64,
        source_id: src,
        limb: limb as u8,
        flags,
        _reserved: 0,
        accel: a.unwrap_or([0.0; 3]),
        gyro: g.unwrap_or([0.0; 3]),
    })
}

/// Línea de cabecera (sin salto de línea final).
pub fn header_line(s: &Session) -> String {
    let mut h = Value::object();
    h.set("format", "jd-session".into());
    h.set("version", Value::Number(1.0));
    h.set("time_base", if s.time_base.is_empty() { "song".into() } else { s.time_base.as_str().into() });
    h.set("song_id", if s.song_id.is_empty() { Value::Null } else { s.song_id.as_str().into() });
    h.set("recorded_at", s.recorded_at.as_str().into());
    h.set("note", s.note.as_str().into());
    let devs = s.devices.iter().map(|d| {
        let mut o = Value::object();
        o.set("source_id", d.source_id.into());
        o.set("name", d.name.as_str().into());
        o.set("kind", d.kind.as_str().into());
        o.set("limb", d.limb.name().into());
        o
    }).collect();
    h.set("devices", Value::Array(devs));
    h.dump()
}

/// Segundos con hasta 9 decimales (exacto al nanosegundo para |t| < 1e7 s), sin ceros sobrantes.
pub fn format_seconds(t: f64) -> String {
    if !t.is_finite() { return "0".into(); }
    let mut r = format!("{:.9}", t);
    if r.contains('.') { while r.ends_with('0') { r.pop(); } if r.ends_with('.') { r.pop(); } }
    if r == "-0" { r = "0".into(); }
    r
}

/// Segundos exactos desde nanosegundos enteros (evita errores de redondeo de f64 en la división).
pub fn format_ns_as_seconds(t_ns: i64) -> String {
    let neg = t_ns < 0;
    let a = t_ns.unsigned_abs();
    let (sec, frac) = (a / 1_000_000_000, a % 1_000_000_000);
    let mut r = if frac == 0 { format!("{}", sec) } else {
        let mut f = format!("{:09}", frac);
        while f.ends_with('0') { f.pop(); }
        format!("{}.{}", sec, f)
    };
    if neg && a != 0 { r.insert(0, '-'); }
    r
}

/// Línea de muestra con `t` ya formateado en segundos.
pub fn sample_line_with_t(t: &str, s: &MotionSample) -> String {
    let mut out = String::with_capacity(96);
    out.push_str("{\"t\":");
    out.push_str(t);
    out.push_str(",\"src\":");
    out.push_str(&s.source_id.to_string());
    out.push_str(",\"limb\":\"");
    out.push_str(s.limb().name());
    out.push('"');
    let v3 = |out: &mut String, k: &str, v: &[f32; 3]| {
        out.push_str(",\"");
        out.push_str(k);
        out.push_str("\":[");
        for (i, x) in v.iter().enumerate() {
            if i > 0 { out.push(','); }
            out.push_str(&json::fmt_number(if x.is_finite() { *x as f64 } else { 0.0 }));
        }
        out.push(']');
    };
    if s.flags & SAMPLE_HAS_ACCEL != 0 { v3(&mut out, "a", &s.accel); }
    if s.flags & SAMPLE_HAS_GYRO != 0 { v3(&mut out, "g", &s.gyro); }
    out.push('}');
    out
}

pub fn save_session(path: &Path, s: &Session) -> Result<(), String> {
    let f = std::fs::File::create(path).map_err(|e| format!("{}: no se puede crear: {}", path.display(), e))?;
    let mut w = BufWriter::new(f);
    let io = |e: std::io::Error| format!("{}: error de escritura: {}", path.display(), e);
    writeln!(w, "{}", header_line(s)).map_err(io)?;
    for smp in &s.samples {
        writeln!(w, "{}", sample_line_with_t(&format_ns_as_seconds(smp.t_ns), smp)).map_err(io)?;
    }
    w.flush().map_err(io)?;
    Ok(())
}

/// Fecha UTC actual en ISO 8601 ("2026-11-06T20:00:00Z").
pub fn now_iso8601_utc() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    iso8601_from_unix(secs)
}

pub fn iso8601_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Algoritmo civil_from_days (H. Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 { y += 1; }
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, rem / 3600, (rem / 60) % 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("jd_a_session_{}_{}", std::process::id(), name));
        d
    }

    const EXAMPLE: &str = r#"{"format":"jd-session","version":1,
"time_base":"song","song_id":"mi_cancion","recorded_at":"2026-11-06T20:00:00Z","note":"bien","devices":[{"source_id":1,"name":"Joy-Con (L)","kind":"joycon","limb":"left_hand"}]}"#;

    #[test]
    fn parse_contract_example() {
        let header = EXAMPLE.replace('\n', "");
        let text = format!("{}\n\n# comentario\n{{\"t\":12.345678,\"src\":1,\"limb\":\"left_hand\",\"a\":[0.1,9.8,0.2],\"g\":[0.01,-0.02,0.0]}}\n{{\"t\":12.35,\"src\":1,\"g\":[1,2,3]}}\n", header);
        let s = parse_session_str(&text, "x.jsonl").unwrap();
        assert_eq!(s.time_base, "song");
        assert_eq!(s.song_id, "mi_cancion");
        assert_eq!(s.devices.len(), 1);
        assert_eq!(s.devices[0].limb, Limb::LeftHand);
        assert_eq!(s.samples.len(), 2);
        assert_eq!(s.samples[0].t_ns, 12_345_678_000);
        assert_eq!(s.samples[0].flags, SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO);
        assert_eq!(s.samples[0].accel, [0.1, 9.8, 0.2]);
        assert_eq!(s.samples[1].flags, SAMPLE_HAS_GYRO);
        assert_eq!(s.samples[1].limb, Limb::LeftHand as u8); // heredada de la cabecera
        assert_eq!(s.samples[1].t_ns, 12_350_000_000);
    }

    #[test]
    fn null_song_id_and_mono() {
        let text = "{\"format\":\"jd-session\",\"version\":1,\"time_base\":\"mono\",\"song_id\":null,\"devices\":[]}\n";
        let s = parse_session_str(text, "m").unwrap();
        assert_eq!(s.time_base, "mono");
        assert_eq!(s.song_id, "");
        assert!(s.samples.is_empty());
    }

    #[test]
    fn errors_have_path_and_line() {
        let text = "# c\n{\"format\":\"jd-session\",\"version\":1}\n{\"t\":1}\n{\"t\":2,\"a\":[1,2]}\n";
        let e = parse_session_str(text, "ses.jsonl").unwrap_err();
        assert!(e.starts_with("ses.jsonl:4: "), "{}", e);
        let e = parse_session_str("{\"format\":\"otra\",\"version\":1}", "a").unwrap_err();
        assert!(e.starts_with("a:1: "), "{}", e);
        let e = parse_session_str("{\"format\":\"jd-session\",\"version\":1}\n{\"t\":", "b").unwrap_err();
        assert!(e.starts_with("b:2: JSON no válido"), "{}", e);
        let e = parse_session_str("\n# solo comentarios\n", "c").unwrap_err();
        assert!(e.starts_with("c:1: "), "{}", e);
        let e = parse_session_str("{\"format\":\"jd-session\",\"version\":2}", "d").unwrap_err();
        assert!(e.contains("versión"), "{}", e);
        let e = parse_session_str("{\"format\":\"jd-session\",\"version\":1}\n{\"t\":1,\"limb\":\"cabeza\"}", "e").unwrap_err();
        assert!(e.starts_with("e:2: "), "{}", e);
        let e = load_session(Path::new("/no/existe.jsonl")).unwrap_err();
        assert!(e.starts_with("/no/existe.jsonl"), "{}", e);
    }

    #[test]
    fn roundtrip_exact() {
        let mut s = Session {
            time_base: "song".into(), song_id: "".into(), recorded_at: "2026-11-06T20:00:00Z".into(),
            note: "nota \"con\" comillas\ny salto".into(),
            devices: vec![SessionDevice { source_id: 7, name: "Pixel 8".into(), kind: "phone".into(), limb: Limb::RightHand }],
            samples: vec![],
        };
        for k in 0..500i64 {
            let flags = match k % 3 { 0 => SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO, 1 => SAMPLE_HAS_ACCEL, _ => SAMPLE_HAS_GYRO };
            let f = k as f32;
            s.samples.push(MotionSample {
                t_ns: -1_000_000_007 + k * 5_000_123, source_id: 7, limb: Limb::RightHand as u8, flags, _reserved: 0,
                accel: if flags & SAMPLE_HAS_ACCEL != 0 { [f * 0.1234567, -9.80665, 1e-7 * f] } else { [0.0; 3] },
                gyro: if flags & SAMPLE_HAS_GYRO != 0 { [f.sin(), 3.4e5, -f / 7.0] } else { [0.0; 3] },
            });
        }
        let p = tmp("rt.jsonl");
        save_session(&p, &s).unwrap();
        let back = load_session(&p).unwrap();
        assert_eq!(back, s);
        // Guardar de nuevo produce el mismo archivo byte a byte.
        let p2 = tmp("rt2.jsonl");
        save_session(&p2, &back).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), std::fs::read(&p2).unwrap());
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_file(p2);
    }

    #[test]
    fn seconds_format() {
        assert_eq!(format_ns_as_seconds(12_345_678_000), "12.345678");
        assert_eq!(format_ns_as_seconds(-5_000_000), "-0.005");
        assert_eq!(format_ns_as_seconds(3_000_000_000), "3");
        assert_eq!(format_ns_as_seconds(0), "0");
        assert_eq!(format_seconds(1.5), "1.5");
        assert_eq!(format_seconds(-0.0), "0");
        assert_eq!(iso8601_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601_from_unix(1_793_995_200), "2026-11-06T20:00:00Z");
    }
}
