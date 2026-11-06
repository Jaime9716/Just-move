//! Codificación/decodificación de los mensajes UDP (contracts/phone_protocol.md).
//! Todo little-endian. `decode` nunca entra en pánico: cualquier dato basura → `Err`.

pub const MAGIC: [u8; 4] = *b"JDM1";
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 16;
pub const SAMPLE_LEN: usize = 36;
pub const DEFAULT_PORT: u16 = 47800;
/// Máximo de muestras por paquete que aceptamos (el móvil envía ≤ 8; margen para lotes tras pérdidas).
pub const MAX_SAMPLES_PER_PACKET: usize = 64;

pub const T_HELLO: u8 = 1;
pub const T_WELCOME: u8 = 2;
pub const T_SAMPLES: u8 = 3;
pub const T_SYNC_REQ: u8 = 4;
pub const T_SYNC_RESP: u8 = 5;
pub const T_STATUS: u8 = 6;
pub const T_BYE: u8 = 7;
pub const T_DISCOVER: u8 = 8;

/// Muestra tal como la envía el móvil (t_ns en el reloj del móvil).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PhoneSample {
    pub t_ns: i64,
    pub flags: u8,
    pub accel: [f32; 3],
    pub gyro: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Hello { pairing_code: u32, name: String },
    Welcome { source_id: u32, limb: u8, accepted: bool },
    Samples(Vec<PhoneSample>),
    SyncReq { t_pc_send: i64 },
    SyncResp { t_pc_send: i64, t_phone_recv: i64, t_phone_send: i64 },
    Status { battery: f32, sensor_rate_hz: u16, dropped: u16 },
    Bye,
    Discover,
}

impl Message {
    pub fn type_id(&self) -> u8 {
        match self {
            Message::Hello { .. } => T_HELLO,
            Message::Welcome { .. } => T_WELCOME,
            Message::Samples(_) => T_SAMPLES,
            Message::SyncReq { .. } => T_SYNC_REQ,
            Message::SyncResp { .. } => T_SYNC_RESP,
            Message::Status { .. } => T_STATUS,
            Message::Bye => T_BYE,
            Message::Discover => T_DISCOVER,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Packet {
    pub device_id: u32,
    pub seq: u32,
    pub msg: Message,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    TooShort(usize),
    BadMagic,
    BadVersion(u8),
    UnknownType(u8),
    /// Longitud del cuerpo incorrecta para el tipo (tipo, esperada, recibida).
    BadLength(u8, usize, usize),
    TooManySamples(usize),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::TooShort(n) => write!(f, "paquete demasiado corto ({} bytes)", n),
            DecodeError::BadMagic => write!(f, "magic distinto de JDM1"),
            DecodeError::BadVersion(v) => write!(f, "versión de protocolo no soportada: {}", v),
            DecodeError::UnknownType(t) => write!(f, "tipo de mensaje desconocido: {}", t),
            DecodeError::BadLength(t, e, g) => write!(f, "tipo {}: cuerpo de {} bytes (se esperaban {})", t, g, e),
            DecodeError::TooManySamples(n) => write!(f, "demasiadas muestras en un paquete: {}", n),
        }
    }
}

// ------------------------------------------------------------------ codificación

fn put_header(out: &mut Vec<u8>, ty: u8, device_id: u32, seq: u32) {
    out.extend_from_slice(&MAGIC);
    out.push(ty);
    out.push(VERSION);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&device_id.to_le_bytes());
    out.extend_from_slice(&seq.to_le_bytes());
}

pub fn encode(p: &Packet) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    put_header(&mut out, p.msg.type_id(), p.device_id, p.seq);
    match &p.msg {
        Message::Hello { pairing_code, name } => {
            out.extend_from_slice(&pairing_code.to_le_bytes());
            // Trunca a 255 bytes respetando los límites de carácter UTF-8.
            let mut n = name.len().min(255);
            while !name.is_char_boundary(n) { n -= 1; }
            out.push(n as u8);
            out.extend_from_slice(&name.as_bytes()[..n]);
        }
        Message::Welcome { source_id, limb, accepted } => {
            out.extend_from_slice(&source_id.to_le_bytes());
            out.push(*limb);
            out.push(*accepted as u8);
        }
        Message::Samples(v) => {
            let count = v.len().min(u16::MAX as usize);
            out.extend_from_slice(&(count as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            for s in &v[..count] {
                out.extend_from_slice(&s.t_ns.to_le_bytes());
                out.push(s.flags);
                out.extend_from_slice(&[0u8; 3]);
                for x in s.accel.iter().chain(s.gyro.iter()) { out.extend_from_slice(&x.to_le_bytes()); }
            }
        }
        Message::SyncReq { t_pc_send } => out.extend_from_slice(&t_pc_send.to_le_bytes()),
        Message::SyncResp { t_pc_send, t_phone_recv, t_phone_send } => {
            out.extend_from_slice(&t_pc_send.to_le_bytes());
            out.extend_from_slice(&t_phone_recv.to_le_bytes());
            out.extend_from_slice(&t_phone_send.to_le_bytes());
        }
        Message::Status { battery, sensor_rate_hz, dropped } => {
            out.extend_from_slice(&battery.to_le_bytes());
            out.extend_from_slice(&sensor_rate_hz.to_le_bytes());
            out.extend_from_slice(&dropped.to_le_bytes());
        }
        Message::Bye | Message::Discover => {}
    }
    out
}

// ------------------------------------------------------------------ decodificación

struct Rd<'a> { b: &'a [u8], i: usize }

