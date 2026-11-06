//! jd_record — graba móviles-mando a una sesión JSONL (contracts/session_format.md). AGENTE A.
//!
//! Uso: jd_record <salida.jsonl> [--seconds N] [--port P] [--code C] [--song ID] [--note TEXTO]
//! Arranca el servidor, muestra IP:puerto y código y graba (time_base "mono") hasta N segundos,
//! Enter o Ctrl+C. Con Ctrl+C se pierde como mucho el último medio segundo (flush periódico) y la
//! cabecera queda con los dispositivos vistos al empezar a grabar.
use jd_core::phone::{PhoneServerSource, DEFAULT_PORT};
use jd_core::recorder::RecorderSource;
use jd_core::types::MotionSource;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn usage() -> ! {
    eprintln!("Uso: jd_record <salida.jsonl> [--seconds N] [--port P] [--code C] [--song ID] [--note TEXTO]");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0].starts_with('-') { usage(); }
    let out = PathBuf::from(&args[0]);
    let (mut seconds, mut port, mut code) = (None::<f64>, DEFAULT_PORT, 0u32);
    let (mut song, mut note) = (String::new(), String::new());
    let mut i = 1;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_else(|| usage());
        match args[i].as_str() {
            "--seconds" => seconds = Some(v.parse().unwrap_or_else(|_| usage())),
            "--port" => port = v.parse().unwrap_or_else(|_| usage()),
            "--code" => code = v.parse().unwrap_or_else(|_| usage()),
            "--song" => song = v,
            "--note" => note = v,
            _ => usage(),
        }
        i += 2;
    }

    let server = PhoneServerSource::new(port, code);
    let handle = server.handle();
    let mut rec = RecorderSource::new(Box::new(server), &out).unwrap_or_else(|e| { eprintln!("{}", e); std::process::exit(1) });
    rec.set_song_id(&song);
    rec.set_note(&note);
    if let Err(e) = rec.start() { eprintln!("no se pudo arrancar el servidor: {}", e); std::process::exit(1); }
    println!("Servidor de mandos en {}   código de emparejado: {:04}", handle.endpoint(), handle.pairing_code());
    println!("Grabando en {} — pulsa Enter (o Ctrl+C) para terminar{}", out.display(),
        seconds.map(|s| format!(", o espera {} s", s)).unwrap_or_default());

    let enter = Arc::new(AtomicBool::new(false));
    {
        let enter = enter.clone();
        std::thread::spawn(move || { let mut l = String::new(); let _ = std::io::stdin().read_line(&mut l); enter.store(true, Ordering::SeqCst); });
    }
    let start = Instant::now();
    let mut last_print = Instant::now();
    let mut buf = Vec::new();
    let mut counts: HashMap<u32, u64> = HashMap::new();
    let mut window: HashMap<u32, u64> = HashMap::new();
    loop {
        if enter.load(Ordering::SeqCst) { break; }
        if let Some(s) = seconds { if start.elapsed().as_secs_f64() >= s { break; } }
        buf.clear();
        rec.poll(&mut buf);
        for s in &buf { *counts.entry(s.source_id).or_default() += 1; *window.entry(s.source_id).or_default() += 1; }
        if last_print.elapsed() >= Duration::from_secs(1) {
            let dt = last_print.elapsed().as_secs_f64();
            last_print = Instant::now();
            let stats = handle.stats();
            if stats.is_empty() { println!("{:6.1} s  esperando móviles...", start.elapsed().as_secs_f64()); }
            for d in stats {
                let rate = window.get(&d.source_id).copied().unwrap_or(0) as f64 / dt;
                let warn = if d.sync_rtt_ms > jd_core::phone::server::RTT_WARN_MS { "  ¡rtt alto!" } else { "" };
                println!("{:6.1} s  [{}] {:<20} {:<10} {:>6.1} Hz  rtt {:5.1} ms  deriva {:6.1} ppm  perdidos {}  batería {}  {}{}",
                    start.elapsed().as_secs_f64(), d.source_id, d.name, d.limb.name(), rate, d.sync_rtt_ms, d.drift_ppm,
                    d.lost_packets, if d.battery >= 0.0 { format!("{:.0}%", d.battery * 100.0) } else { "?".into() },
                    if d.connected { "conectado" } else { "DESCONECTADO" }, warn);
            }
            window.clear();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    rec.stop();
    let err = rec.last_error();
    if !err.is_empty() { eprintln!("aviso: {}", err); }
    let total: u64 = counts.values().sum();
    println!("Grabadas {} muestras de {} móvil(es) en {:.1} s -> {}", total, counts.len(), start.elapsed().as_secs_f64(), out.display());
}
