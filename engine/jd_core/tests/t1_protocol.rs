//! T1 · Protocolo móvil→PC (caja negra): vectores de la app Android (Kotlin) decodificados con
//! jd_core::phone::protocol, codificación independiente según contracts/phone_protocol.md y robustez.
#[path = "t1_util.rs"]
mod util;

use jd_core::json::Value;
use jd_core::phone::protocol::{decode, encode, Message, Packet, PhoneSample};
use util::*;

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn num(v: &Value, k: &str) -> f64 { v.get(k).as_f64().unwrap_or_else(|| panic!("falta {}", k)) }

/// Los i64 extremos no caben en f64: se leen del texto del vector.
fn i64_field(raw: &str, id: &str, key: &str) -> i64 {
    let pos = raw.find(&format!("\"id\": \"{}\"", id)).or_else(|| raw.find(&format!("\"id\":\"{}\"", id))).unwrap();
    let rest = &raw[pos..];
    let k = rest.find(&format!("\"{}\"", key)).unwrap();
    let after = rest[k + key.len() + 2..].trim_start_matches([':', ' ']);
    let end = after.find([',', '}']).unwrap();
    after[..end].trim().parse().unwrap()
}

/// Codificación independiente (escrita aquí a partir del contrato, no con `encode`).
fn my_encode(ty: u8, dev: u32, seq: u32, body: &[u8]) -> Vec<u8> {
    let mut b = b"JDM1".to_vec();
    b.push(ty);
    b.push(1);
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(&dev.to_le_bytes());
    b.extend_from_slice(&seq.to_le_bytes());
    b.extend_from_slice(body);
    b
}

#[test]
fn android_vectors_decode_and_reencode() {
    let path = repo_root().join("android/app/src/test/resources/protocol_vectors.json");
    let raw = std::fs::read_to_string(&path).unwrap();
    let root = jd_core::json::parse(&raw).unwrap();
    let vs = root.get("vectors").as_array().unwrap();
    assert!(vs.len() >= 19, "solo {} vectores", vs.len());
    let mut types_seen = std::collections::BTreeSet::new();
    for v in vs {
        let id = v.get("id").as_str().unwrap();
        let ty = v.get("type").as_str().unwrap();
        types_seen.insert(ty.to_string());
        let bytes = unhex(v.get("hex").as_str().unwrap());
        let p = decode(&bytes).unwrap_or_else(|e| panic!("{}: {}", id, e));
        assert_eq!(p.device_id as f64, num(v, "device_id"), "{}", id);
        assert_eq!(p.seq as f64, num(v, "seq"), "{}", id);
        let mut body = Vec::new();
        let type_id;
        match (&p.msg, ty) {
            (Message::Hello { pairing_code, name }, "HELLO") => {
                assert_eq!(*pairing_code as f64, num(v, "pairing_code"));
                assert_eq!(name, v.get("name").as_str().unwrap(), "{}", id);
                body.extend_from_slice(&pairing_code.to_le_bytes());
                body.push(name.len() as u8);
                body.extend_from_slice(name.as_bytes());
                type_id = 1;
            }
            (Message::Welcome { source_id, limb, accepted }, "WELCOME") => {
                assert_eq!(*source_id as f64, num(v, "source_id"));
                assert_eq!(*limb as f64, num(v, "limb"));
                assert_eq!(*accepted, v.get("accepted").as_bool().unwrap());
                body.extend_from_slice(&source_id.to_le_bytes());
                body.push(*limb);
                body.push(*accepted as u8);
                type_id = 2;
            }
            (Message::Samples(ss), "SAMPLES") => {
                let want = v.get("samples").as_array().unwrap();
                assert_eq!(ss.len(), want.len(), "{}", id);
                body.extend_from_slice(&(ss.len() as u16).to_le_bytes());
                body.extend_from_slice(&[0, 0]);
                for (s, w) in ss.iter().zip(want) {
                    assert_eq!(s.t_ns as f64, num(w, "t_ns"), "{}", id);
                    assert_eq!(s.flags as f64, num(w, "flags"));
                    for (k, arr) in [("accel", s.accel), ("gyro", s.gyro)] {
                        let wa = w.get(k).as_array().unwrap();
                        for i in 0..3 {
                            assert_eq!(arr[i], wa[i].as_f64().unwrap() as f32, "{} {}[{}]", id, k, i);
                        }
                    }
                    body.extend_from_slice(&s.t_ns.to_le_bytes());
                    body.extend_from_slice(&[s.flags, 0, 0, 0]);
                    for x in s.accel.iter().chain(s.gyro.iter()) { body.extend_from_slice(&x.to_le_bytes()); }
                }
                type_id = 3;
            }
            (Message::SyncReq { t_pc_send }, "SYNC_REQ") => {
                assert_eq!(*t_pc_send, i64_field(&raw, id, "t_pc_send"));
                body.extend_from_slice(&t_pc_send.to_le_bytes());
                type_id = 4;
            }
            (Message::SyncResp { t_pc_send, t_phone_recv, t_phone_send }, "SYNC_RESP") => {
                assert_eq!(*t_pc_send, i64_field(&raw, id, "t_pc_send"), "{}", id);
                assert_eq!(*t_phone_recv, i64_field(&raw, id, "t_phone_recv"));
                assert_eq!(*t_phone_send, i64_field(&raw, id, "t_phone_send"));
                for x in [t_pc_send, t_phone_recv, t_phone_send] { body.extend_from_slice(&x.to_le_bytes()); }
                type_id = 5;
            }
            (Message::Status { battery, sensor_rate_hz, dropped }, "STATUS") => {
                assert_eq!(*battery, num(v, "battery") as f32);
                assert_eq!(*sensor_rate_hz as f64, num(v, "sensor_rate_hz"));
                assert_eq!(*dropped as f64, num(v, "dropped"));
                body.extend_from_slice(&battery.to_le_bytes());
                body.extend_from_slice(&sensor_rate_hz.to_le_bytes());
                body.extend_from_slice(&dropped.to_le_bytes());
                type_id = 6;
            }
            (Message::Bye, "BYE") => type_id = 7,
            (Message::Discover, "DISCOVER") => type_id = 8,
            (m, t) => panic!("{}: tipo {} decodificado como {:?}", id, t, m),
        }
        assert_eq!(my_encode(type_id, p.device_id, p.seq, &body), bytes, "{}: codificación independiente ≠ vector", id);
        assert_eq!(encode(&p), bytes, "{}: encode(decode(x)) ≠ x", id);
    }
    for t in ["HELLO", "WELCOME", "SAMPLES", "SYNC_REQ", "SYNC_RESP", "STATUS", "BYE", "DISCOVER"] {
        assert!(types_seen.contains(t), "no hay vector de {}", t);
    }
}

