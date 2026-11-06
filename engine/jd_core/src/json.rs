//! JSON mínimo sin dependencias (Fase 0, arquitecto). Solo lectura para el resto de agentes.
//! Objetos con orden de inserción conservado; números como f64.

use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

static NULL: Value = Value::Null;

impl Value {
    pub fn object() -> Value { Value::Object(Vec::new()) }
    pub fn is_null(&self) -> bool { matches!(self, Value::Null) }
    pub fn as_f64(&self) -> Option<f64> { if let Value::Number(n) = self { Some(*n) } else { None } }
    pub fn as_str(&self) -> Option<&str> { if let Value::String(s) = self { Some(s) } else { None } }
    pub fn as_bool(&self) -> Option<bool> { if let Value::Bool(b) = self { Some(*b) } else { None } }
    pub fn as_array(&self) -> Option<&Vec<Value>> { if let Value::Array(a) = self { Some(a) } else { None } }
    pub fn as_object(&self) -> Option<&Vec<(String, Value)>> { if let Value::Object(o) = self { Some(o) } else { None } }
    /// Miembro de un objeto o Null.
    pub fn get(&self, key: &str) -> &Value {
        if let Value::Object(o) = self { for (k, v) in o { if k == key { return v; } } }
        &NULL
    }
    pub fn has(&self, key: &str) -> bool {
        matches!(self, Value::Object(o) if o.iter().any(|(k, _)| k == key))
    }
    /// Inserta o reemplaza (convierte en objeto si no lo era).
    pub fn set(&mut self, key: &str, v: Value) {
        if !matches!(self, Value::Object(_)) { *self = Value::object(); }
        if let Value::Object(o) = self {
            if let Some(e) = o.iter_mut().find(|(k, _)| k == key) { e.1 = v; } else { o.push((key.to_string(), v)); }
        }
    }
    /// Serialización compacta en una línea; números con hasta 9 cifras significativas.
    pub fn dump(&self) -> String { let mut s = String::new(); dump(self, &mut s); s }
}

impl From<f64> for Value { fn from(v: f64) -> Self { Value::Number(v) } }
impl From<f32> for Value { fn from(v: f32) -> Self { Value::Number(v as f64) } }
impl From<i64> for Value { fn from(v: i64) -> Self { Value::Number(v as f64) } }
impl From<u32> for Value { fn from(v: u32) -> Self { Value::Number(v as f64) } }
impl From<bool> for Value { fn from(v: bool) -> Self { Value::Bool(v) } }
impl From<&str> for Value { fn from(v: &str) -> Self { Value::String(v.to_string()) } }
impl From<String> for Value { fn from(v: String) -> Self { Value::String(v) } }

/// Formato numérico estable (%.9g de C).
pub fn fmt_number(d: f64) -> String {
    if !d.is_finite() { return "null".into(); }
    if d == d.trunc() && d.abs() < 1e15 { return format!("{}", d as i64); }
    let s = format!("{:.8e}", d);
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    if (-5..9).contains(&exp) {
        let decimals = (8 - exp).max(0) as usize;
        let mut r = format!("{:.*}", decimals, d);
        if r.contains('.') { while r.ends_with('0') { r.pop(); } if r.ends_with('.') { r.pop(); } }
        r
    } else {
        let mut m = mant.to_string();
        if m.contains('.') { while m.ends_with('0') { m.pop(); } if m.ends_with('.') { m.pop(); } }
        format!("{}e{}{:02}", m, if exp < 0 { '-' } else { '+' }, exp.abs())
    }
}

fn dump_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"), '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"), '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => { let _ = write!(out, "\\u{:04x}", c as u32); }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn dump(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&fmt_number(*n)),
        Value::String(s) => dump_str(s, out),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() { if i > 0 { out.push(','); } dump(x, out); }
            out.push(']');
        }
        Value::Object(o) => {
            out.push('{');
            for (i, (k, x)) in o.iter().enumerate() { if i > 0 { out.push(','); } dump_str(k, out); out.push(':'); dump(x, out); }
            out.push('}');
        }
    }
}

struct Parser<'a> { b: &'a [u8], i: usize }

