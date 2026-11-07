//! T1 · API C del motor de punta a punta (contracts/engine_api.h), llamada como lo haría la
//! GDExtension: solo funciones `extern "C"` y punteros crudos.
#![allow(clippy::missing_safety_doc)]

use jd_core::session::load_session;
use jd_core::types::*;
use jd_engine::engine::*;
use jd_engine::sources::*;
use jd_engine::{jd_api_version, jd_now_ns};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

fn repo() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..") }
fn synth() -> PathBuf { repo().join("data/sessions/synthetic") }
fn cs(p: &Path) -> CString { CString::new(p.to_str().unwrap()).unwrap() }
fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("jd_t1capi_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
fn grade_name(g: u8) -> &'static str { match g { 0 => "miss", 1 => "good", 2 => "perfect", _ => "?" } }

fn golden(name: &str) -> Vec<(i64, String, String, f64)> {
    let v = jd_core::json::parse_file(&synth().join("golden").join(format!("{}.json", name))).unwrap();
    v.as_array().unwrap().iter().map(|r| (r.get("step").as_f64().unwrap() as i64, r.get("limb").as_str().unwrap().to_string(),
        r.get("grade").as_str().unwrap().to_string(), r.get("similarity").as_f64().unwrap())).collect()
}

/// Paquete sin referencia IMU (para el golden perfect_noref).
fn noref_song() -> PathBuf {
    let d = tmp("noref");
    let text = std::fs::read_to_string(synth().join("song_demo/choreo.json")).unwrap();
    let mut v = jd_core::json::parse(&text).unwrap();
    v.set("references", jd_core::json::Value::object());
    std::fs::write(d.join("choreo.json"), v.dump()).unwrap();
    d
}

unsafe fn load(e: *mut JdEngine, dir: &Path) -> (i32, String) {
    let mut err = [0 as c_char; 512];
    let r = jd_engine_load_song(e, cs(dir).as_ptr(), err.as_mut_ptr(), err.len() as i32);
    (r, CStr::from_ptr(err.as_ptr()).to_string_lossy().into_owned())
}

unsafe fn drain(e: *mut JdEngine, t: f64, max: usize, out: &mut Vec<StepResult>) {
    let mut buf = vec![StepResult { step_index: -1, limb: 0, grade: 0, _reserved: 0, similarity: 0.0, timing_error_ms: 0.0 }; max];
    loop {
        let n = jd_engine_update(e, t, buf.as_mut_ptr(), max as i32);
        assert!(n >= 0 && n as usize <= max);
        out.extend_from_slice(&buf[..n as usize]);
        if (n as usize) < max { break; }
    }
}

#[test]
fn version_and_abi() {
    assert_eq!(jd_api_version(), 1);
    assert_eq!(std::mem::size_of::<MotionSample>(), 40);
    assert_eq!(std::mem::size_of::<StepResult>(), 16);
    assert_eq!(std::mem::size_of::<JudgeConfig>(), 12);
    assert_eq!(std::mem::size_of::<JD_DeviceInfoC>(), 96);
    let a = jd_now_ns();
    let b = jd_now_ns();
    assert!(b >= a);
    assert_eq!(jd_core::clock::now_ns() / 1_000_000_000, jd_now_ns() / 1_000_000_000);
}

#[test]
fn lifecycle_and_default_config() {
    unsafe {
        let e = jd_engine_create();
        assert!(!e.is_null());
        let mut c = JudgeConfig { perfect_window_ms: 0.0, good_window_ms: 0.0, foot_window_scale: 0.0 };
        jd_engine_default_config(&mut c);
        assert_eq!((c.perfect_window_ms, c.good_window_ms, c.foot_window_scale), (50.0, 100.0, 1.5));
        jd_engine_set_config(e, &c);
        let (r, msg) = load(e, &synth().join("song_demo"));
        assert_eq!(r, 0, "{}", msg);
        assert_eq!(msg, "");
        jd_engine_reset(e);
        jd_engine_destroy(e);
        // varios motores a la vez
        let es: Vec<_> = (0..4).map(|_| jd_engine_create()).collect();
        for e in es { jd_engine_destroy(e); }
    }
}

#[test]
fn load_song_errors_have_message() {
    unsafe {
        let e = jd_engine_create();
        let (r, msg) = load(e, Path::new("/no/existe/cancion"));
        assert!(r < 0);
        assert!(msg.contains("/no/existe/cancion"), "mensaje sin ruta: {}", msg);
        // choreo corrupto: ruta JSON en el mensaje
        let d = tmp("bad");
        std::fs::write(d.join("choreo.json"), r#"{"format":"jd-choreo","version":1,"song":{"title":"x","audio":"a.ogg","bpm":120,"offset_s":0,"duration_s":10},
            "steps":[{"t":2,"dur":0.5,"limbs":["right_hand"],"kind":"accent"},{"t":1,"dur":0.5,"limbs":["right_hand"],"kind":"accent"}],"references":{}}"#).unwrap();
        let (r, msg) = load(e, &d);
        assert!(r < 0);
        assert!(msg.contains("steps[1].t"), "{}", msg);
        // referencia IMU ausente
        std::fs::write(d.join("choreo.json"), std::fs::read_to_string(synth().join("song_demo/choreo.json")).unwrap()).unwrap();
        let (r, msg) = load(e, &d);
        assert!(r < 0 && msg.contains("ref_imu"), "{}", msg);
        // err nulo o de longitud 0/1 no rompen
        assert!(jd_engine_load_song(e, cs(Path::new("/no")).as_ptr(), null_mut(), 100) < 0);
        let mut one = [b'X' as c_char; 1];
        assert!(jd_engine_load_song(e, cs(Path::new("/no")).as_ptr(), one.as_mut_ptr(), 1) < 0);
        assert_eq!(one[0], 0);
        let mut z = [b'X' as c_char; 4];
        assert!(jd_engine_load_song(e, cs(Path::new("/no")).as_ptr(), z.as_mut_ptr(), 0) < 0);
        assert_eq!(z[0], b'X' as c_char, "err_len 0 no debe escribir");
        // tras un fallo, el motor sigue sin canción y no emite nada
        let mut out = Vec::new();
        drain(e, 1e9, 8, &mut out);
        // (puede conservar la canción anterior si la hubo; aquí nunca hubo una válida)
        assert!(out.is_empty());
        jd_engine_destroy(e);
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// contracts/engine_api.h: "Todas las cadenas son UTF-8". Al truncar el mensaje de error en `err_len`,
/// `write_c_string` corta por bytes y puede partir un carácter multibyte → cadena no UTF-8.
#[test]
fn truncated_error_is_valid_utf8() {
    unsafe {
        let e = jd_engine_create();
        let dir = CString::new("/ñññññññññ/canción").unwrap();
        for len in 1..64 {
            let mut err = vec![0 as c_char; len];
            assert!(jd_engine_load_song(e, dir.as_ptr(), err.as_mut_ptr(), len as i32) < 0);
            let c = CStr::from_ptr(err.as_ptr());
            assert!(c.to_bytes().len() < len);
            assert!(c.to_str().is_ok(), "err_len {}: {:?} no es UTF-8", len, c.to_bytes());
        }
        jd_engine_destroy(e);
    }
}

#[test]
fn update_without_song_and_null_pointers() {
    unsafe {
        let e = jd_engine_create();
        let s = [MotionSample { t_ns: 1, source_id: 1, limb: 1, flags: 3, _reserved: 0, accel: [0.0, 9.8, 0.0], gyro: [0.0; 3] }; 4];
        jd_engine_push_samples(e, s.as_ptr(), 4);
        let mut out = [StepResult { step_index: 0, limb: 0, grade: 0, _reserved: 0, similarity: 0.0, timing_error_ms: 0.0 }; 4];
        assert_eq!(jd_engine_update(e, 100.0, out.as_mut_ptr(), 4), 0);
        jd_engine_reset(e);
        // nulos
        jd_engine_destroy(null_mut());
        jd_engine_default_config(null_mut());
        jd_engine_set_config(null_mut(), null());
        jd_engine_set_config(e, null());
        assert!(jd_engine_load_song(null_mut(), cs(Path::new("/x")).as_ptr(), null_mut(), 0) < 0);
        assert!(jd_engine_load_song(e, null(), null_mut(), 0) < 0);
        jd_engine_set_time_mapping(null_mut(), 1.0);
        jd_engine_push_samples(null_mut(), s.as_ptr(), 4);
        jd_engine_push_samples(e, null(), 4);
        jd_engine_push_samples(e, s.as_ptr(), -3);
        assert!(jd_engine_update(null_mut(), 1.0, out.as_mut_ptr(), 4) <= 0);
        assert_eq!(jd_engine_update(e, 1.0, null_mut(), 4), 0);
        assert_eq!(jd_engine_update(e, 1.0, out.as_mut_ptr(), 0), 0);
        jd_engine_reset(null_mut());
        // fuentes
        assert!(jd_source_create_replay(null(), 1.0).is_null());
        assert!(jd_source_create_recorder(null_mut(), null()).is_null());
        jd_source_recorder_set_mapping(null_mut(), 0.0);
        assert!(jd_source_start(null_mut()) < 0);
        jd_source_stop(null_mut());
        let mut ms = [MotionSample::default(); 2];
        assert!(jd_source_poll(null_mut(), ms.as_mut_ptr(), 2) < 0);
        assert_eq!(jd_source_device_count(null_mut()), 0);
        assert!(jd_source_device_info(null_mut(), 0, null_mut()) < 0);
        jd_source_assign_limb(null_mut(), 1, 1);
        assert!(!jd_source_last_error(null_mut()).is_null());
        jd_source_destroy(null_mut());
        let mut buf = [0 as c_char; 32];
        assert_eq!(jd_source_phone_endpoint(null_mut(), buf.as_mut_ptr(), 32), 0);
        jd_engine_destroy(e);
    }
}

#[test]
fn replay_of_bad_session_reports_error() {
    unsafe {
        let s = jd_source_create_replay(cs(Path::new("/no/existe.jsonl")).as_ptr(), 1.0);
        assert!(!s.is_null());
        assert!(jd_source_start(s) < 0);
        let msg = CStr::from_ptr(jd_source_last_error(s)).to_string_lossy().into_owned();
        assert!(msg.contains("existe.jsonl"), "{}", msg);
        jd_source_destroy(s);
    }
}

/// Juzga una sesión empujándola por la API C en fotogramas simulados de 16 ms (mono = canción + 1000 s).
unsafe fn judge_via_capi(song: &Path, session: &Path, max_out: usize) -> Vec<StepResult> {
    let e = jd_engine_create();
    let (r, msg) = load(e, song);
    assert_eq!(r, 0, "{}", msg);
    let offset = 1000.0;
    jd_engine_set_time_mapping(e, -offset);
    let mut s = load_session(session).unwrap().samples;
    s.sort_by_key(|x| x.t_ns);
    for x in s.iter_mut() { x.t_ns += (offset * 1e9) as i64; }
    let mut out = Vec::new();
    let (mut idx, mut t) = (0usize, 0.0f64);
    let end = s.last().unwrap().t_ns as f64 * 1e-9 - offset + 1.0;
    while t < end {
        t += 0.016;
        let st = idx;
        while idx < s.len() && s[idx].t_ns as f64 * 1e-9 - offset <= t { idx += 1; }
        if idx > st { jd_engine_push_samples(e, s[st..].as_ptr(), (idx - st) as i32); }
        drain(e, t, max_out, &mut out);
    }
    drain(e, 1e9, max_out, &mut out);
    jd_engine_destroy(e);
    out
}

fn compare(name: &str, r: &[StepResult], g: &[(i64, String, String, f64)]) -> (usize, usize) {
    assert_eq!(r.len(), g.len(), "{}: nº de resultados", name);
    let mut same = 0;
    for (a, b) in r.iter().zip(g) {
        assert_eq!((a.step_index as i64, Limb::from_u8(a.limb).name()), (b.0, b.1.as_str()), "{}: orden", name);
        if grade_name(a.grade) == b.2 { same += 1; }
        else { eprintln!("{}: paso {} {}: {} vs golden {} (sim {:.4} / {:.4})", name, b.0, b.1, grade_name(a.grade), b.2, a.similarity, b.3); }
    }
    (same, r.len())
}

const SESSIONS: [&str; 8] = ["perfect", "late_40ms", "late_150ms", "early_80ms", "wrong_moves", "still", "noisy", "foot_only"];

#[test]
fn capi_push_matches_golden() {
    let (mut same, mut total) = (0, 0);
    let noref = noref_song();
    for (i, name) in SESSIONS.iter().chain(["perfect_noref"].iter()).enumerate() {
        let (song, sess) = if *name == "perfect_noref" { (noref.clone(), "perfect") } else { (synth().join("song_demo"), *name) };
        // max_out pequeño en algunos casos: no se debe perder ningún resultado
        let r = unsafe { judge_via_capi(&song, &synth().join(format!("{}.jsonl", sess)), [1, 3, 64][i % 3]) };
        let (s, t) = compare(name, &r, &golden(name));
        same += s;
        total += t;
    }
    println!("T1 CAPI push vs golden: {}/{} grades iguales", same, total);
    assert!(same as f64 >= 0.95 * total as f64, "{}/{}", same, total);
    let _ = std::fs::remove_dir_all(&noref);
}

/// Reproducción REAL (jd_source_create_replay a velocidad 1, reloj del sistema) + motor por la API C,
/// como en el juego: las 9 sesiones en paralelo (~32 s de pared).
#[test]
fn capi_replay_realtime_matches_golden() {
    let noref = noref_song();
    let mut handles = Vec::new();
    for name in SESSIONS.iter().chain(["perfect_noref"].iter()) {
        let name = name.to_string();
        let noref = noref.clone();
        handles.push(std::thread::spawn(move || unsafe {
            let (song, sess) = if name == "perfect_noref" { (noref, "perfect".to_string()) } else { (synth().join("song_demo"), name.clone()) };
            let path = synth().join(format!("{}.jsonl", sess));
            let first_song_s = load_session(&path).unwrap().samples.iter().map(|x| x.t_ns).min().unwrap() as f64 * 1e-9;
            let e = jd_engine_create();
            let (r, msg) = load(e, &song);
            assert_eq!(r, 0, "{}", msg);
            let src = jd_source_create_replay(cs(&path).as_ptr(), 1.0);
            assert_eq!(jd_source_device_count(src), if sess == "foot_only" { 1 } else { 3 });
            let mut info = std::mem::zeroed::<JD_DeviceInfoC>();
            assert_eq!(jd_source_device_info(src, 0, &mut info), 0);
            assert_eq!(CStr::from_ptr(info.kind.as_ptr()).to_str().unwrap(), "replay");
            assert_eq!(jd_source_start(src), 0);
            let mut buf = vec![MotionSample::default(); 256];
            let mut mapping: Option<f64> = None;
            let mut out = Vec::new();
            let t_wall0 = jd_now_ns();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(16));
                loop {
                    let n = jd_source_poll(src, buf.as_mut_ptr(), buf.len() as i32);
                    assert!(n >= 0);
                    if n == 0 { break; }
                    if mapping.is_none() {
                        let t0 = buf[..n as usize].iter().map(|x| x.t_ns).min().unwrap();
                        let m = first_song_s - t0 as f64 * 1e-9;
                        mapping = Some(m);
                        jd_engine_set_time_mapping(e, m);
                    }
                    jd_engine_push_samples(e, buf.as_ptr(), n);
                }
                if let Some(m) = mapping {
                    let song_t = jd_now_ns() as f64 * 1e-9 + m;
                    drain(e, song_t, 16, &mut out);
                    if song_t > 31.5 { break; }
                }
                assert!(jd_now_ns() - t_wall0 < 60_000_000_000, "{}: no termina", name);
            }
            drain(e, 1e9, 16, &mut out);
            jd_source_destroy(src);
            jd_engine_destroy(e);
            let g = golden(&name);
            compare(&name, &out, &g)
        }));
    }
    let (mut same, mut total) = (0, 0);
    for h in handles { let (s, t) = h.join().unwrap(); same += s; total += t; }
    println!("T1 CAPI replay en tiempo real vs golden: {}/{} grades iguales", same, total);
    assert!(same as f64 >= 0.95 * total as f64, "{}/{}", same, total);
    let _ = std::fs::remove_dir_all(&noref);
}

#[test]
fn mapping_shifts_song_time() {
    // Mismo resultado con mapping 0 (t_ns = canción) y con mono desplazado 12345.678 s.
    let song = synth().join("song_demo");
    let path = synth().join("late_40ms.jsonl");
    let base = unsafe { judge_via_capi(&song, &path, 64) };
    unsafe {
        let e = jd_engine_create();
        load(e, &song);
        let off = 12345.678;
        jd_engine_set_time_mapping(e, -off);
        let mut s = load_session(&path).unwrap().samples;
        s.sort_by_key(|x| x.t_ns);
        for x in s.iter_mut() { x.t_ns += (off * 1e9).round() as i64; }
        jd_engine_push_samples(e, s.as_ptr(), s.len() as i32);
        let mut out = Vec::new();
        drain(e, 1e9, 64, &mut out);
        let g: Vec<_> = out.iter().map(|r| r.grade).collect();
        let b: Vec<_> = base.iter().map(|r| r.grade).collect();
        assert_eq!(g, b);
        // reset: vuelve a empezar (tras reset, update emite de nuevo todos los pasos como Miss)
        jd_engine_reset(e);
        let mut again = Vec::new();
        drain(e, 1e9, 64, &mut again);
        assert_eq!(again.len(), out.len());
        assert!(again.iter().all(|r| r.grade == 0));
        jd_engine_destroy(e);
    }
}

#[test]
fn recorder_via_capi_writes_song_time() {
    unsafe {
        let d = tmp("rec");
        // sesión corta de 1.5 s
        let mut text = String::from("{\"format\":\"jd-session\",\"version\":1,\"time_base\":\"song\",\"devices\":[{\"source_id\":1,\"name\":\"P\",\"kind\":\"phone\",\"limb\":\"right_hand\"}]}\n");
        for i in 0..300 { text += &format!("{{\"t\":{:.6},\"src\":1,\"limb\":\"right_hand\",\"a\":[0.5,9.8,{}],\"g\":[0,0,0.25]}}\n", 10.0 + i as f64 * 0.005, i); }
        std::fs::write(d.join("in.jsonl"), text).unwrap();
        let rp = jd_source_create_replay(cs(&d.join("in.jsonl")).as_ptr(), 1.0);
        let rec = jd_source_create_recorder(rp, cs(&d.join("out.jsonl")).as_ptr());
        assert!(!rec.is_null());
        // El mapping se fija antes de la primera muestra (la cabecera se escribe con ella): replay t0 ≈ ahora.
        jd_source_recorder_set_mapping(rec, 10.0 - jd_now_ns() as f64 * 1e-9);
        assert_eq!(jd_source_start(rec), 0);
        let mut buf = vec![MotionSample::default(); 7];
        let mut got = 0;
        let t0 = jd_now_ns();
        while got < 300 && jd_now_ns() - t0 < 5_000_000_000 {
            let n = jd_source_poll(rec, buf.as_mut_ptr(), 7);
            assert!(n >= 0);
            got += n;
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(got, 300);
        jd_source_stop(rec);
        jd_source_destroy(rec);
        jd_source_destroy(rp);
        let s = load_session(&d.join("out.jsonl")).unwrap();
        assert_eq!(s.samples.len(), 300);
        assert_eq!(s.time_base, "song");
        for (i, x) in s.samples.iter().enumerate() {
            let want = 10.0 + i as f64 * 0.005;
            assert!((x.t_ns as f64 * 1e-9 - want).abs() < 2e-3, "t {} vs {}", x.t_ns, want);
            assert_eq!(x.accel[2], i as f32);
            // los intervalos entre muestras se conservan al µs
            let rel = (x.t_ns - s.samples[0].t_ns) as f64 * 1e-9;
            assert!((rel - i as f64 * 0.005).abs() < 1e-6, "intervalo {} en {}", rel, i);
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
