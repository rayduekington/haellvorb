//! Simple algorithmic reverb (Schroeder/Freeverb-style: comb + allpass filters)
//! with post-tank mid-band distortion and sparse random particles.
//! Preallocated buffers, RT-safe steady-state processing.

/// Tuning for 44.1kHz, scaled to sample rate.
const COMB_TUNINGS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNINGS: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;

/// Maximum pre-delay in seconds.
const MAX_PRE_DELAY_SEC: f64 = 0.5;

const PARTICLE_VOICES: usize = 8;

#[derive(Clone, Copy)]
struct Particle {
    active: bool,
    env: f32,
    decay: f32,
    gain: f32,
    pan: f32, // 0..1 -> left/right mix for injection
    tone_lp: f32,
}

impl Particle {
    fn new() -> Self {
        Self {
            active: false,
            env: 0.0,
            decay: 0.9995,
            gain: 0.0,
            pan: 0.5,
            tone_lp: 0.0,
        }
    }
}

#[derive(Clone)]
struct Rng(u32);
impl Rng {
    fn new(seed: u32) -> Self { Self(if seed == 0 { 1 } else { seed }) }
    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0
    }
    #[inline]
    fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32).clamp(0.0, 1.0)
    }
    #[inline]
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }
}

struct Comb {
    buf: Vec<f32>,
    idx: usize,
    filter_store: f32,
    damp1: f32,
    damp2: f32,
    feedback: f32,
}

impl Comb {
    fn new(size: usize) -> Self {
        Self {
            buf: vec![0.0; size.max(1)],
            idx: 0,
            filter_store: 0.0,
            damp1: 0.0,
            damp2: 1.0,
            feedback: 0.0,
        }
    }
    fn set_damp(&mut self, val: f32) {
        self.damp1 = val;
        self.damp2 = 1.0 - val;
    }
    fn set_feedback(&mut self, val: f32) {
        self.feedback = val;
    }
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buf[self.idx];
        self.filter_store = output * self.damp2 + self.filter_store * self.damp1;
        self.buf[self.idx] = input + self.filter_store * self.feedback;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        output
    }
    fn mute(&mut self) {
        self.buf.fill(0.0);
        self.filter_store = 0.0;
        self.idx = 0;
    }
}

struct AllPass {
    buf: Vec<f32>,
    idx: usize,
}

impl AllPass {
    fn new(size: usize) -> Self {
        Self {
            buf: vec![0.0; size.max(1)],
            idx: 0,
        }
    }
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buf[self.idx];
        let output = -input + bufout;
        self.buf[self.idx] = input + bufout * 0.5;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        output
    }
    fn mute(&mut self) {
        self.buf.fill(0.0);
        self.idx = 0;
    }
}

pub struct Reverb {
    combs_l: Vec<Comb>,
    combs_r: Vec<Comb>,
    allpasses_l: Vec<AllPass>,
    allpasses_r: Vec<AllPass>,
    pre_delay_buf_l: Vec<f32>,
    pre_delay_buf_r: Vec<f32>,
    pre_delay_write: usize,
    pre_delay_samples: usize,
    on: bool,
    mix: f32,
    decay: f32,
    pre_delay: f32,
    damping: f32,
    sample_rate: f64,

    // Mid-band distortion (post-tank, per channel)
    mid_lp1_l: f32,
    mid_lp2_l: f32,
    mid_lp1_r: f32,
    mid_lp2_r: f32,
    mid_lp1_coeff: f32,
    mid_lp2_coeff: f32,
    mid_drive: f32,

    // Random particles
    particles: [Particle; PARTICLE_VOICES],
    next_particle: usize,
    particle_timer: usize,
    rng: Rng,

    // Input activity gate for particles (avoid fluttering in silence)
    input_env: f32,
    input_hold: usize,
}

#[inline]
fn one_pole_lp(x: f32, state: &mut f32, coeff: f32) -> f32 {
    *state += coeff * (x - *state);
    *state
}

