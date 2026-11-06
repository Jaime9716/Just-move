//! B7 · RecorderSource: envuelve otra fuente y graba en JSONL. AGENTE A.
//!
//! - `poll()` reenvía a la fuente interior y escribe cada muestra (BufWriter, flush cada 500 ms).
//! - La cabecera se escribe con la primera muestra (o al parar), así ya conoce los dispositivos y
//!   si hay `set_mapping`. Con mapping → `time_base:"song"`, `t = t_ns·1e-9 + mono_to_song_s` (valor
//!   vigente al grabar cada muestra). Sin mapping → `"mono"`, segundos desde `start()`.
//! - Al parar, si aparecieron dispositivos nuevos o cambió alguna extremidad, se reescribe la línea 1.
//! - Si se llama a `set_mapping` cuando la cabecera ya se escribió como "mono", la base no cambia
//!   (no mezclar bases en un archivo); el último mapping se anota en la cabecera (`mono_to_song_s`).
use crate::clock;
use crate::json::{self, Value};
use crate::session::{format_seconds, header_line, now_iso8601_utc};
use crate::types::{DeviceInfo, Limb, MotionSample, MotionSource, Session, SessionDevice};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

const FLUSH_INTERVAL_NS: i64 = 500_000_000;

pub struct RecorderSource {
    inner: Box<dyn MotionSource>,
    path: PathBuf,
    writer: Option<BufWriter<File>>,
    header_written: bool,
    header_devices: Vec<SessionDevice>,
    devices: Vec<SessionDevice>,
    time_base_song: bool,
    mapping: Option<f64>,
    t_start_ns: i64,
    last_flush_ns: i64,
    song_id: String,
    note: String,
    recorded_at: String,
    written: u64,
    error: String,
}

impl RecorderSource {
    /// Crea (trunca) `out_path` de inmediato para detectar errores de ruta al principio.
    pub fn new(inner: Box<dyn MotionSource>, out_path: &Path) -> Result<RecorderSource, String> {
        let f = File::create(out_path).map_err(|e| format!("{}: no se puede crear: {}", out_path.display(), e))?;
        Ok(RecorderSource {
            inner,
            path: out_path.to_path_buf(),
            writer: Some(BufWriter::new(f)),
            header_written: false,
            header_devices: Vec::new(),
            devices: Vec::new(),
            time_base_song: false,
            mapping: None,
            t_start_ns: clock::now_ns(),
            last_flush_ns: clock::now_ns(),
            song_id: String::new(),
            note: String::new(),
            recorded_at: now_iso8601_utc(),
            written: 0,
            error: String::new(),
        })
    }

    pub fn set_song_id(&mut self, id: &str) { self.song_id = id.to_string(); }
    pub fn set_note(&mut self, note: &str) { self.note = note.to_string(); }
    /// `song_time = t_ns·1e-9 + mono_to_song_s` (lo calcula SongClock; llamar cada fotograma).
    pub fn set_mapping(&mut self, mono_to_song_s: f64) {
        if mono_to_song_s.is_finite() { self.mapping = Some(mono_to_song_s); }
    }
    pub fn samples_written(&self) -> u64 { self.written }
    pub fn path(&self) -> &Path { &self.path }
    pub fn inner(&self) -> &dyn MotionSource { self.inner.as_ref() }
    pub fn inner_mut(&mut self) -> &mut dyn MotionSource { self.inner.as_mut() }

    fn merge_devices(&mut self, list: &[DeviceInfo]) {
        for d in list {
            let sd = SessionDevice { source_id: d.source_id, name: d.name.clone(), kind: d.kind.clone(), limb: d.limb };
            match self.devices.iter_mut().find(|x| x.source_id == d.source_id) {
                Some(x) => *x = sd,
                None => self.devices.push(sd),
            }
        }
    }

    fn header_session(&self) -> Session {
        Session {
            time_base: if self.time_base_song { "song".into() } else { "mono".into() },
            song_id: self.song_id.clone(),
            recorded_at: self.recorded_at.clone(),
            note: self.note.clone(),
            devices: self.devices.clone(),
            samples: Vec::new(),
        }
    }

