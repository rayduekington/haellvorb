//! Wilhelm scream sample playback with random level-threshold triggering
//! and simple pitch-fitting (playback-rate adjustment).
//!
//! Steady-state processing is RT-safe (no allocation/IO/locking).



/// Maximum simultaneous scream voices.
const MAX_VOICES: usize = 4;
/// Analysis window for input pitch detection (samples).
const PITCH_WINDOW: usize = 1024;

#[derive(Clone)]
struct Sample {
    pcm: Vec<f32>,
    /// Estimated base frequency (Hz) of the sample, 0 if unknown.
    base_hz: f32,
}

struct Voice {
    sample_idx: usize,
    pos: f64,
    step: f64,
    gain: f32,
    active: bool,
}

impl Voice {
    fn new() -> Self {
        Self { sample_idx: 0, pos: 0.0, step: 1.0, gain: 0.0, active: false }
    }
}

/// Simple LCG for RT-safe random numbers.
#[derive(Clone)]
struct Rng(u32);
impl Rng {
    fn new(seed: u32) -> Self { Self(if seed == 0 { 1 } else { seed }) }
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0
    }
    fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32).clamp(0.0, 1.0)
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }
}

pub struct ScreamEngine {
    sample_rate: f64,
    samples: Vec<Sample>,
    voices: [Voice; MAX_VOICES],
    next_voice: usize,

    // Triggering / level
    env: f32,
    env_attack: f32,
    env_release: f32,
    threshold_db: f32,
    threshold_min_db: f32,
    threshold_max_db: f32,
    armed: bool,
    hold_samples: usize,
    hold_counter: usize,

    // Params
    enabled: bool,
    scream_gain: f32,
    sensitivity: f32, // 0..1 maps threshold range
    change_rate: f32, // how often/randomly threshold moves
    change_counter: usize,

    // Pitch detection (input)
    pitch_buf: [f32; PITCH_WINDOW],
    pitch_idx: usize,
    pitch_filled: bool,
    detected_hz: f32,
    pitch_update: usize,

    // Gate screams to actual input activity
    input_hold: usize,

    rng: Rng,
}

impl ScreamEngine {
    pub fn new(sample_rate: f64) -> Self {
        let mut e = Self {
            sample_rate,
            samples: Vec::new(),
            voices: std::array::from_fn(|_| Voice::new()),
            next_voice: 0,
            env: 0.0,
            env_attack: 0.0,
            env_release: 0.0,
            threshold_db: -18.0,
            threshold_min_db: -40.0,
            threshold_max_db: -6.0,
            armed: true,
            hold_samples: (sample_rate * 0.4) as usize,
            hold_counter: 0,
            enabled: true,
            scream_gain: 1.0,
            sensitivity: 0.5,
            change_rate: 0.5,
            change_counter: 0,
            pitch_buf: [0.0; PITCH_WINDOW],
            pitch_idx: 0,
            pitch_filled: false,
            detected_hz: 0.0,
            pitch_update: 0,
            input_hold: 0,
            rng: Rng::new(0xC0FFEE),
        };
        e.update_coeffs();
        e.randomize_threshold();
        e
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        if (sample_rate - self.sample_rate).abs() < 0.5 { return; }
        self.sample_rate = sample_rate;
        self.hold_samples = (sample_rate * 0.4) as usize;
        self.update_coeffs();
        self.reset();
    }

    fn update_coeffs(&mut self) {
        // ~5ms attack, ~200ms release envelope follower
        self.env_attack = (-1.0 / (0.005 * self.sample_rate as f32)).exp();
        self.env_release = (-1.0 / (0.200 * self.sample_rate as f32)).exp();
    }

    pub fn set_params(&mut self, enabled: bool, sensitivity: f32, change_rate: f32, gain: f32) {
        self.enabled = enabled;
        self.sensitivity = sensitivity.clamp(0.0, 1.0);
        self.change_rate = change_rate.clamp(0.0, 1.0);
        // Adjustable scream amount/level.
        self.scream_gain = gain.clamp(0.0, 4.0);
        // sensitivity maps threshold range: high sensitivity -> lower thresholds (trigger easier)
        self.threshold_min_db = -48.0 + (1.0 - self.sensitivity) * 12.0;
        self.threshold_max_db = -24.0 + (1.0 - self.sensitivity) * 18.0;
        if self.threshold_max_db <= self.threshold_min_db + 1.0 {
            self.threshold_max_db = self.threshold_min_db + 6.0;
        }
    }