impl Reverb {
    pub fn new(sample_rate: f64) -> Self {
        let mut r = Self {
            combs_l: Vec::new(),
            combs_r: Vec::new(),
            allpasses_l: Vec::new(),
            allpasses_r: Vec::new(),
            pre_delay_buf_l: Vec::new(),
            pre_delay_buf_r: Vec::new(),
            pre_delay_write: 0,
            pre_delay_samples: 0,
            on: false,
            mix: 0.2,
            decay: 2.0,
            pre_delay: 0.05,
            damping: 0.3,
            sample_rate,

            mid_lp1_l: 0.0,
            mid_lp2_l: 0.0,
            mid_lp1_r: 0.0,
            mid_lp2_r: 0.0,
            mid_lp1_coeff: 0.0,
            mid_lp2_coeff: 0.0,
            mid_drive: 3.5,

            particles: [Particle::new(); PARTICLE_VOICES],
            next_particle: 0,
            particle_timer: 0,
            rng: Rng::new(0xA17B_52C3),

            input_env: 0.0,
            input_hold: 0,
        };
        r.configure(sample_rate);
        r
    }

    fn configure(&mut self, sample_rate: f64) {
        let scale = (sample_rate / 44100.0).max(0.1);
        self.combs_l.clear();
        self.combs_r.clear();
        self.allpasses_l.clear();
        self.allpasses_r.clear();
        for &t in &COMB_TUNINGS {
            let size = ((t as f64) * scale).round() as usize;
            self.combs_l.push(Comb::new(size));
            self.combs_r.push(Comb::new(size + STEREO_SPREAD));
        }
        for &t in &ALLPASS_TUNINGS {
            let size = ((t as f64) * scale).round() as usize;
            self.allpasses_l.push(AllPass::new(size));
            self.allpasses_r.push(AllPass::new(size + STEREO_SPREAD));
        }
        let pd_cap = ((sample_rate * MAX_PRE_DELAY_SEC).ceil() as usize).max(1);
        self.pre_delay_buf_l = vec![0.0; pd_cap];
        self.pre_delay_buf_r = vec![0.0; pd_cap];
        self.pre_delay_write = 0;
        self.update_tone_coeffs(sample_rate);
        self.schedule_next_particle();
        self.update_params();
    }

    fn update_tone_coeffs(&mut self, sample_rate: f64) {
        let sr = sample_rate.max(1.0) as f32;
        // Mid band roughly 300 Hz .. 2500 Hz
        let fc1 = 300.0f32;
        let fc2 = 2500.0f32;
        self.mid_lp1_coeff = (-2.0 * std::f32::consts::PI * fc1 / sr).exp();
        self.mid_lp1_coeff = 1.0 - self.mid_lp1_coeff;
        self.mid_lp2_coeff = (-2.0 * std::f32::consts::PI * fc2 / sr).exp();
        self.mid_lp2_coeff = 1.0 - self.mid_lp2_coeff;
    }

    fn schedule_next_particle(&mut self) {
        // More particles: ~20-180 ms between triggers on average
        let min_s = 0.02f32;
        let max_s = 0.18f32;
        let secs = self.rng.range(min_s, max_s);
        self.particle_timer = (secs * self.sample_rate as f32).max(1.0) as usize;
    }

    fn spawn_particle(&mut self) {
        let i = self.next_particle;
        self.next_particle = (self.next_particle + 1) % PARTICLE_VOICES;
        let p = &mut self.particles[i];
        p.active = true;
        // Quick attack/decay noise burst
        p.env = self.rng.range(0.015, 0.06); // audible but controlled
        // 10-80 ms decay depending on random
        let decay_ms = self.rng.range(10.0, 80.0);
        let samples = (decay_ms * 0.001 * self.sample_rate as f32).max(8.0);
        p.decay = (-1.0f32 / samples).exp();
        p.gain = self.rng.range(0.6, 1.2);
        p.pan = self.rng.next_f32();
        p.tone_lp = 0.0;
    }