#[test]
fn sample_layout_is_36_bytes_le() {
    let s = PhoneSample { t_ns: 0x0102030405060708, flags: 3, accel: [1.0, -2.0, 9.80665], gyro: [0.5, 0.0, -0.25] };
    let b = encode(&Packet { device_id: 9, seq: 1, msg: Message::Samples(vec![s, s]) });
    assert_eq!(b.len(), 16 + 4 + 2 * 36);
    assert_eq!(&b[16..20], &[2, 0, 0, 0]);
    assert_eq!(&b[20..28], &0x0102030405060708i64.to_le_bytes());
    assert_eq!(&b[28..32], &[3, 0, 0, 0]);
    assert_eq!(&b[32..36], &1.0f32.to_le_bytes());
    assert_eq!(&b[52..56], &(-0.25f32).to_le_bytes());
}

#[test]
fn malformed_packets_are_errors_not_panics() {
    let good = encode(&Packet { device_id: 1, seq: 2, msg: Message::Samples(vec![PhoneSample::default(); 3]) });
    for n in 0..good.len() { assert!(decode(&good[..n]).is_err(), "truncado a {} aceptado", n); }
    let mut bad = good.clone();
    bad[0] = b'X';
    assert!(decode(&bad).is_err());
    let mut bad = good.clone();
    bad[5] = 2;
    assert!(decode(&bad).is_err(), "versión 2 aceptada");
    let mut bad = good.clone();
    bad[4] = 99;
    assert!(decode(&bad).is_err());
    let mut bad = good.clone();
    bad.push(0);
    assert!(decode(&bad).is_err(), "byte de más aceptado");
    let mut bad = good.clone();
    bad[16] = 0xff;
    bad[17] = 0xff; // count = 65535
    assert!(decode(&bad).is_err());
    let hello = my_encode(1, 1, 1, &[1, 0, 0, 0, 200, b'a']); // name_len 200 > resto
    assert!(decode(&hello).is_err());
    // basura aleatoria determinista
    let mut r = Lcg(99);
    for _ in 0..20000 {
        let n = (r.next_u32() % 80) as usize;
        let mut b: Vec<u8> = (0..n).map(|_| r.next_u32() as u8).collect();
        if n >= 6 && r.next_u32() % 2 == 0 { b[..4].copy_from_slice(b"JDM1"); b[5] = 1; b[4] = (r.next_u32() % 9) as u8; }
        let _ = decode(&b);
    }
}

#[test]
fn invalid_utf8_name_does_not_panic() {
    let b = my_encode(1, 1, 1, &[1, 0, 0, 0, 3, 0xff, 0xfe, 0x41]);
    let p = decode(&b).unwrap();
    assert!(matches!(p.msg, Message::Hello { .. }));
}