    /// Replace sample bank (decoded mono PCM, engine sample rate). RT-unsafe: allocates.
    pub fn set_samples(&mut self, samples: Vec<(Vec<f32>, f32)>) {
        self.samples = samples.into_iter().map(|(pcm, base_hz)| Sample { pcm, base_hz }).collect();
        self.reset();
    }

    pub fn reset(&mut self) {
        for v in &mut self.voices { v.active = false; }
        self.env = 0.0;
        self.armed = true;
        self.hold_counter = 0;
        self.pitch_buf.fill(0.0);
        self.pitch_idx = 0;
        self.pitch_filled = false;
        self.detected_hz = 0.0;
        self.pitch_update = 0;
        self.change_counter = 0;
        self.input_hold = 0;
    }

    fn randomize_threshold(&mut self) {
        self.threshold_db = self.rng.range(self.threshold_min_db, self.threshold_max_db);
        // schedule next change: 0.5s .. 6s scaled by (1-change_rate)
        let base = 0.5 + (1.0 - self.change_rate) * 5.5;
        let span = base * 0.8;
        let secs = (base - span * 0.5 + self.rng.next_f32() * span).max(0.2);
        self.change_counter = (secs * self.sample_rate as f32) as usize;
    }

    fn detect_pitch(&mut self) {
        // Simple autocorrelation pitch detection on pitch_buf.
        let n = PITCH_WINDOW;
        let mut mean = 0.0f32;
        for &s in &self.pitch_buf { mean += s; }
        mean /= n as f32;
        // energy check
        let mut energy = 0.0f32;
        for &s in &self.pitch_buf { let d = s - mean; energy += d * d; }
        if energy / (n as f32) < 1e-5 { self.detected_hz = 0.0; return; }

        let sr = self.sample_rate as f32;
        let min_lag = (sr / 1000.0) as usize; // 1kHz max
        let max_lag = (sr / 60.0) as usize;   // 60Hz min
        let max_lag = max_lag.min(n / 2).max(min_lag + 1);
        let mut best_lag = 0usize;
        let mut best_corr = 0.0f32;
        let mut lag = min_lag.max(1);
        while lag < max_lag {
            let mut corr = 0.0f32;
            let mut norm_a = 0.0f32;
            let mut norm_b = 0.0f32;
            let mut i = 0;
            while i + lag < n {
                let a = self.pitch_buf[i] - mean;
                let b = self.pitch_buf[i + lag] - mean;
                corr += a * b;
                norm_a += a * a;
                norm_b += b * b;
                i += 1;
            }
            let denom = (norm_a * norm_b).sqrt();
            let c = if denom > 1e-9 { corr / denom } else { 0.0 };
            if c > best_corr { best_corr = c; best_lag = lag; }
            lag += 1;
        }
        if best_corr > 0.3 && best_lag > 0 {
            self.detected_hz = sr / best_lag as f32;
        } else {
            self.detected_hz = 0.0;
        }
    }

    fn trigger(&mut self) {
        if self.samples.is_empty() { return; }
        let sample_idx = (self.rng.next_u32() as usize) % self.samples.len();
        let base_hz = self.samples[sample_idx].base_hz;
        // Pitch-fit: adjust playback rate so sample pitch approximates detected input pitch.
        let step = if self.detected_hz > 60.0 && base_hz > 60.0 {
            (self.detected_hz / base_hz).clamp(0.5, 2.0) as f64
        } else {
            // small random variation when no pitch detected
            self.rng.range(0.85, 1.15) as f64
        };
        let v = &mut self.voices[self.next_voice];
        self.next_voice = (self.next_voice + 1) % MAX_VOICES;
        v.sample_idx = sample_idx;
        v.pos = 0.0;
        v.step = step;
        v.gain = self.scream_gain;
        v.active = true;
        self.randomize_threshold();
    }

