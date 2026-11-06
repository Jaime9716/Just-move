//! jd_judge — juzga una sesión grabada contra un paquete de canción (offline). AGENTE C.
//!
//! Uso: jd_judge <song_dir> <session.jsonl> [--json]
//! La sesión debe estar en tiempo de canción (time_base "song"); con "mono" se avisa y se usa tal cual.
use jd_core::engine::{judge_offline, reference_frames};
use jd_core::json::Value;
use jd_core::types::{Grade, Limb, StepKind};
use std::path::Path;
use std::process::ExitCode;

/// Escribe en stdout sin fallar si la tubería se cierra (p. ej. `| head`).
macro_rules! out {
    ($($a:tt)*) => {{ use std::io::Write as _; let _ = writeln!(std::io::stdout(), $($a)*); }};
}

fn grade_name(g: u8) -> &'static str {
    match g { 2 => "perfect", 1 => "good", _ => "miss" }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if pos.len() != 2 {
        eprintln!("uso: jd_judge <song_dir> <session.jsonl> [--json]");
        return ExitCode::from(2);
    }
    let song_dir = Path::new(pos[0]);
    let choreo = match jd_core::choreo::load_choreo(&song_dir.join("choreo.json")) {
        Ok(c) => c,
        Err(e) => { eprintln!("error: {}", e); return ExitCode::from(1); }
    };
    let reference = if choreo.ref_imu.is_empty() { Vec::new() } else {
        match jd_core::session::load_session(&song_dir.join(&choreo.ref_imu)) {
            Ok(s) => reference_frames(&s),
            Err(e) => { eprintln!("error: referencia IMU: {}", e); return ExitCode::from(1); }
        }
    };
    let session = match jd_core::session::load_session(Path::new(pos[1])) {
        Ok(s) => s,
        Err(e) => { eprintln!("error: {}", e); return ExitCode::from(1); }
    };
    if session.time_base != "song" {
        eprintln!("aviso: la sesión tiene time_base \"{}\"; se interpreta como tiempo de canción", session.time_base);
    }
    let cfg = jd_core::types::JudgeConfig::default();
    let results = judge_offline(&choreo, &reference, &session.samples, &cfg);

    let mut counts = [0usize; 3];
    let mut sim_sum = 0.0f64;
    for r in &results { counts[r.grade as usize] += 1; sim_sum += r.similarity as f64; }
    let n = results.len();
    let score = if n == 0 { 0.0 } else {
        (counts[Grade::Perfect as usize] as f64 * 1.0 + counts[Grade::Good as usize] as f64 * 0.5) / n as f64
    };

    if json {
        let mut arr = Vec::new();
        for r in &results {
            let st = &choreo.steps[r.step_index as usize];
            let mut o = Value::object();
            o.set("step", Value::from(r.step_index as i64));
            o.set("t", Value::from(st.t));
            o.set("kind", Value::from(if st.kind == StepKind::Accent { "accent" } else { "move" }));
            o.set("limb", Value::from(Limb::from_u8(r.limb).name()));
            o.set("grade", Value::from(grade_name(r.grade)));
            o.set("similarity", Value::from(r.similarity));
            o.set("timing_error_ms", Value::from(r.timing_error_ms));
            arr.push(o);
        }
        let mut sum = Value::object();
        sum.set("results", Value::from(n as i64));
        sum.set("perfect", Value::from(counts[2] as i64));
        sum.set("good", Value::from(counts[1] as i64));
        sum.set("miss", Value::from(counts[0] as i64));
        sum.set("mean_similarity", Value::from(if n == 0 { 0.0 } else { sim_sum / n as f64 }));
        sum.set("score", Value::from(score));
        sum.set("has_reference", Value::from(!reference.is_empty()));
        let mut root = Value::object();
        root.set("song", Value::from(choreo.title.as_str()));
        root.set("results", Value::Array(arr));
        root.set("summary", sum);
        out!("{}", root.dump());
    } else {
        out!("Canción: {} — {} ({} pasos, referencia IMU: {})", choreo.title, choreo.artist, choreo.steps.len(),
            if reference.is_empty() { "no" } else { "sí" });
        out!("{:>5} {:>8} {:>6} {:<11} {:<8} {:>6} {:>9}", "paso", "t", "tipo", "extremidad", "nota", "simil", "error_ms");
        for r in &results {
            let st = &choreo.steps[r.step_index as usize];
            out!("{:>5} {:>8.3} {:>6} {:<11} {:<8} {:>6.3} {:>9.1}", r.step_index, st.t,
                if st.kind == StepKind::Accent { "accent" } else { "move" },
                Limb::from_u8(r.limb).name(), grade_name(r.grade), r.similarity, r.timing_error_ms);
        }
        out!("Resumen: {} resultados · perfect {} · good {} · miss {} · similitud media {:.3} · puntuación {:.1} %",
            n, counts[2], counts[1], counts[0], if n == 0 { 0.0 } else { sim_sum / n as f64 }, score * 100.0);
    }
    ExitCode::SUCCESS
}
