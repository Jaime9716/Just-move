//! Motor de alto nivel: señal -> rasgos -> juez para una canción. AGENTE C.
//! Lo usan jd_capi (API C), jd_tools y la GDExtension.
use crate::features::make_feature_extractor;
use crate::judge::make_judge;
use crate::signal::make_signal_processor;
use crate::types::{
    Choreo, FeatureExtractor, FeatureFrame, FilteredSample, Judge, JudgeConfig, MotionSample, Session,
    SignalProcessor, StepResult,
};
use std::path::Path;

/// Rasgos de una serie completa de muestras (tiempo: t_ns·1e-9 + mono_to_song_s), con flush.
/// Las muestras se ordenan de forma estable por t_ns antes de procesarlas.
pub fn compute_features(samples: &[MotionSample], mono_to_song_s: f64) -> Vec<FeatureFrame> {
    let mut sorted: Vec<MotionSample> = samples.to_vec();
    sorted.sort_by_key(|s| s.t_ns);
    let mut sp = make_signal_processor();
    let mut fx = make_feature_extractor();
    let mut filtered = Vec::with_capacity(sorted.len());
    sp.process(&sorted, mono_to_song_s, &mut filtered);
    let mut frames = Vec::new();
    fx.push(&filtered, &mut frames);
    fx.flush(&mut frames);
    frames
}

/// Juicio offline completo (equivale a `jdproto.pipeline.judge_song`): todas las muestras del
/// jugador en tiempo de canción, resultados en orden de paso y extremidad.
pub fn judge_offline(choreo: &Choreo, reference_frames: &[FeatureFrame], player: &[MotionSample], cfg: &JudgeConfig) -> Vec<StepResult> {
    let frames = compute_features(player, 0.0);
    let mut j = make_judge();
    j.load(choreo, reference_frames, cfg);
    j.push(&frames);
    let mut out = Vec::new();
    j.update(f64::MAX, &mut out);
    out
}

pub struct Engine {
    cfg: JudgeConfig,
    choreo: Option<Choreo>,
    reference: Vec<FeatureFrame>,
    mono_to_song_s: f64,
    signal: Box<dyn SignalProcessor>,
    features: Box<dyn FeatureExtractor>,
    judge: Box<dyn Judge>,
    filtered: Vec<FilteredSample>,
    frames: Vec<FeatureFrame>,
}

impl Engine {
    pub fn new() -> Engine {
        Engine {
            cfg: JudgeConfig::default(),
            choreo: None,
            reference: Vec::new(),
            mono_to_song_s: 0.0,
            signal: make_signal_processor(),
            features: make_feature_extractor(),
            judge: make_judge(),
            filtered: Vec::new(),
            frames: Vec::new(),
        }
    }

    /// Cambia las ventanas del juez. Si hay canción cargada, vuelve a empezar la partida.
    pub fn set_config(&mut self, cfg: JudgeConfig) {
        self.cfg = cfg;
        if self.choreo.is_some() { self.reload_judge(); }
    }

    pub fn config(&self) -> JudgeConfig { self.cfg }

    /// Carga <dir>/choreo.json y, si existe, la referencia IMU (rasgos precalculados).
    pub fn load_song(&mut self, song_dir: &Path) -> Result<(), String> {
        let choreo = crate::choreo::load_choreo(&song_dir.join("choreo.json"))?;
        let reference = if choreo.ref_imu.is_empty() {
            Vec::new()
        } else {
            let path = song_dir.join(&choreo.ref_imu);
            let s = crate::session::load_session(&path)
                .map_err(|e| format!("referencia IMU {}: {}", path.display(), e))?;
            reference_frames(&s)
        };
        self.load_choreo(choreo, reference);
        Ok(())
    }

    /// Carga una coreografía ya construida (y rasgos de referencia en tiempo de canción).
    pub fn load_choreo(&mut self, choreo: Choreo, reference: Vec<FeatureFrame>) {
        self.choreo = Some(choreo);
        self.reference = reference;
        self.reload_judge();
    }

    fn reload_judge(&mut self) {
        self.reset();
        if let Some(c) = &self.choreo { self.judge.load(c, &self.reference, &self.cfg); }
    }

    pub fn choreo(&self) -> Option<&Choreo> { self.choreo.as_ref() }
    pub fn reference_frames(&self) -> &[FeatureFrame] { &self.reference }