    /// Process in-place stereo buffers and mix in screams.
    /// `input_present` gates triggering/output so screams only occur when actual input is present.
    pub fn process(&mut self, buffers: &mut [&mut [f32]], input_present: bool) {
        if buffers.is_empty() { return; }
        let n = buffers[0].len();
        let stereo = buffers.len() > 1;

        // Track input activity with hold so screams stop shortly after input goes silent.
        if input_present {
            self.input_hold = (self.sample_rate as usize / 2).max(1); // ~0.5s hold
        } else if self.input_hold > 0 {
            // decrease once per block for efficiency
            self.input_hold = self.input_hold.saturating_sub(n);
        }
        let allow_screams = input_present || self.input_hold > 0;

        if !allow_screams {
            // No recent input: stop active screams and skip processing/mixing.
            for v in &mut self.voices { v.active = false; }
            self.env *= 0.9;
            self.armed = true;
            self.hold_counter = 0;
            return;
        }

        for i in 0..n {
            let l = buffers[0][i];
            let r = if stereo { buffers[1][i] } else { l };
            let mono = 0.5 * (l + r);

            // Pitch detection buffer
            self.pitch_buf[self.pitch_idx] = mono;
            self.pitch_idx += 1;
            if self.pitch_idx >= PITCH_WINDOW {
                self.pitch_idx = 0;
                self.pitch_filled = true;
            }
            self.pitch_update += 1;
            if self.pitch_filled && self.pitch_update >= PITCH_WINDOW / 2 {
                self.pitch_update = 0;
                self.detect_pitch();
            }

            // Envelope follower
            let level = mono.abs();
            if level > self.env {
                self.env = self.env_attack * self.env + (1.0 - self.env_attack) * level;
            } else {
                self.env = self.env_release * self.env + (1.0 - self.env_release) * level;
            }
            let env_db = if self.env > 1e-8 { 20.0 * self.env.log10() } else { -120.0 };

            // Threshold change over time
            if self.change_counter > 0 {
                self.change_counter -= 1;
            } else {
                self.randomize_threshold();
            }

            if self.hold_counter > 0 { self.hold_counter -= 1; }

            if self.enabled {
                if self.armed && env_db >= self.threshold_db && self.hold_counter == 0 && input_present {
                    self.trigger();
                    self.armed = false;
                    self.hold_counter = self.hold_samples;
                } else if !self.armed && env_db < self.threshold_db - 3.0 {
                    // re-arm with hysteresis
                    self.armed = true;
                }
            }

            // Mix active voices
            let mut s = 0.0f32;
            for v in &mut self.voices {
                if !v.active { continue; }
                if v.sample_idx >= self.samples.len() { v.active = false; continue; }
                let pcm = &self.samples[v.sample_idx].pcm;
                let idx = v.pos as usize;
                if idx + 1 >= pcm.len() {
                    v.active = false;
                    continue;
                }
                let frac = (v.pos - idx as f64) as f32;
                let a = pcm[idx];
                let b = pcm[idx + 1];
                s += (a + (b - a) * frac) * v.gain;
                v.pos += v.step;
            }

            if s != 0.0 {
                buffers[0][i] = l + s;
                if stereo { buffers[1][i] = r + s; }
            }
        }
    }
}

/// Decode OGG/Vorbis bytes to mono f32 PCM, estimate base pitch (Hz), and return sample rate.
pub fn decode_ogg_mono(bytes: &[u8]) -> Option<(Vec<f32>, f32, u32)> {
    use lewton::inside_ogg::OggStreamReader;
    use std::io::Cursor;
    let mut reader = OggStreamReader::new(Cursor::new(bytes)).ok()?;
    let channels = reader.ident_hdr.audio_channels as usize;
    let sr = reader.ident_hdr.audio_sample_rate;
    if channels == 0 || sr == 0 { return None; }
    let mut out: Vec<f32> = Vec::new();
    while let Ok(Some(pck)) = reader.read_dec_packet_itl() {
        if channels == 1 {
            for s in pck { out.push(s as f32 / 32768.0); }
        } else {
            let mut i = 0;
            while i + channels <= pck.len() {
                let mut sum = 0.0f32;
                for c in 0..channels { sum += pck[i + c] as f32 / 32768.0; }
                out.push(sum / channels as f32);
                i += channels;
            }
        }
    }
    if out.is_empty() { return None; }
    let base_hz = estimate_pitch(&out, sr as f32);
    Some((out, base_hz, sr))
}