    fn update_params(&mut self) {
        // Map decay (seconds, RT60-ish) to feedback based on average comb delay.
        let decay = self.decay.clamp(0.1, 10.0);
        let avg_delay_sec = if !self.combs_l.is_empty() {
            let sum: usize = self.combs_l.iter().map(|c| c.buf.len()).sum();
            (sum as f32 / self.combs_l.len() as f32) / self.sample_rate.max(1.0) as f32
        } else {
            0.03
        };
        let feedback = 10f32.powf(-3.0 * avg_delay_sec / decay).clamp(0.02, 0.995);
        let damp = self.damping.clamp(0.0, 1.0);
        for c in &mut self.combs_l {
            c.set_feedback(feedback);
            c.set_damp(damp);
        }
        for c in &mut self.combs_r {
            c.set_feedback(feedback);
            c.set_damp(damp);
        }
        let pd = (self.pre_delay.clamp(0.0, MAX_PRE_DELAY_SEC as f32) as f64 * self.sample_rate) as usize;
        self.pre_delay_samples = pd.min(self.pre_delay_buf_l.len().saturating_sub(1));
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        if (sample_rate - self.sample_rate).abs() < 0.5 {
            return;
        }
        self.sample_rate = sample_rate;
        self.configure(sample_rate);
    }

    pub fn set_params(&mut self, on: bool, mix: f32, decay: f32, pre_delay: f32, damping: f32) {
        self.on = on;
        self.mix = mix.clamp(0.0, 1.0);
        self.decay = decay.clamp(0.1, 10.0);
        self.pre_delay = pre_delay.clamp(0.0, MAX_PRE_DELAY_SEC as f32);
        self.damping = damping.clamp(0.0, 1.0);
        self.update_params();
    }

    pub fn reset(&mut self) {
        for c in &mut self.combs_l { c.mute(); }
        for c in &mut self.combs_r { c.mute(); }
        for a in &mut self.allpasses_l { a.mute(); }
        for a in &mut self.allpasses_r { a.mute(); }
        self.pre_delay_buf_l.fill(0.0);
        self.pre_delay_buf_r.fill(0.0);
        self.pre_delay_write = 0;
        self.mid_lp1_l = 0.0;
        self.mid_lp2_l = 0.0;
        self.mid_lp1_r = 0.0;
        self.mid_lp2_r = 0.0;
        self.particles = [Particle::new(); PARTICLE_VOICES];
        self.next_particle = 0;
        self.schedule_next_particle();
        self.input_env = 0.0;
        self.input_hold = 0;
    }

