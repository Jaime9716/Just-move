//! T1 · B7 Sesiones (caja negra): ida y vuelta exacta, reproducción con reloj falso, grabación.
#[path = "t1_util.rs"]
mod util;

use jd_core::recorder::RecorderSource;
use jd_core::replay::ReplaySource;
use jd_core::session::{load_session, parse_session_str, save_session};
use jd_core::types::*;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use util::*;

fn random_session(n: usize, seed: u64) -> Session {
    let mut r = Lcg(seed);
    let devices = vec![
        SessionDevice { source_id: 1, name: "Móvil ñ \"1\"".into(), kind: "phone".into(), limb: Limb::RightHand },
        SessionDevice { source_id: 7, name: "Pie".into(), kind: "phone".into(), limb: Limb::LeftFoot },
    ];
    let mut t = 1_234_567i64;
    let samples = (0..n).map(|i| {
        t += 4_000_000 + (r.next_u32() % 2_000_000) as i64;
        let (src, limb) = if i % 3 == 0 { (7, Limb::LeftFoot) } else { (1, Limb::RightHand) };
        let flags = match i % 5 { 0 => SAMPLE_HAS_ACCEL, 1 => SAMPLE_HAS_GYRO, _ => SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO };
        let mut f = || (r.uni() * 80.0) as f32;
        let mut s = MotionSample { t_ns: t, source_id: src, limb: limb as u8, flags, _reserved: 0,
            accel: [f(), f(), f()], gyro: [f() / 10.0, f() / 10.0, f() / 10.0] };
        if i == 10 { s.accel = [f32::MIN_POSITIVE, -0.0, 1e-30]; }
        if i == 11 { s.accel = [f32::MAX, 3.4e38, -1.17549435e-38]; }
        if flags & SAMPLE_HAS_ACCEL == 0 { s.accel = [0.0; 3]; }
        if flags & SAMPLE_HAS_GYRO == 0 { s.gyro = [0.0; 3]; }
        s
    }).collect();
    Session { time_base: "song".into(), song_id: "mi_cancion".into(), recorded_at: "2026-11-06T20:00:00Z".into(),
        note: "nota con \"comillas\" y \\ barra\nsalto".into(), devices, samples }
}