    fn header_text(&self) -> String {
        let line = header_line(&self.header_session());
        match (self.time_base_song, self.mapping) {
            (false, Some(m)) => {
                // Anota el mapping disponible al final (campo extra, ignorado por los lectores).
                let mut v = json::parse(&line).unwrap_or_default();
                v.set("mono_to_song_s", Value::Number(m));
                v.set("mono_start_ns", Value::Number(self.t_start_ns as f64));
                v.dump()
            }
            _ => line,
        }
    }

    fn write_header(&mut self) {
        if self.header_written { return; }
        self.time_base_song = self.mapping.is_some();
        let text = self.header_text();
        self.header_devices = self.devices.clone();
        self.header_written = true;
        if let Some(w) = self.writer.as_mut() {
            if let Err(e) = writeln!(w, "{}", text) { self.error = format!("{}: {}", self.path.display(), e); }
        }
    }

    fn write_samples(&mut self, samples: &[MotionSample]) {
        if samples.is_empty() { return; }
        self.write_header();
        let song = self.time_base_song;
        let map = self.mapping.unwrap_or(0.0);
        let t0 = self.t_start_ns;
        let mut err = None;
        if let Some(w) = self.writer.as_mut() {
            for s in samples {
                let t = if song { s.t_ns as f64 * 1e-9 + map } else { (s.t_ns - t0) as f64 * 1e-9 };
                let line = crate::session::sample_line_with_t(&format_seconds(t), s);
                if let Err(e) = writeln!(w, "{}", line) { err = Some(e); break; }
                self.written += 1;
            }
        }
        if let Some(e) = err { self.error = format!("{}: error de escritura: {}", self.path.display(), e); }
        let now = clock::now_ns();
        if now - self.last_flush_ns >= FLUSH_INTERVAL_NS {
            self.last_flush_ns = now;
            if let Some(w) = self.writer.as_mut() { let _ = w.flush(); }
        }
    }

    /// Cierra el archivo: escribe la cabecera si falta y la reescribe si cambiaron los dispositivos.
    pub fn finish(&mut self) -> Result<(), String> {
        self.merge_devices(&self.inner.devices());
        self.write_header();
        if let Some(mut w) = self.writer.take() {
            w.flush().map_err(|e| format!("{}: {}", self.path.display(), e))?;
        } else {
            return Ok(());
        }
        let need_rewrite = self.devices != self.header_devices || (!self.time_base_song && self.mapping.is_some());
        if need_rewrite { self.rewrite_header()?; }
        Ok(())
    }

    fn rewrite_header(&mut self) -> Result<(), String> {
        let io = |e: std::io::Error| format!("{}: reescribiendo cabecera: {}", self.path.display(), e);
        let tmp = self.path.with_extension("jsonl.tmp");
        {
            let mut r = BufReader::new(File::open(&self.path).map_err(io)?);
            let mut w = BufWriter::new(File::create(&tmp).map_err(io)?);
            let mut first = Vec::new();
            r.read_until(b'\n', &mut first).map_err(io)?;
            writeln!(w, "{}", self.header_text()).map_err(io)?;
            std::io::copy(&mut r, &mut w).map_err(io)?;
            w.flush().map_err(io)?;
        }
        std::fs::rename(&tmp, &self.path).map_err(io)?;
        self.header_devices = self.devices.clone();
        Ok(())
    }
}

impl MotionSource for RecorderSource {
    fn start(&mut self) -> Result<(), String> {
        if self.writer.is_none() {
            // Reabrir tras stop(): nueva grabación sobre el mismo archivo.
            let f = File::create(&self.path).map_err(|e| format!("{}: no se puede crear: {}", self.path.display(), e))?;
            self.writer = Some(BufWriter::new(f));
            self.header_written = false;
            self.written = 0;
            self.recorded_at = now_iso8601_utc();
        }
        self.t_start_ns = clock::now_ns();
        self.last_flush_ns = self.t_start_ns;
        self.inner.start()
    }

    fn stop(&mut self) {
        self.inner.stop();
        if let Err(e) = self.finish() { self.error = e; }
    }

    fn poll(&mut self, out: &mut Vec<MotionSample>) -> usize {
        let from = out.len();
        let n = self.inner.poll(out);
        if self.writer.is_some() {
            if n > 0 || !self.header_written {
                let d = self.inner.devices();
                self.merge_devices(&d);
            }
            let new: Vec<MotionSample> = out[from..].to_vec();
            self.write_samples(&new);
        }
        n
    }

