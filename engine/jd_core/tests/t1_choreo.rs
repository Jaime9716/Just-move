//! T1 · B6 Coreografía (caja negra): válidos cargan; inválidos se rechazan con ruta JSON y mensaje claro.
#[path = "t1_util.rs"]
mod util;

use jd_core::choreo::{load_choreo, parse_choreo, validate_choreo};
use jd_core::types::*;
use util::*;

const BASE: &str = r#"{"format":"jd-choreo","version":1,
 "song":{"title":"T","artist":"A","audio":"audio.ogg","video":null,"bpm":120.0,"offset_s":0.0,"duration_s":30.0},
 "steps":[{"t":4.0,"dur":0.5,"limbs":["right_hand"],"icon":"punch_up","kind":"accent"},
          {"t":4.5,"dur":1.0,"limbs":["left_hand","right_hand"],"icon":"wave","kind":"move"}],
 "references":{}}"#;

fn with(from: &str, to: &str) -> String {
    assert!(BASE.contains(from), "plantilla sin {}", from);
    BASE.replacen(from, to, 1)
}

#[test]
fn valid_minimal_parses_fields() {
    let c = parse_choreo(BASE).unwrap();
    assert_eq!(c.title, "T");
    assert_eq!(c.video, "");
    assert_eq!(c.ref_imu, "");
    assert_eq!(c.steps.len(), 2);
    assert_eq!(c.steps[1].limbs, vec![Limb::LeftHand, Limb::RightHand]);
    assert_eq!(c.steps[1].kind, StepKind::Move);
    assert!(validate_choreo(&c).is_ok());
    // campos desconocidos se ignoran; video/references ausentes
    let c2 = parse_choreo(&with(r#""references":{}"#, r#""extra":{"x":1}"#).replace(r#""video":null,"#, "")).unwrap();
    assert_eq!(c2.video, "");
    let c3 = parse_choreo(&with(r#""references":{}"#, r#""references":{"imu":"ref_imu.jsonl"}"#)).unwrap();
    assert_eq!(c3.ref_imu, "ref_imu.jsonl");
}

#[test]
fn repo_packages_are_valid() {
    for d in [synth_dir().join("song_demo"), repo_root().join("data/songs/demo_metronomo")] {
        let c = load_choreo(&d.join("choreo.json")).unwrap_or_else(|e| panic!("{}: {}", d.display(), e));
        assert!(!c.steps.is_empty());
        assert!(c.steps.windows(2).all(|w| w[0].t <= w[1].t));
    }
}

#[test]
fn invalid_cases_report_json_path() {
    let cases: Vec<(String, &str)> = vec![
        (with(r#""format":"jd-choreo""#, r#""format":"otro""#), "format"),
        (with(r#""version":1"#, r#""version":2"#), "version"),
        (with(r#""bpm":120.0"#, r#""bpm":-1"#), "song.bpm"),
        (with(r#""bpm":120.0"#, r#""bpm":"rápido""#), "song.bpm"),
        (with(r#""duration_s":30.0"#, r#""duration_s":0"#), "song.duration_s"),
        (with(r#""title":"T","#, ""), "song.title"),
        (with(r#""audio":"audio.ogg""#, r#""audio":"../x.ogg""#), "song.audio"),
        (with(r#""t":4.5"#, r#""t":3.0"#), "steps[1].t"),
        (with(r#""t":4.0"#, r#""t":-1"#), "steps[0].t"),
        (with(r#""dur":0.5"#, r#""dur":-0.5"#), "steps[0].dur"),
        (with(r#""limbs":["right_hand"]"#, r#""limbs":[]"#), "steps[0].limbs"),
        (with(r#""limbs":["right_hand"]"#, r#""limbs":["cabeza"]"#), "steps[0].limbs"),
        (with(r#""limbs":["left_hand","right_hand"]"#, r#""limbs":["left_hand","left_hand"]"#), "steps[1].limbs"),
        (with(r#""kind":"accent""#, r#""kind":"golpe""#), "steps[0].kind"),
        (with(r#""dur":1.0"#, r#""dur":100.0"#), "steps[1]"),
        (with(r#""steps":["#, r#""steps":{"a":1},"old":["#), "steps"),
    ];
    for (text, path) in cases {
        let e = parse_choreo(&text).map(|_| ()).err().unwrap_or_else(|| panic!("aceptado y no debía ({}):\n{}", path, text));
        assert!(e.contains(path), "el error no cita la ruta {}: {}", path, e);
        assert!(e.len() < 2000 && !e.trim().is_empty());
    }
}

#[test]
fn corrupt_and_empty_files_are_rejected_clearly() {
    let d = tmp_dir("choreo");
    let cases: [(&str, &str); 5] = [
        ("empty.json", ""),
        ("blank.json", "   \n\n"),
        ("trunc.json", &BASE[..BASE.len() / 2]),
        ("garbage.json", "\u{0}\u{1}not json at all"),
        ("array.json", "[1,2,3]"),
    ];
    for (name, text) in cases {
        let p = d.join(name);
        std::fs::write(&p, text).unwrap();
        let e = load_choreo(&p).err().unwrap_or_else(|| panic!("{} aceptado", name));
        assert!(e.contains(name), "el error no nombra el archivo: {}", e);
    }
    let e = load_choreo(&d.join("no_existe.json")).unwrap_err();
    assert!(e.contains("no_existe.json"), "{}", e);
    // JSON corrupto: línea y columna
    std::fs::write(d.join("bad.json"), "{\n \"format\": \"jd-choreo\",\n \"version\": 1,,\n}").unwrap();
    let e = load_choreo(&d.join("bad.json")).unwrap_err();
    assert!(e.contains(":3:"), "sin línea del error: {}", e);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn validate_choreo_on_struct() {
    let mut c = parse_choreo(BASE).unwrap();
    c.steps[0].limbs.clear();
    let e = validate_choreo(&c).unwrap_err();
    assert!(e.contains("steps[0]"), "{}", e);
    let mut c = parse_choreo(BASE).unwrap();
    c.steps.swap(0, 1);
    assert!(validate_choreo(&c).unwrap_err().contains("steps[1].t"));
    let mut c = parse_choreo(BASE).unwrap();
    c.bpm = f64::NAN;
    assert!(validate_choreo(&c).is_err(), "bpm NaN aceptado");
}
