//! Agente C · compara el juez Rust con los golden del prototipo Python (agente B).
//! data/sessions/synthetic/golden/<nombre>.json = judge_song(choreo, <nombre>.jsonl, ref_imu).
//! `perfect_noref` = perfect.jsonl sin referencia. Criterio: ≥ 95 % de grades iguales.
use jd_core::engine::{judge_offline, reference_frames};
use jd_core::json;
use jd_core::types::*;
use std::path::PathBuf;

fn syn_dir() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/sessions/synthetic") }

fn grade_name(g: u8) -> &'static str { match g { 2 => "perfect", 1 => "good", _ => "miss" } }

#[test]
fn matches_python_golden() {
    let syn = syn_dir();
    let gold_dir = syn.join("golden");
    if !gold_dir.is_dir() {
        eprintln!("c_golden: no hay golden en {} (se omite)", gold_dir.display());
        return;
    }
    let song = syn.join("song_demo");
    let choreo = jd_core::choreo::load_choreo(&song.join("choreo.json")).expect("choreo");
    let ref_sess = jd_core::session::load_session(&song.join(&choreo.ref_imu)).expect("ref_imu");
    let ref_frames = reference_frames(&ref_sess);
    let cfg = JudgeConfig::default();

    let mut names: Vec<String> = std::fs::read_dir(&gold_dir).unwrap()
        .filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".json")).map(|n| n.trim_end_matches(".json").to_string()).collect();
    names.sort();
    let (mut total, mut same) = (0usize, 0usize);
    let mut sim_err_max = 0.0f64;
    let mut te_err_max = 0.0f64;
    for name in &names {
        let gold = json::parse_file(&gold_dir.join(format!("{}.json", name))).expect("golden");
        let gold = gold.as_array().expect("array");
        let (sess_name, use_ref) = if name == "perfect_noref" { ("perfect", false) } else { (name.as_str(), true) };
        let sess = jd_core::session::load_session(&syn.join(format!("{}.jsonl", sess_name))).expect("sesión");
        let refs: &[FeatureFrame] = if use_ref { &ref_frames } else { &[] };
        let res = judge_offline(&choreo, refs, &sess.samples, &cfg);
        assert_eq!(res.len(), gold.len(), "{}: nº de resultados", name);
        let (mut n, mut ok) = (0, 0);
        for (r, g) in res.iter().zip(gold.iter()) {
            let gs = g.get("step").as_f64().unwrap() as i32;
            let gl = Limb::from_name(g.get("limb").as_str().unwrap());
            assert_eq!((r.step_index, r.limb), (gs, gl as u8), "{}: orden de resultados", name);
            let gg = g.get("grade").as_str().unwrap();
            let gsim = g.get("similarity").as_f64().unwrap();
            let gte = g.get("timing_error_ms").as_f64().unwrap();
            n += 1;
            let d = (r.similarity as f64 - gsim).abs();
            sim_err_max = sim_err_max.max(d);
            te_err_max = te_err_max.max((r.timing_error_ms as f64 - gte).abs());
            if grade_name(r.grade) == gg { ok += 1; } else {
                println!("DIFERENCIA {} paso {} {}: rust {} (sim {:.4}, {:.1} ms) · python {} (sim {:.4}, {:.1} ms)",
                    name, gs, gl.name(), grade_name(r.grade), r.similarity, r.timing_error_ms, gg, gsim, gte);
            }
            if d > 0.02 {
                println!("similitud distinta {} paso {} {}: rust {:.4} · python {:.4}", name, gs, gl.name(), r.similarity, gsim);
            }
        }
        println!("c_golden {}: {}/{} grades iguales", name, ok, n);
        total += n;
        same += ok;
    }
    println!("c_golden TOTAL: {}/{} ({:.1} %), máx |Δsimilitud| = {:.5}, máx |Δtiming| = {:.3} ms", same, total, 100.0 * same as f64 / total.max(1) as f64, sim_err_max, te_err_max);
    assert!(total > 0);
    assert!(same * 100 >= total * 95, "coincidencia {}/{}", same, total);
}

/// Motor completo (load_song + tiempo real en fotogramas de 16 ms con desfase mono→canción)
/// frente al golden: mismos grades que el juicio offline.
#[test]
fn engine_realtime_matches_golden() {
    let syn = syn_dir();
    let gold_path = syn.join("golden/late_40ms.json");
    if !gold_path.is_file() { return; }
    let gold = json::parse_file(&gold_path).unwrap();
    let gold = gold.as_array().unwrap();
    let mut e = jd_core::engine::Engine::new();
    e.load_song(&syn.join("song_demo")).expect("load_song");
    assert!(!e.reference_frames().is_empty());
    let sess = jd_core::session::load_session(&syn.join("late_40ms.jsonl")).unwrap();
    let mut samples = sess.samples.clone();
    samples.sort_by_key(|s| s.t_ns);
    let offset_ns: i64 = 123_456_789_000; // reloj monotónico = canción + 123.456789 s
    for s in samples.iter_mut() { s.t_ns += offset_ns; }
    e.set_time_mapping(-(offset_ns as f64) * 1e-9);
    let mut out = Vec::new();
    let (mut idx, mut t) = (0usize, 0.0f64);
    let end = e.choreo().unwrap().duration_s + 2.0;
    while t < end {
        t += 0.016;
        let s0 = idx;
        while idx < samples.len() && ((samples[idx].t_ns - offset_ns) as f64) * 1e-9 <= t { idx += 1; }
        e.push_samples(&samples[s0..idx]);
        e.update(t, &mut out);
    }
    e.flush();
    e.update(f64::MAX, &mut out);
    out.sort_by_key(|r| (r.step_index, r.limb));
    assert_eq!(out.len(), gold.len());
    let same = out.iter().zip(gold.iter()).filter(|(r, g)| grade_name(r.grade) == g.get("grade").as_str().unwrap()).count();
    println!("c_golden tiempo real late_40ms: {}/{}", same, gold.len());
    assert!(same * 100 >= gold.len() * 95);
}
