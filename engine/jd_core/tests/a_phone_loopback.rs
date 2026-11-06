//! Integración B1 (agente A): servidor de móviles + simulador en 127.0.0.1.
use jd_core::clock::now_ns;
use jd_core::phone::{PhoneServerSource, PhoneSim, SimConfig};
use jd_core::types::{Limb, MotionSample, MotionSource};
use std::net::SocketAddr;
use std::time::Duration;

fn server(code: u32) -> (PhoneServerSource, SocketAddr) {
    let mut s = PhoneServerSource::new(0, code);
    s.start().expect("arranca el servidor");
    let addr: SocketAddr = format!("127.0.0.1:{}", s.local_port()).parse().unwrap();
    (s, addr)
}

fn sim(addr: SocketAddr, code: u32, device_id: u32, offset_ns: i64, drift_ppm: f64) -> PhoneSim {
    PhoneSim::new(addr, SimConfig {
        device_id, pairing_code: code, name: format!("sim-{:x}", device_id),
        clock_offset_ns: offset_ns, drift_ppm, tag_index: true, ..Default::default()
    }).unwrap()
}

/// Comprueba que cada muestra recibida llega con el t_ns real del PC (± tol).
fn check_times(sim: &PhoneSim, got: &[MotionSample], source_id: u32, tol_ns: i64) -> usize {
    let mut n = 0;
    let mut last = i64::MIN;
    for s in got.iter().filter(|s| s.source_id == source_id) {
        let idx = s.accel[2] as usize;
        let expected = sim.true_pc_time(sim.sent_log[idx]);
        assert!((s.t_ns - expected).abs() <= tol_ns, "muestra {}: t_ns {} esperado {} (Δ {} µs)", idx, s.t_ns, expected, (s.t_ns - expected) / 1000);
        assert!(s.t_ns > last, "t_ns no creciente");
        last = s.t_ns;
        n += 1;
    }
    n
}

#[test]
fn samples_arrive_with_pc_time() {
    let (mut srv, addr) = server(4321);
    assert_eq!(srv.pairing_code(), 4321);
    assert!(srv.endpoint().ends_with(&format!(":{}", srv.local_port())));
    // Reloj del móvil desplazado ~ +3 días y con deriva de 50 ppm.
    let mut ph = sim(addr, 4321, 0xA1, 260_000_000_000_000, 50.0);
    assert_eq!(ph.pair(Duration::from_secs(2)), Ok(true));
    assert_eq!(ph.limb, Limb::RightHand as u8);
    let t_start = now_ns();
    ph.run_for(Duration::from_millis(1500));
    let elapsed = (now_ns() - t_start) as f64 / 1e9;
    std::thread::sleep(Duration::from_millis(30));
    let mut got = Vec::new();
    srv.poll(&mut got);
    let n = check_times(&ph, &got, 1, 2_000_000);
    let expected = (elapsed * 200.0) as usize;
    assert!(n as f64 > expected as f64 * 0.9, "solo {} de ~{} muestras", n, expected);
    assert!(got.iter().all(|s| s.limb == Limb::RightHand as u8 && s.flags == 3));

    let devs = srv.devices();
    assert_eq!(devs.len(), 1);
    assert!(devs[0].connected);
    assert_eq!(devs[0].kind, "phone");
    assert!(devs[0].sync_rtt_ms >= 0.0 && devs[0].sync_rtt_ms < 30.0, "rtt {}", devs[0].sync_rtt_ms);
    assert!((devs[0].battery - 0.8).abs() < 1e-6);
    let st = srv.stats();
    assert_eq!(st[0].lost_packets, 0);

    // Segundo móvil: left_hand por defecto; assign_limb cambia la extremidad de sus muestras.
    let mut ph2 = sim(addr, 4321, 0xB2, -5_000_000_000, -30.0);
    assert_eq!(ph2.pair(Duration::from_secs(2)), Ok(true));
    assert_eq!(ph2.limb, Limb::LeftHand as u8);
    srv.assign_limb(2, Limb::LeftFoot);
    ph2.run_for(Duration::from_millis(400));
    assert_eq!(ph2.limb, Limb::LeftFoot as u8, "el servidor reenvía WELCOME con la extremidad nueva");
    got.clear();
    std::thread::sleep(Duration::from_millis(30));
    srv.poll(&mut got);
    assert!(check_times(&ph2, &got, 2, 2_000_000) > 40);
    assert!(got.iter().filter(|s| s.source_id == 2).all(|s| s.limb == Limb::LeftFoot as u8));
    srv.stop();
}

