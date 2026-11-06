//! phone_sim — simula un móvil-mando (contracts/phone_protocol.md). AGENTE A.
//!
//! Uso: phone_sim <IP:puerto> [--code N] [--seconds S] [--name TEXTO] [--id N]
//!                [--offset-s X] [--drift-ppm P] [--period-s T] [--rate HZ] [--discover]
//! Empareja con HELLO, responde SYNC con su reloj desplazado y envía muestras sintéticas
//! (sacudidas periódicas) a 200 Hz. Sin --seconds corre hasta Ctrl+C.
use jd_core::phone::protocol::{self, Message};
use jd_core::phone::{PhoneSim, SimConfig};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

fn usage() -> ! {
    eprintln!("Uso: phone_sim <IP:puerto> [--code N] [--seconds S] [--name TEXTO] [--id N] [--offset-s X] [--drift-ppm P] [--period-s T] [--rate HZ]");
    eprintln!("     phone_sim --discover [puerto]   (busca servidores por broadcast)");
    std::process::exit(2);
}

fn num<T: std::str::FromStr>(args: &[String], i: usize, name: &str) -> T {
    args.get(i).and_then(|v| v.parse().ok()).unwrap_or_else(|| { eprintln!("valor no válido para {}", name); usage() })
}

fn discover(port: u16) {
    let sock = UdpSocket::bind("0.0.0.0:0").expect("socket");
    sock.set_broadcast(true).expect("broadcast");
    sock.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
    let b = protocol::encode(&protocol::Packet { device_id: 1, seq: 1, msg: Message::Discover });
    for target in [format!("255.255.255.255:{}", port), format!("127.0.0.1:{}", port)] {
        if let Err(e) = sock.send_to(&b, &target) { eprintln!("no se pudo enviar a {}: {}", target, e); }
    }
    let end = Instant::now() + Duration::from_secs(2);
    let mut buf = [0u8; 512];
    let mut found = 0;
    while Instant::now() < end {
        if let Ok((n, from)) = sock.recv_from(&mut buf) {
            if let Ok(p) = protocol::decode(&buf[..n]) {
                if matches!(p.msg, Message::Welcome { .. }) { println!("servidor encontrado: {}", from); found += 1; }
            }
        }
    }
    if found == 0 { println!("no se encontró ningún servidor en el puerto {}", port); }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" { usage(); }
    if args[0] == "--discover" {
        discover(args.get(1).and_then(|p| p.parse().ok()).unwrap_or(protocol::DEFAULT_PORT));
        return;
    }
    let server: SocketAddr = match args[0].to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => { eprintln!("dirección no válida: {}", args[0]); usage() }
    };
    let mut cfg = SimConfig { device_id: 0x51A0_0000 | (std::process::id() & 0xFFFF), ..Default::default() };
    let mut seconds: Option<f64> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--code" => { cfg.pairing_code = num(&args, i + 1, "--code"); i += 1; }
            "--seconds" => { seconds = Some(num(&args, i + 1, "--seconds")); i += 1; }
            "--name" => { cfg.name = args.get(i + 1).cloned().unwrap_or_else(|| usage()); i += 1; }
            "--id" => { cfg.device_id = num(&args, i + 1, "--id"); i += 1; }
            "--offset-s" => { let s: f64 = num(&args, i + 1, "--offset-s"); cfg.clock_offset_ns = (s * 1e9) as i64; i += 1; }
            "--drift-ppm" => { cfg.drift_ppm = num(&args, i + 1, "--drift-ppm"); i += 1; }
            "--period-s" => { cfg.shake_period_s = num(&args, i + 1, "--period-s"); i += 1; }
            "--rate" => { cfg.rate_hz = num(&args, i + 1, "--rate"); i += 1; }
            other => { eprintln!("opción desconocida: {}", other); usage() }
        }
        i += 1;
    }
    let mut sim = PhoneSim::new(server, cfg.clone()).unwrap_or_else(|e| { eprintln!("no se pudo abrir el socket: {}", e); std::process::exit(1) });
    println!("phone_sim: id {:08x}, emparejando con {} (código {:04})...", cfg.device_id, server, cfg.pairing_code);
    match sim.pair(Duration::from_secs(10)) {
        Ok(true) => println!("aceptado: source_id {}, extremidad {}", sim.source_id, jd_core::types::Limb::from_u8(sim.limb)),
        Ok(false) => { eprintln!("rechazado: código de emparejado incorrecto"); std::process::exit(1); }
        Err(e) => { eprintln!("{}", e); std::process::exit(1); }
    }
    let start = Instant::now();
    let total = seconds.map(Duration::from_secs_f64);
    loop {
        let slice = Duration::from_secs(1);
        let left = total.map(|t| t.saturating_sub(start.elapsed()));
        if left == Some(Duration::ZERO) { break; }
        sim.run_for(left.map(|l| l.min(slice)).unwrap_or(slice));
        println!("{:6.1} s  muestras {:7}  paquetes {:6}  sync {:4}  {}", start.elapsed().as_secs_f64(),
            sim.stats.samples_sent, sim.stats.packets_sent, sim.stats.sync_answered,
            if sim.accepted { "conectado" } else { "re-emparejando" });
    }
    sim.send_bye();
    println!("fin: {} muestras enviadas", sim.stats.samples_sent);
}