#[test]
fn save_load_roundtrip_is_exact() {
    let d = tmp_dir("sess");
    let s = random_session(2000, 42);
    let p1 = d.join("a.jsonl");
    save_session(&p1, &s).unwrap();
    let l = load_session(&p1).unwrap();
    assert_eq!(l.time_base, s.time_base);
    assert_eq!(l.song_id, s.song_id);
    assert_eq!(l.note, s.note);
    assert_eq!(l.devices, s.devices);
    assert_eq!(l.samples.len(), s.samples.len());
    for (a, b) in s.samples.iter().zip(&l.samples) {
        assert_eq!(a.t_ns, b.t_ns);
        assert_eq!((a.source_id, a.limb, a.flags), (b.source_id, b.limb, b.flags));
        if a.flags & SAMPLE_HAS_ACCEL != 0 {
            // igualdad numérica exacta (-0.0 se guarda como 0: inocuo, anotado en el informe)
            for i in 0..3 { assert!(a.accel[i] == b.accel[i], "accel {:?} vs {:?}", a.accel, b.accel); }
        }
        if a.flags & SAMPLE_HAS_GYRO != 0 {
            for i in 0..3 { assert!(a.gyro[i] == b.gyro[i], "gyro {:?} vs {:?}", a.gyro, b.gyro); }
        }
    }
    let p2 = d.join("b.jsonl");
    save_session(&p2, &l).unwrap();
    assert_eq!(std::fs::read(&p1).unwrap(), std::fs::read(&p2).unwrap(), "guardar dos veces no da los mismos bytes");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn synthetic_sessions_roundtrip() {
    let d = tmp_dir("sess2");
    for name in ["perfect", "foot_only"] {
        let s = load_session(&synth_dir().join(format!("{}.jsonl", name))).unwrap();
        assert!(s.samples.len() > 1000);
        let p = d.join("x.jsonl");
        save_session(&p, &s).unwrap();
        let l = load_session(&p).unwrap();
        assert_eq!(l.samples, s.samples);
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn parse_rules_from_contract() {
    let text = "# comentario\n\n{\"format\":\"jd-session\",\"version\":1,\"time_base\":\"mono\",\"song_id\":null,\"recorded_at\":\"x\",\"note\":\"\",\
\"devices\":[{\"source_id\":1,\"name\":\"P\",\"kind\":\"phone\",\"limb\":\"left_hand\"}]}\n\
{\"t\":12.345678,\"src\":1,\"limb\":\"left_hand\",\"a\":[0.1,9.8,0.2],\"g\":[0.01,-0.02,0.0]}\n\
# otro\n\n\
{\"t\":12.35,\"src\":1,\"limb\":\"left_hand\",\"a\":[0.1,9.8,0.2]}\n\
{\"t\":12.355,\"src\":1,\"limb\":\"left_hand\",\"g\":[0.1,0.2,0.3]}\n";
    let s = parse_session_str(text, "mem").unwrap();
    assert_eq!(s.time_base, "mono");
    assert_eq!(s.samples.len(), 3);
    assert_eq!(s.samples[0].t_ns, 12_345_678_000);
    assert_eq!(s.samples[0].limb, Limb::LeftHand as u8);
    assert_eq!(s.samples[1].flags, SAMPLE_HAS_ACCEL);
    assert_eq!(s.samples[2].flags, SAMPLE_HAS_GYRO);
    assert_eq!(s.samples[0].flags, SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO);
}

#[test]
fn invalid_sessions_rejected_with_line() {
    let h = "{\"format\":\"jd-session\",\"version\":1,\"time_base\":\"song\",\"devices\":[]}";
    for (text, line) in [
        (String::new(), None),
        ("\n# solo comentarios\n".to_string(), None),
        ("{\"format\":\"otra\",\"version\":1}".to_string(), Some(":1:")),
        (format!("{}\n{{\"t\":1.0,\"src\":1,\"limb\":\"left_hand\",\"a\":[1,2]}}", h), Some(":2:")),
        (format!("{}\n{{\"t\":1.0,\"src\":1,\"limb\":\"left_hand\",\"a\":[1,2,3]}}\n{{\"t\":", h), Some(":3:")),
        (format!("{}\n{{\"src\":1,\"limb\":\"left_hand\",\"a\":[1,2,3]}}", h), Some(":2:")),
    ] {
        let e = parse_session_str(&text, "s.jsonl").err().unwrap_or_else(|| panic!("aceptada: {:?}", text));
        assert!(e.starts_with("s.jsonl"), "{}", e);
        if let Some(l) = line { assert!(e.contains(l), "sin número de línea {}: {}", l, e); }
    }
}

fn fake_clock() -> (Arc<AtomicI64>, Box<dyn Fn() -> i64 + Send>) {
    let c = Arc::new(AtomicI64::new(5_000_000_000_000));
    let c2 = c.clone();
    (c, Box::new(move || c2.load(Ordering::SeqCst)))
}

#[test]
fn replay_with_fake_clock_emits_in_time_and_identical() {
    let s = load_session(&synth_dir().join("late_40ms.jsonl")).unwrap();
    for speed in [1.0, 2.0] {
        let (clk, f) = fake_clock();
        let mut r = ReplaySource::new(s.clone(), speed).with_clock(f);
        let t0 = clk.load(Ordering::SeqCst);
        let mut out = Vec::new();
        assert_eq!(r.poll(&mut out), 0, "emite antes de start()");
        r.start().unwrap();
        let first = s.samples.iter().map(|x| x.t_ns).min().unwrap();
        let mut now = t0;
        while !r.finished() {
            now += 16_000_000;
            clk.store(now, Ordering::SeqCst);
            let before = out.len();
            r.poll(&mut out);
            for x in &out[before..] { assert!(x.t_ns <= now, "muestra del futuro: {} > {}", x.t_ns, now); }
            assert!(now - t0 < 100_000_000_000, "no termina");
        }
        assert_eq!(out.len(), s.samples.len());
        let mut src = s.samples.clone();
        src.sort_by_key(|x| x.t_ns);
        for (a, b) in src.iter().zip(&out) {
            let want = t0 + ((a.t_ns - first) as f64 / speed).round() as i64;
            assert!((b.t_ns - want).abs() <= 1, "t_ns {} vs {}", b.t_ns, want);
            assert_eq!((a.accel, a.gyro, a.flags, a.limb, a.source_id), (b.accel, b.gyro, b.flags, b.limb, b.source_id));
        }
        // t_ns creciente por dispositivo
        for id in [1u32, 2, 3] {
            let ts: Vec<i64> = out.iter().filter(|x| x.source_id == id).map(|x| x.t_ns).collect();
            assert!(ts.windows(2).all(|w| w[0] < w[1]), "t_ns no creciente en src {}", id);
        }
        assert!(r.devices().iter().all(|d| d.kind == "replay"));
        assert_eq!(r.devices().len(), 3);
    }
}

#[test]
fn replay_assign_limb_and_stop() {
    let s = load_session(&synth_dir().join("perfect.jsonl")).unwrap();
    let (clk, f) = fake_clock();
    let mut r = ReplaySource::new(s, 1.0).with_clock(f);
    r.start().unwrap();
    r.assign_limb(1, Limb::LeftFoot);
    clk.fetch_add(1_000_000_000, Ordering::SeqCst);
    let mut out = Vec::new();
    r.poll(&mut out);
    assert!(out.iter().filter(|x| x.source_id == 1).all(|x| x.limb == Limb::LeftFoot as u8));
    assert_eq!(r.devices().iter().find(|d| d.source_id == 1).unwrap().limb, Limb::LeftFoot);
    r.stop();
    clk.fetch_add(1_000_000_000, Ordering::SeqCst);
    out.clear();
    assert_eq!(r.poll(&mut out), 0, "emite tras stop()");
}

#[test]
fn record_replay_roundtrip_in_song_time() {
    let d = tmp_dir("rec");
    let s = load_session(&synth_dir().join("foot_only.jsonl")).unwrap();
    let (clk, f) = fake_clock();
    let t0 = clk.load(Ordering::SeqCst);
    let replay = ReplaySource::new(s.clone(), 1.0).with_clock(f);
    let p = d.join("rec.jsonl");
    let mut rec = RecorderSource::new(Box::new(replay), &p).unwrap();
    let first = s.samples.iter().map(|x| x.t_ns).min().unwrap();
    // song = mono·1e-9 + m  ⇒  m = t_first − t0·1e-9
    rec.set_mapping(first as f64 * 1e-9 - t0 as f64 * 1e-9);
    rec.start().unwrap();
    let mut out = Vec::new();
    let mut now = t0;
    for _ in 0..3000 {
        now += 16_000_000;
        clk.store(now, Ordering::SeqCst);
        rec.poll(&mut out);
    }
    rec.stop();
    rec.finish().unwrap();
    drop(rec);
    let l = load_session(&p).unwrap();
    assert_eq!(l.time_base, "song");
    assert_eq!(l.samples.len(), s.samples.len());
    let mut src = s.samples.clone();
    src.sort_by_key(|x| x.t_ns);
    for (a, b) in src.iter().zip(&l.samples) {
        assert!((a.t_ns - b.t_ns).abs() <= 1000, "t {} vs {}", a.t_ns, b.t_ns);
        assert_eq!((a.accel, a.gyro, a.flags, a.limb), (b.accel, b.gyro, b.flags, b.limb));
    }
    let _ = std::fs::remove_dir_all(&d);
}