impl<'a> Rd<'a> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        // El llamante ya validó la longitud; aun así, nunca indexa fuera de rango.
        let mut a = [0u8; N];
        if self.i + N <= self.b.len() { a.copy_from_slice(&self.b[self.i..self.i + N]); }
        self.i += N;
        a
    }
    fn u8(&mut self) -> u8 { self.take::<1>()[0] }
    fn u16(&mut self) -> u16 { u16::from_le_bytes(self.take()) }
    fn u32(&mut self) -> u32 { u32::from_le_bytes(self.take()) }
    fn i64(&mut self) -> i64 { i64::from_le_bytes(self.take()) }
    fn f32(&mut self) -> f32 { f32::from_le_bytes(self.take()) }
}

fn expect_len(ty: u8, body: usize, want: usize) -> Result<(), DecodeError> {
    if body == want { Ok(()) } else { Err(DecodeError::BadLength(ty, want, body)) }
}

pub fn decode(b: &[u8]) -> Result<Packet, DecodeError> {
    if b.len() < HEADER_LEN { return Err(DecodeError::TooShort(b.len())); }
    if b[0..4] != MAGIC { return Err(DecodeError::BadMagic); }
    let ty = b[4];
    if b[5] != VERSION { return Err(DecodeError::BadVersion(b[5])); }
    let mut r = Rd { b, i: 8 };
    let device_id = r.u32();
    let seq = r.u32();
    let body = b.len() - HEADER_LEN;
    let msg = match ty {
        T_HELLO => {
            if body < 5 { return Err(DecodeError::BadLength(ty, 5, body)); }
            let pairing_code = r.u32();
            let n = r.u8() as usize;
            expect_len(ty, body, 5 + n)?;
            let name = String::from_utf8_lossy(&b[HEADER_LEN + 5..HEADER_LEN + 5 + n]).into_owned();
            Message::Hello { pairing_code, name }
        }
        T_WELCOME => {
            expect_len(ty, body, 6)?;
            let source_id = r.u32();
            let limb = r.u8();
            let accepted = r.u8() != 0;
            Message::Welcome { source_id, limb, accepted }
        }
        T_SAMPLES => {
            if body < 4 { return Err(DecodeError::BadLength(ty, 4, body)); }
            let count = r.u16() as usize;
            let _ = r.u16();
            if count > MAX_SAMPLES_PER_PACKET { return Err(DecodeError::TooManySamples(count)); }
            expect_len(ty, body, 4 + count * SAMPLE_LEN)?;
            let mut v = Vec::with_capacity(count);
            for _ in 0..count {
                let t_ns = r.i64();
                let flags = r.u8();
                let _pad = r.take::<3>();
                let accel = [r.f32(), r.f32(), r.f32()];
                let gyro = [r.f32(), r.f32(), r.f32()];
                v.push(PhoneSample { t_ns, flags, accel, gyro });
            }
            Message::Samples(v)
        }
        T_SYNC_REQ => { expect_len(ty, body, 8)?; Message::SyncReq { t_pc_send: r.i64() } }
        T_SYNC_RESP => {
            expect_len(ty, body, 24)?;
            Message::SyncResp { t_pc_send: r.i64(), t_phone_recv: r.i64(), t_phone_send: r.i64() }
        }
        T_STATUS => {
            expect_len(ty, body, 8)?;
            Message::Status { battery: r.f32(), sensor_rate_hz: r.u16(), dropped: r.u16() }
        }
        T_BYE => { expect_len(ty, body, 0)?; Message::Bye }
        T_DISCOVER => { expect_len(ty, body, 0)?; Message::Discover }
        t => return Err(DecodeError::UnknownType(t)),
    };
    Ok(Packet { device_id, seq, msg })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_messages() -> Vec<Message> {
        vec![
            Message::Hello { pairing_code: 1234, name: "Pixel 8 Pro ñ".into() },
            Message::Hello { pairing_code: 0, name: String::new() },
            Message::Welcome { source_id: 3, limb: 1, accepted: true },
            Message::Welcome { source_id: 0, limb: 255, accepted: false },
            Message::Samples(vec![]),
            Message::Samples((0..8).map(|k| PhoneSample { t_ns: -5 + k * 5_000_000, flags: 3,
                accel: [k as f32, 9.81, -0.5], gyro: [0.25, -1.0, 1e-3] }).collect()),
            Message::SyncReq { t_pc_send: 123_456_789_012 },
            Message::SyncResp { t_pc_send: 1, t_phone_recv: -2, t_phone_send: i64::MAX },
            Message::Status { battery: 0.75, sensor_rate_hz: 200, dropped: 3 },
            Message::Bye,
            Message::Discover,
        ]
    }

    #[test]
    fn roundtrip_all() {
        for (i, m) in all_messages().into_iter().enumerate() {
            let p = Packet { device_id: 0xDEADBEEF, seq: i as u32, msg: m };
            let b = encode(&p);
            assert_eq!(&b[0..4], b"JDM1");
            assert_eq!(b[5], 1);
            assert_eq!(decode(&b).unwrap(), p);
        }
    }

    #[test]
    fn exact_layout() {
        let b = encode(&Packet { device_id: 0x01020304, seq: 0x0A0B0C0D, msg: Message::SyncReq { t_pc_send: 0x1122334455667788 } });
        assert_eq!(b, vec![b'J', b'D', b'M', b'1', 4, 1, 0, 0, 4, 3, 2, 1, 0x0D, 0x0C, 0x0B, 0x0A,
            0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]);
        let s = encode(&Packet { device_id: 1, seq: 2, msg: Message::Samples(vec![PhoneSample::default()]) });
        assert_eq!(s.len(), 16 + 4 + 36);
        let h = encode(&Packet { device_id: 1, seq: 2, msg: Message::Hello { pairing_code: 7, name: "ab".into() } });
        assert_eq!(h.len(), 16 + 4 + 1 + 2);
        assert_eq!(encode(&Packet { device_id: 1, seq: 2, msg: Message::Welcome { source_id: 1, limb: 1, accepted: true } }).len(), 22);
        assert_eq!(encode(&Packet { device_id: 1, seq: 2, msg: Message::Status { battery: 1.0, sensor_rate_hz: 1, dropped: 1 } }).len(), 24);
        assert_eq!(encode(&Packet { device_id: 1, seq: 2, msg: Message::SyncResp { t_pc_send: 0, t_phone_recv: 0, t_phone_send: 0 } }).len(), 40);
    }

    #[test]
    fn rejects_bad_input() {
        let good = encode(&Packet { device_id: 1, seq: 1, msg: Message::Status { battery: 0.5, sensor_rate_hz: 200, dropped: 0 } });
        assert_eq!(decode(&good[..10]), Err(DecodeError::TooShort(10)));
        let mut m = good.clone(); m[0] = b'X';
        assert_eq!(decode(&m), Err(DecodeError::BadMagic));
        let mut v = good.clone(); v[5] = 2;
        assert_eq!(decode(&v), Err(DecodeError::BadVersion(2)));
        let mut t = good.clone(); t[4] = 99;
        assert_eq!(decode(&t), Err(DecodeError::UnknownType(99)));
        assert!(matches!(decode(&good[..good.len() - 1]), Err(DecodeError::BadLength(6, 8, 7))));
        let mut long = good.clone(); long.push(0);
        assert!(decode(&long).is_err());
        // HELLO con name_len mayor que lo recibido.
        let mut h = encode(&Packet { device_id: 1, seq: 1, msg: Message::Hello { pairing_code: 1, name: "abc".into() } });
        h[20] = 200;
        assert!(decode(&h).is_err());
        // SAMPLES con count que no encaja.
        let mut s = encode(&Packet { device_id: 1, seq: 1, msg: Message::Samples(vec![PhoneSample::default(); 2]) });
        s[16] = 3;
        assert!(decode(&s).is_err());
        s[16] = 0xFF; s[17] = 0xFF;
        assert_eq!(decode(&s), Err(DecodeError::TooManySamples(65535)));
    }

    #[test]
    fn fuzz_never_panics() {
        let mut x: u64 = 0x9E3779B97F4A7C15;
        let mut rnd = || { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x };
        let seeds: Vec<Vec<u8>> = all_messages().into_iter()
            .map(|m| encode(&Packet { device_id: 5, seq: 9, msg: m })).collect();
        for i in 0..200_000 {
            let mut b = if i % 3 == 0 {
                let n = (rnd() % 80) as usize;
                (0..n).map(|_| rnd() as u8).collect::<Vec<u8>>()
            } else {
                seeds[(rnd() as usize) % seeds.len()].clone()
            };
            if !b.is_empty() {
                for _ in 0..(rnd() % 4) { let k = (rnd() as usize) % b.len(); b[k] = rnd() as u8; }
                if rnd() % 4 == 0 { let n = (rnd() as usize) % (b.len() + 1); b.truncate(n); }
            }
            let _ = decode(&b);
        }
    }
}