    pub fn set_time_mapping(&mut self, mono_to_song_s: f64) { self.mono_to_song_s = mono_to_song_s; }

    pub fn push_samples(&mut self, s: &[MotionSample]) {
        if s.is_empty() { return; }
        self.filtered.clear();
        self.signal.process(s, self.mono_to_song_s, &mut self.filtered);
        self.frames.clear();
        self.features.push(&self.filtered, &mut self.frames);
        if self.choreo.is_some() && !self.frames.is_empty() { self.judge.push(&self.frames); }
    }

    /// Emite las tramas pendientes (fin de datos; útil en reproducción offline).
    pub fn flush(&mut self) {
        self.frames.clear();
        self.features.flush(&mut self.frames);
        if self.choreo.is_some() && !self.frames.is_empty() { self.judge.push(&self.frames); }
    }

    /// Resultados de pasos cuya ventana se cerró hasta song_time_s.
    pub fn update(&mut self, song_time_s: f64, out: &mut Vec<StepResult>) {
        if self.choreo.is_some() { self.judge.update(song_time_s, out); }
    }

    /// Vuelve al inicio de la canción (descarta muestras y resultados pendientes).
    pub fn reset(&mut self) {
        self.signal.reset();
        self.features.reset();
        self.judge.reset();
    }
}

/// Rasgos de una sesión de referencia (time_base "song"; si es "mono" se usa tal cual).
pub fn reference_frames(s: &Session) -> Vec<FeatureFrame> { compute_features(&s.samples, 0.0) }

impl Default for Engine { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Grade, Limb, Step, StepKind, SAMPLE_HAS_ACCEL, SAMPLE_HAS_GYRO};

    fn punches(hits: &[f64], limb: Limb, t1: f64) -> Vec<MotionSample> {
        let n = (t1 / 0.005) as usize;
        (0..n).map(|i| {
            let t = i as f64 * 0.005;
            let mut ay = 9.80665;
            for &h in hits { ay += 20.0 * (-(t - h) * (t - h) / (2.0 * 0.012 * 0.012)).exp(); }
            MotionSample { t_ns: (t * 1e9).round() as i64, source_id: 1, limb: limb as u8,
                flags: SAMPLE_HAS_ACCEL | SAMPLE_HAS_GYRO, _reserved: 0, accel: [0.0, ay as f32, 0.0], gyro: [0.0; 3] }
        }).collect()
    }

    #[test]
    fn engine_realtime_with_mapping() {
        let choreo = Choreo { steps: vec![
            Step { t: 2.0, dur: 0.2, limbs: vec![Limb::RightHand], icon: "x".into(), kind: StepKind::Accent },
            Step { t: 3.0, dur: 0.2, limbs: vec![Limb::RightHand], icon: "x".into(), kind: StepKind::Accent },
        ], ..Default::default() };
        let mut e = Engine::new();
        e.load_choreo(choreo, Vec::new());
        let offset = 10.0; // mono = canción + 10 s
        e.set_time_mapping(-offset);
        let mut s = punches(&[2.0, 3.12], Limb::RightHand, 4.0);
        for x in s.iter_mut() { x.t_ns += (offset * 1e9) as i64; }
        let mut out = Vec::new();
        // Tiempo real: en cada fotograma de 16 ms llegan las muestras de ese intervalo.
        let mut idx = 0;
        let mut t = 0.0;
        while t < 4.5 {
            t += 0.016;
            let start = idx;
            while idx < s.len() && (s[idx].t_ns as f64 * 1e-9 - offset) <= t { idx += 1; }
            e.push_samples(&s[start..idx]);
            e.update(t, &mut out);
        }
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].grade, Grade::Perfect as u8);
        assert_eq!(out[1].grade, Grade::Miss as u8);
        e.reset();
        out.clear();
        e.update(100.0, &mut out);
        assert!(out.iter().all(|r| r.grade == Grade::Miss as u8));
    }

    #[test]
    fn no_song_is_harmless() {
        let mut e = Engine::new();
        e.push_samples(&punches(&[1.0], Limb::LeftHand, 2.0));
        let mut out = Vec::new();
        e.update(10.0, &mut out);
        assert!(out.is_empty());
        assert!(e.load_song(Path::new("/no/existe")).is_err());
        let cfg = JudgeConfig { perfect_window_ms: 30.0, good_window_ms: 80.0, foot_window_scale: 2.0 };
        e.set_config(cfg);
        assert_eq!(e.config(), cfg);
    }
}