/// Decode WAV bytes to mono f32 PCM, estimate base pitch (Hz), and return sample rate.
pub fn decode_wav_mono(bytes: &[u8]) -> Option<(Vec<f32>, f32, u32)> {
    use std::io::Cursor;
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).ok()?;
    let spec = reader.spec();
    let sr = spec.sample_rate;
    let channels = spec.channels.max(1) as usize;
    if sr == 0 { return None; }
    let mut out: Vec<f32> = Vec::new();
    match spec.sample_format {
        hound::SampleFormat::Float => {
            let samples: Vec<f32> = reader.samples::<f32>().filter_map(Result::ok).collect();
            if channels == 1 { out = samples; } else {
                let mut i = 0;
                while i + channels <= samples.len() {
                    let mut sum = 0.0f32;
                    for c in 0..channels { sum += samples[i + c]; }
                    out.push(sum / channels as f32);
                    i += channels;
                }
            }
        }
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample as i32;
            let scale = (1i64 << (bits.saturating_sub(1))) as f32;
            if scale <= 0.0 { return None; }
            let samples: Vec<i32> = reader.samples::<i32>().filter_map(Result::ok).collect();
            if channels == 1 {
                out = samples.into_iter().map(|s| s as f32 / scale).collect();
            } else {
                let mut i = 0;
                while i + channels <= samples.len() {
                    let mut sum = 0.0f32;
                    for c in 0..channels { sum += samples[i + c] as f32 / scale; }
                    out.push(sum / channels as f32);
                    i += channels;
                }
            }
        }
    }
    if out.is_empty() { return None; }
    let base_hz = estimate_pitch(&out, sr as f32);
    Some((out, base_hz, sr))
}

/// Resample mono PCM from src_sr to dst_sr (linear, offline).
pub fn resample_linear(input: &[f32], src_sr: f64, dst_sr: f64) -> Vec<f32> {
    if input.is_empty() || src_sr <= 0.0 || dst_sr <= 0.0 { return Vec::new(); }
    if (src_sr - dst_sr).abs() < 0.5 { return input.to_vec(); }
    let ratio = src_sr / dst_sr;
    let out_len = ((input.len() as f64) / ratio).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);
    let mut pos = 0.0f64;
    for _ in 0..out_len {
        let idx = pos as usize;
        if idx + 1 >= input.len() {
            out.push(*input.last().unwrap_or(&0.0));
        } else {
            let frac = (pos - idx as f64) as f32;
            let a = input[idx];
            let b = input[idx + 1];
            out.push(a + (b - a) * frac);
        }
        pos += ratio;
    }
    out
}

fn estimate_pitch(pcm: &[f32], sr: f32) -> f32 {
    // Estimate dominant pitch using autocorrelation on a middle window.
    let n = 2048.min(pcm.len());
    if n < 256 { return 0.0; }
    let start = (pcm.len().saturating_sub(n)) / 2;
    let buf = &pcm[start..start + n];
    let mut mean = 0.0f32;
    for &s in buf { mean += s; }
    mean /= n as f32;
    let min_lag = (sr / 1200.0) as usize;
    let max_lag = ((sr / 80.0) as usize).min(n / 2).max(min_lag + 1);
    let mut best_lag = 0usize;
    let mut best_corr = 0.0f32;
    let mut lag = min_lag.max(1);
    while lag < max_lag {
        let mut corr = 0.0f32;
        let mut na = 0.0f32;
        let mut nb = 0.0f32;
        let mut i = 0;
        while i + lag < n {
            let a = buf[i] - mean;
            let b = buf[i + lag] - mean;
            corr += a * b;
            na += a * a;
            nb += b * b;
            i += 1;
        }
        let d = (na * nb).sqrt();
        let c = if d > 1e-9 { corr / d } else { 0.0 };
        if c > best_corr { best_corr = c; best_lag = lag; }
        lag += 1;
    }
    if best_corr > 0.25 && best_lag > 0 { sr / best_lag as f32 } else { 0.0 }
}