#[test]
fn wrong_code_rejected() {
    let (mut srv, addr) = server(1111);
    let mut ph = sim(addr, 2222, 0xC3, 1_000_000, 0.0);
    assert_eq!(ph.pair(Duration::from_secs(2)), Ok(false));
    // Aunque envíe muestras, el servidor las ignora.
    ph.accepted = true;
    ph.run_for(Duration::from_millis(200));
    std::thread::sleep(Duration::from_millis(20));
    let mut got = Vec::new();
    assert_eq!(srv.poll(&mut got), 0);
    assert!(srv.devices().is_empty());
    assert!(srv.last_error().contains("código"), "{}", srv.last_error());
    assert!(srv.handle().rejected_hellos() >= 1);
    srv.stop();
}

#[test]
fn discover_gets_answer() {
    let (mut srv, addr) = server(1234);
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let b = jd_core::phone::protocol::encode(&jd_core::phone::protocol::Packet { device_id: 9, seq: 1, msg: jd_core::phone::protocol::Message::Discover });
    sock.send_to(&b, addr).unwrap();
    let mut buf = [0u8; 256];
    let (n, _) = sock.recv_from(&mut buf).unwrap();
    let p = jd_core::phone::protocol::decode(&buf[..n]).unwrap();
    assert_eq!(p.msg, jd_core::phone::protocol::Message::Welcome { source_id: 0, limb: 255, accepted: false });
    // Basura: el servidor sigue vivo.
    sock.send_to(b"JDM1\x03\x01xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", addr).unwrap();
    sock.send_to(&b, addr).unwrap();
    assert!(sock.recv_from(&mut buf).is_ok());
    srv.stop();
}

#[test]
fn timeout_and_reconnect() {
    let (mut srv, addr) = server(5555);
    let mut ph = sim(addr, 5555, 0xD4, 42_000_000_000, 20.0);
    assert_eq!(ph.pair(Duration::from_secs(2)), Ok(true));
    ph.run_for(Duration::from_millis(300));
    assert!(srv.devices()[0].connected);
    // El móvil calla más de 3 s -> desconectado.
    std::thread::sleep(Duration::from_millis(3400));
    assert!(!srv.devices()[0].connected, "debería haber expirado");
    let mut got = Vec::new();
    srv.poll(&mut got);

    // La app vuelve (socket nuevo, mismo device_id) con HELLO: mismo source_id y extremidad.
    drop(ph);
    let mut ph = sim(addr, 5555, 0xD4, 42_000_000_000, 20.0);
    assert_eq!(ph.pair(Duration::from_secs(2)), Ok(true));
    assert_eq!(ph.source_id, 1);
    assert_eq!(ph.limb, Limb::RightHand as u8);
    ph.run_for(Duration::from_millis(500));
    std::thread::sleep(Duration::from_millis(30));
    got.clear();
    srv.poll(&mut got);
    assert!(check_times(&ph, &got, 1, 2_000_000) > 60);
    let d = &srv.devices()[0];
    assert!(d.connected);
    assert_eq!(srv.stats()[0].reconnects, 1);

    // Corte con el mismo socket: el simulador re-empareja solo al no oír al PC 3 s.
    std::thread::sleep(Duration::from_millis(3300));
    assert!(!srv.devices()[0].connected);
    ph.run_for(Duration::from_millis(800));
    assert!(srv.devices()[0].connected);
    std::thread::sleep(Duration::from_millis(30));
    got.clear();
    srv.poll(&mut got);
    assert!(check_times(&ph, &got, 1, 2_000_000) > 50);
    srv.stop();
    assert!(!srv.devices()[0].connected);
}
