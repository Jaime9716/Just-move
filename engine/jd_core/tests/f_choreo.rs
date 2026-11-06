//! Agente F · pruebas de integración de B6 (carga y validación de choreo.json).
use jd_core::choreo::{load_choreo, parse_choreo, validate_choreo};
use jd_core::{Limb, StepKind};
use std::path::PathBuf;

const VALID: &str = r#"{
  "format": "jd-choreo",
  "version": 1,
  "song": {"title": "Mi canción", "artist": "Artista", "audio": "audio.ogg", "video": "video.ogv",
           "bpm": 120.0, "offset_s": 0.0, "duration_s": 180.0},
  "steps": [
    {"t": 4.0, "dur": 0.5, "limbs": ["right_hand"], "icon": "punch_up", "kind": "accent"},
    {"t": 4.5, "dur": 1.0, "limbs": ["left_hand","right_hand"], "icon": "wave", "kind": "move"},
    {"t": 6.0, "dur": 0.5, "limbs": ["left_foot","right_foot"], "icon": "stomp", "kind": "accent"}
  ],
  "references": { "imu": "ref_imu.jsonl" },
  "extra_field_ignored": 1
}"#;

fn tmp_file(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jd_f_tests_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

fn err_of(text: &str) -> String {
    match parse_choreo(text) {
        Ok(_) => panic!("debería fallar:\n{}", text),
        Err(e) => e,
    }
}

#[test]
fn valid_example_from_contract_loads() {
    let p = tmp_file("valid.json", VALID);
    let c = load_choreo(&p).unwrap();
    assert_eq!(c.title, "Mi canción");
    assert_eq!(c.artist, "Artista");
    assert_eq!(c.audio, "audio.ogg");
    assert_eq!(c.video, "video.ogv");
    assert_eq!(c.bpm, 120.0);
    assert_eq!(c.duration_s, 180.0);
    assert_eq!(c.ref_imu, "ref_imu.jsonl");
    assert_eq!(c.steps.len(), 3);
    assert_eq!(c.steps[0].kind, StepKind::Accent);
    assert_eq!(c.steps[1].kind, StepKind::Move);
    assert_eq!(c.steps[1].limbs, vec![Limb::LeftHand, Limb::RightHand]);
    assert_eq!(c.steps[2].limbs, vec![Limb::LeftFoot, Limb::RightFoot]);
    assert_eq!(c.steps[0].icon, "punch_up");
    validate_choreo(&c).unwrap();
}

#[test]
fn minimal_valid() {
    let c = parse_choreo(r#"{"format":"jd-choreo","version":1,
        "song":{"title":"x","audio":"a.ogg","bpm":90,"offset_s":0,"duration_s":10},"steps":[]}"#).unwrap();
    assert_eq!(c.video, "");
    assert_eq!(c.artist, "");
    assert_eq!(c.ref_imu, "");
    assert!(c.steps.is_empty());
}

#[test]
fn null_video_and_empty_references() {
    let t = VALID.replace("\"video.ogv\"", "null").replace("{ \"imu\": \"ref_imu.jsonl\" }", "{}");
    let c = parse_choreo(&t).unwrap();
    assert_eq!(c.video, "");
    assert_eq!(c.ref_imu, "");
}

#[test]
fn incomplete_files_report_missing_fields() {
    let e = err_of(r#"{"format":"jd-choreo","version":1}"#);
    assert!(e.contains("song: falta"), "{}", e);
    assert!(e.contains("steps: falta"), "{}", e);

    let e = err_of(&VALID.replace("\"bpm\": 120.0,", ""));
    assert!(e.contains("song.bpm: falta"), "{}", e);

    let e = err_of(&VALID.replace("\"dur\": 1.0, ", ""));
    assert!(e.contains("steps[1].dur: falta"), "{}", e);

    let e = err_of(&VALID.replace("\"format\": \"jd-choreo\",", ""));
    assert!(e.contains("format: falta"), "{}", e);

    let e = err_of(&VALID.replace("\"limbs\": [\"right_hand\"], ", ""));
    assert!(e.contains("steps[0].limbs: falta"), "{}", e);
}

#[test]
fn corrupt_json_reports_file_and_position() {
    let p = tmp_file("corrupt.json", &VALID[..VALID.len() / 2]);
    let e = load_choreo(&p).unwrap_err();
    assert!(e.contains("corrupt.json:"), "{}", e);
    // archivo:línea:columna
    assert!(e.split(':').nth(1).map(|s| s.trim().parse::<u32>().is_ok()).unwrap_or(false), "{}", e);

    let p = tmp_file("not_json.json", "esto no es json");
    assert!(load_choreo(&p).is_err());

    let e = load_choreo(&PathBuf::from("/no/existe/choreo.json")).unwrap_err();
    assert!(e.contains("no se puede abrir"), "{}", e);

    let e = err_of("[1,2,3]");
    assert!(e.contains("raíz"), "{}", e);
}

#[test]
fn wrong_types() {
    let e = err_of(&VALID.replace("\"bpm\": 120.0", "\"bpm\": \"120\""));
    assert!(e.contains("song.bpm: debe ser un número"), "{}", e);
    let e = err_of(&VALID.replace("\"version\": 1", "\"version\": 2"));
    assert!(e.contains("version: versión no soportada 2"), "{}", e);
    let e = err_of(&VALID.replace("\"format\": \"jd-choreo\"", "\"format\": \"otro\""));
    assert!(e.contains("format: debe ser \"jd-choreo\""), "{}", e);
    let e = err_of(&VALID.replace("\"limbs\": [\"right_hand\"]", "\"limbs\": \"right_hand\""));
    assert!(e.contains("steps[0].limbs: debe ser una lista"), "{}", e);
    let e = err_of(&VALID.replace("\"icon\": \"wave\"", "\"icon\": 3"));
    assert!(e.contains("steps[1].icon: debe ser texto"), "{}", e);
}

#[test]
fn unordered_steps() {
    let t = VALID.replace("{\"t\": 6.0,", "{\"t\": 4.2,");
    let e = err_of(&t);
    assert!(e.contains("steps[2].t: debe ser >= steps[1].t"), "{}", e);
    // Mismo t está permitido (pasos simultáneos).
    parse_choreo(&VALID.replace("{\"t\": 4.5,", "{\"t\": 4.0,")).unwrap();
}

#[test]
fn unknown_and_repeated_limbs() {
    let e = err_of(&VALID.replace("[\"right_hand\"]", "[\"head\"]"));
    assert!(e.contains("steps[0].limbs[0]: extremidad desconocida \"head\""), "{}", e);
    let e = err_of(&VALID.replace("[\"right_hand\"]", "[]"));
    assert!(e.contains("steps[0].limbs: no puede estar vacío"), "{}", e);
    let e = err_of(&VALID.replace("[\"left_hand\",\"right_hand\"]", "[\"left_hand\",\"left_hand\"]"));
    assert!(e.contains("steps[1].limbs[1]: extremidad repetida"), "{}", e);
    let e = err_of(&VALID.replace("[\"right_hand\"]", "[7]"));
    assert!(e.contains("steps[0].limbs[0]: debe ser texto"), "{}", e);
}

#[test]
fn invalid_kind() {
    let e = err_of(&VALID.replace("\"kind\": \"move\"", "\"kind\": \"spin\""));
    assert!(e.contains("steps[1].kind: debe ser \"accent\" o \"move\""), "{}", e);
    let e = err_of(&VALID.replace(", \"kind\": \"move\"", ""));
    assert!(e.contains("steps[1].kind: falta"), "{}", e);
}

#[test]
fn ranges() {
    let e = err_of(&VALID.replace("\"bpm\": 120.0", "\"bpm\": 0"));
    assert!(e.contains("song.bpm: debe estar en"), "{}", e);
    let e = err_of(&VALID.replace("\"duration_s\": 180.0", "\"duration_s\": 5.0"));
    assert!(e.contains("steps[2].t: t + dur"), "{}", e);
    let e = err_of(&VALID.replace("\"offset_s\": 0.0", "\"offset_s\": -1"));
    assert!(e.contains("song.offset_s: debe ser >= 0"), "{}", e);
    let e = err_of(&VALID.replace("{\"t\": 4.0,", "{\"t\": -4.0,"));
    assert!(e.contains("steps[0].t: debe ser >= 0"), "{}", e);
    let e = err_of(&VALID.replace("\"dur\": 0.5, \"limbs\": [\"right_hand\"]", "\"dur\": -0.5, \"limbs\": [\"right_hand\"]"));
    assert!(e.contains("steps[0].dur: debe ser >= 0"), "{}", e);
    let e = err_of(&VALID.replace("\"audio\": \"audio.ogg\"", "\"audio\": \"../../audio.ogg\""));
    assert!(e.contains("song.audio: no puede salir del paquete"), "{}", e);
    let e = err_of(&VALID.replace("\"title\": \"Mi canción\"", "\"title\": \"  \""));
    assert!(e.contains("song.title: no puede estar vacío"), "{}", e);
}

#[test]
fn file_errors_are_prefixed_per_line() {
    let p = tmp_file("multi.json", &VALID.replace("\"kind\": \"move\"", "\"kind\": \"x\"").replace("[\"right_hand\"]", "[\"head\"]"));
    let e = load_choreo(&p).unwrap_err();
    let lines: Vec<&str> = e.lines().collect();
    assert_eq!(lines.len(), 2, "{}", e);
    assert!(lines.iter().all(|l| l.contains("multi.json: steps[")), "{}", e);
}

#[test]
fn repo_song_packages_are_valid() {
    // Todo choreo.json versionado en data/songs debe cargar.
    let songs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/songs");
    let Ok(rd) = std::fs::read_dir(&songs) else { return };
    for entry in rd.flatten() {
        let p = entry.path().join("choreo.json");
        if p.exists() {
            let c = load_choreo(&p).unwrap_or_else(|e| panic!("{}", e));
            assert!(!c.steps.is_empty(), "{}: sin pasos", p.display());
        }
    }
}

#[test]
fn shared_cases_with_python_validator() {
    // Mismos casos que tools/tests/test_choreo_validate.py (paridad Rust ↔ Python).
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/tests/choreo_cases/cases.json");
    let root = jd_core::json::parse_file(&p).unwrap();
    let cases = root.get("cases").as_array().unwrap();
    assert!(cases.len() > 30);
    for c in cases {
        let name = c.get("name").as_str().unwrap();
        let text = match c.get("raw").as_str() { Some(r) => r.to_string(), None => c.get("doc").dump() };
        let res = parse_choreo(&text);
        match c.get("error") {
            jd_core::json::Value::Null => { res.unwrap_or_else(|e| panic!("{}: debería ser válido: {}", name, e)); }
            v => {
                let want = v.as_str().unwrap();
                let e = match res { Ok(_) => panic!("{}: debería fallar", name), Err(e) => e };
                assert!(e.contains(want), "{}: se esperaba '{}' en:\n{}", name, want, e);
            }
        }
    }
}