    pub fn process(&mut self, buffers: &mut [&mut [f32]]) {
        if !self.on || buffers.len() < 2 {
            return;
        }
        let n = buffers[0].len().min(buffers[1].len());
        if n == 0 {
            return;
        }
        let (l_buf, r_buf) = buffers.split_at_mut(1);
        let left = &mut l_buf[0][..n];
        let right = &mut r_buf[0][..n];

        let pd_cap = self.pre_delay_buf_l.len();
        let pd = self.pre_delay_samples.min(pd_cap.saturating_sub(1));
        let mix = self.mix.clamp(0.0, 1.0);
        let wet_mix = mix;
        let dry_mix = 1.0 - mix;

        for i in 0..n {
            let in_l = left[i];
            let in_r = right[i];
            let input = (in_l + in_r) * 0.5;

            // Input activity detection: particles only when input occurs (avoids flutter in silence)
            let abs_in = input.abs();
            if abs_in > self.input_env {
                self.input_env = abs_in;
            } else {
                self.input_env *= 0.9995;
            }
            let threshold = 1e-4f32; // ~ -80 dBFS
            if self.input_env > threshold {
                self.input_hold = (self.sample_rate as usize / 2).max(1); // ~0.5s hold
            } else if self.input_hold > 0 {
                self.input_hold -= 1;
            }
            let particles_active = self.input_hold > 0;

            // Pre-delay
            let pd_idx = (self.pre_delay_write + pd_cap - pd) % pd_cap;
            let pd_in = input;
            self.pre_delay_buf_l[self.pre_delay_write] = pd_in;
            self.pre_delay_buf_r[self.pre_delay_write] = pd_in;
            let rev_in = if pd_cap > 0 { self.pre_delay_buf_l[pd_idx] } else { input };
            self.pre_delay_write = (self.pre_delay_write + 1) % pd_cap;

            // Combs in parallel (normalize sum to avoid excessive wet level/peaking)
            let mut out_l = 0.0f32;
            let mut out_r = 0.0f32;
            for c in &mut self.combs_l { out_l += c.process(rev_in); }
            for c in &mut self.combs_r { out_r += c.process(rev_in); }
            let comb_norm_l = (self.combs_l.len().max(1) as f32).recip();
            let comb_norm_r = (self.combs_r.len().max(1) as f32).recip();
            out_l *= comb_norm_l;
            out_r *= comb_norm_r;
            // Allpasses in series
            for a in &mut self.allpasses_l { out_l = a.process(out_l); }
            for a in &mut self.allpasses_r { out_r = a.process(out_r); }

            // Random particles (more of them): short dark noise bursts into wet, gated by input
            if particles_active {
                if self.particle_timer == 0 {
                    self.spawn_particle();
                    // occasional extra burst for denser particles
                    if self.rng.next_f32() < 0.35 {
                        self.spawn_particle();
                    }
                    self.schedule_next_particle();
                } else {
                    self.particle_timer -= 1;
                }
            } else {
                // keep timer primed so particles can start promptly when input returns
                if self.particle_timer == 0 {
                    self.schedule_next_particle();
                }
            }
            let mut p_l = 0.0f32;
            let mut p_r = 0.0f32;
            for p in &mut self.particles {
                if !p.active { continue; }
                let n0 = self.rng.range(-1.0, 1.0);
                // darken particle
                p.tone_lp += 0.18 * (n0 - p.tone_lp);
                let s = p.tone_lp * p.env * p.gain;
                p_l += s * (1.0 - p.pan);
                p_r += s * p.pan;
                p.env *= p.decay;
                if p.env < 1e-5 {
                    p.active = false;
                    p.env = 0.0;
                }
            }
            if particles_active {
                out_l += p_l;
                out_r += p_r;
            }

            // Post-tank mid-band distortion (more distortion in mids)
            let low_l = one_pole_lp(out_l, &mut self.mid_lp1_l, self.mid_lp1_coeff);
            let band_l = one_pole_lp(out_l, &mut self.mid_lp2_l, self.mid_lp2_coeff);
            let mid_l = band_l - low_l;
            let high_l = out_l - band_l;
            let mid_l_d = (self.mid_drive * mid_l).tanh();
            out_l = low_l + mid_l_d + high_l;

            let low_r = one_pole_lp(out_r, &mut self.mid_lp1_r, self.mid_lp1_coeff);
            let band_r = one_pole_lp(out_r, &mut self.mid_lp2_r, self.mid_lp2_coeff);
            let mid_r = band_r - low_r;
            let high_r = out_r - band_r;
            let mid_r_d = (self.mid_drive * mid_r).tanh();
            out_r = low_r + mid_r_d + high_r;

            // Volume normalization: output gets lower as mix is ramped up
            let norm = 1.0 - 0.5 * mix;
            left[i] = (in_l * dry_mix + out_l * wet_mix) * norm;
            right[i] = (in_r * dry_mix + out_r * wet_mix) * norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reverb_processes() {
        let mut r = Reverb::new(48000.0);
        r.set_params(true, 0.15, 2.0, 0.05, 0.3);
        let mut left = vec![0.1f32; 1024];
        let mut right = vec![0.1f32; 1024];
        let mut buffers = [left.as_mut_slice(), right.as_mut_slice()];
        r.process(&mut buffers);
        assert!(buffers[0].iter().all(|s| s.is_finite()));
    }
}