    fn devices(&self) -> Vec<DeviceInfo> { self.inner.devices() }
    fn assign_limb(&mut self, source_id: u32, limb: Limb) { self.inner.assign_limb(source_id, limb); }
    fn last_error(&self) -> String {
        let inner = self.inner.last_error();
        if !self.error.is_empty() { self.error.clone() } else { inner }
    }
}

impl Drop for RecorderSource {
    fn drop(&mut self) {
        if self.writer.is_some() { let _ = self.finish(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::ReplaySource;
    use crate::session::load_session;
    use crate::types::{SAMPLE_HAS_ACCEL, SAMPLE_HAS_GYRO};
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;

    fn src_session() -> Session {
        let mut s = Session { time_base: "song".into(), ..Default::default() };
        s.devices.push(SessionDevice { source_id: 3, name: "X".into(), kind: "phone".into(), limb: Limb::LeftFoot });
        for k in 0..50 {
            s.samples.push(MotionSample { t_ns: k * 5_000_000, source_id: 3, limb: Limb::LeftFoot as u8,
                flags: SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO, _reserved: 0,
                accel: [k as f32 * 0.5, 9.81, -1.0], gyro: [0.1, 0.2, k as f32] });
        }
        s
    }

    #[test]
    fn record_replay_song_time() {
        let c = Arc::new(AtomicI64::new(10_000_000_000));
        let c2 = c.clone();
        let replay = ReplaySource::new(src_session(), 1.0).with_clock(Box::new(move || c2.load(Ordering::SeqCst)));
        let p = std::env::temp_dir().join(format!("jd_a_rec_{}.jsonl", std::process::id()));
        let mut rec = RecorderSource::new(Box::new(replay), &p).unwrap();
        rec.set_song_id("cancion");
        rec.set_mapping(-10.0); // t0 de reproducción = 10 s mono -> canción 0
        rec.start().unwrap();
        let mut out = Vec::new();
        for _ in 0..30 { c.fetch_add(10_000_000, Ordering::SeqCst); rec.poll(&mut out); }
        assert_eq!(out.len(), 50);
        rec.assign_limb(3, Limb::RightHand);
        rec.stop();
        let back = load_session(&p).unwrap();
        assert_eq!(back.time_base, "song");
        assert_eq!(back.song_id, "cancion");
        assert_eq!(back.samples.len(), 50);
        // Cabecera reescrita con la extremidad nueva.
        assert_eq!(back.devices[0].limb, Limb::RightHand);
        let orig = src_session();
        for (a, b) in orig.samples.iter().zip(back.samples.iter()) {
            assert!((a.t_ns - b.t_ns).abs() <= 1, "{} vs {}", a.t_ns, b.t_ns);
            assert_eq!(a.accel, b.accel);
            assert_eq!(a.gyro, b.gyro);
            assert_eq!(a.flags, b.flags);
        }
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn record_mono_relative() {
        let replay = ReplaySource::new(src_session(), 1000.0);
        let p = std::env::temp_dir().join(format!("jd_a_rec_mono_{}.jsonl", std::process::id()));
        {
            let mut rec = RecorderSource::new(Box::new(replay), &p).unwrap();
            rec.start().unwrap();
            let mut out = Vec::new();
            let t_end = clock::now_ns() + 2_000_000_000;
            while out.len() < 50 && clock::now_ns() < t_end { rec.poll(&mut out); std::thread::sleep(std::time::Duration::from_millis(1)); }
            assert_eq!(out.len(), 50);
            // drop -> finish()
        }
        let back = load_session(&p).unwrap();
        assert_eq!(back.time_base, "mono");
        assert_eq!(back.samples.len(), 50);
        assert!(back.samples[0].t_ns >= 0 && back.samples[0].t_ns < 1_000_000_000);
        let span = back.samples[49].t_ns - back.samples[0].t_ns;
        assert!((span - 245_000).abs() <= 2, "{}", span); // 245 ms / 1000
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn bad_path() {
        let replay = ReplaySource::new(src_session(), 1.0);
        assert!(RecorderSource::new(Box::new(replay), Path::new("/no/existe/x.jsonl")).is_err());
    }
}