impl<'a> Parser<'a> {
    fn err(&self, msg: &str) -> String {
        let (mut line, mut col) = (1, 1);
        for &c in &self.b[..self.i.min(self.b.len())] { if c == b'\n' { line += 1; col = 1; } else { col += 1; } }
        format!("{}:{}: {}", line, col, msg)
    }
    fn ws(&mut self) { while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') { self.i += 1; } }
    fn lit(&mut self, s: &str) -> bool {
        if self.b[self.i..].starts_with(s.as_bytes()) { self.i += s.len(); true } else { false }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        if self.i + 4 > self.b.len() { return Err(self.err("escape \\u incompleto")); }
        let s = std::str::from_utf8(&self.b[self.i..self.i + 4]).map_err(|_| self.err("escape \\u no válido"))?;
        let v = u32::from_str_radix(s, 16).map_err(|_| self.err("escape \\u no hexadecimal"))?;
        self.i += 4;
        Ok(v)
    }
    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).map_err(|_| self.err("UTF-8 no válido")),
                b'\\' => {
                    if self.i >= self.b.len() { break; }
                    let e = self.b[self.i];
                    self.i += 1;
                    match e {
                        b'"' => out.push(b'"'), b'\\' => out.push(b'\\'), b'/' => out.push(b'/'),
                        b'b' => out.push(8), b'f' => out.push(12), b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'), b't' => out.push(b'\t'),
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..=0xDBFF).contains(&cp) && self.b[self.i..].starts_with(b"\\u") {
                                self.i += 2;
                                let lo = self.hex4()?;
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF);
                            }
                            let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(self.err("escape desconocido")),
                    }
                }
                _ => out.push(c),
            }
        }
        Err(self.err("cadena sin cerrar"))
    }
    fn value(&mut self, depth: u32) -> Result<Value, String> {
        if depth > 200 { return Err(self.err("anidamiento excesivo")); }
        self.ws();
        if self.i >= self.b.len() { return Err(self.err("se esperaba un valor")); }
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                let mut o = Vec::new();
                self.ws();
                if self.i < self.b.len() && self.b[self.i] == b'}' { self.i += 1; return Ok(Value::Object(o)); }
                loop {
                    self.ws();
                    if self.i >= self.b.len() || self.b[self.i] != b'"' { return Err(self.err("se esperaba una clave entre comillas")); }
                    let k = self.string()?;
                    self.ws();
                    if self.i >= self.b.len() || self.b[self.i] != b':' { return Err(self.err("se esperaba ':'")); }
                    self.i += 1;
                    let v = self.value(depth + 1)?;
                    if let Some(e) = o.iter_mut().find(|(kk, _): &&mut (String, Value)| *kk == k) { e.1 = v; } else { o.push((k, v)); }
                    self.ws();
                    if self.i < self.b.len() && self.b[self.i] == b',' { self.i += 1; continue; }
                    if self.i < self.b.len() && self.b[self.i] == b'}' { self.i += 1; return Ok(Value::Object(o)); }
                    return Err(self.err("se esperaba ',' o '}'"));
                }
            }
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.i < self.b.len() && self.b[self.i] == b']' { self.i += 1; return Ok(Value::Array(a)); }
                loop {
                    a.push(self.value(depth + 1)?);
                    self.ws();
                    if self.i < self.b.len() && self.b[self.i] == b',' { self.i += 1; continue; }
                    if self.i < self.b.len() && self.b[self.i] == b']' { self.i += 1; return Ok(Value::Array(a)); }
                    return Err(self.err("se esperaba ',' o ']'"));
                }
            }
            b'"' => Ok(Value::String(self.string()?)),
            b't' | b'f' | b'n' => {
                if self.lit("true") { Ok(Value::Bool(true)) }
                else if self.lit("false") { Ok(Value::Bool(false)) }
                else if self.lit("null") { Ok(Value::Null) }
                else { Err(self.err("carácter inesperado")) }
            }
            c if c == b'-' || c.is_ascii_digit() => {
                let start = self.i;
                self.i += 1;
                while self.i < self.b.len() && matches!(self.b[self.i], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-') { self.i += 1; }
                let s = std::str::from_utf8(&self.b[start..self.i]).unwrap();
                s.parse::<f64>().map(Value::Number).map_err(|_| { self.i = start; self.err("número no válido") })
            }
            _ => Err(self.err("carácter inesperado")),
        }
    }
}

/// Error con "línea:columna: mensaje".
pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser { b: text.as_bytes(), i: 0 };
    let v = p.value(0)?;
    p.ws();
    if p.i != p.b.len() { return Err(p.err("contenido extra tras el JSON")); }
    Ok(v)
}

pub fn parse_file(path: &std::path::Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("no se puede abrir {}: {}", path.display(), e))?;
    parse(&text).map_err(|e| format!("{}:{}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let v = parse(r#"{"a":[1,2.5,"xé"],"b":true,"c":null,"d":-0.000123456789}"#).unwrap();
        assert_eq!(v.get("a").as_array().unwrap().len(), 3);
        assert_eq!(v.get("a").as_array().unwrap()[2].as_str(), Some("xé"));
        assert_eq!(v.get("b").as_bool(), Some(true));
        assert!(v.get("c").is_null());
        let w = parse(&v.dump()).unwrap();
        assert_eq!(w.dump(), v.dump());
    }
    #[test]
    fn numbers() {
        assert_eq!(fmt_number(12.0), "12");
        assert_eq!(fmt_number(2.5), "2.5");
        assert_eq!(fmt_number(12.345678), "12.345678");
        assert_eq!(parse(&fmt_number(0.1 + 0.2)).unwrap().as_f64().unwrap(), 0.300000000);
    }
    #[test]
    fn error_position() {
        let e = parse("{\n \"a\": }").unwrap_err();
        assert!(e.starts_with("2:"), "{}", e);
    }
}
