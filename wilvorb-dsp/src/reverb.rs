//! Simple algorithmic reverb (Schroeder/Freeverb-style: comb + allpass filters).
//! Preallocated buffers, RT-safe steady-state processing.

/// Tuning for 44.1kHz, scaled to sample rate.
const COMB_TUNINGS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNINGS: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;

/// Maximum pre-delay in seconds.
const MAX_PRE_DELAY_SEC: f64 = 0.5;

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
            mix: 0.3,
            decay: 2.0,
            pre_delay: 0.05,
            damping: 0.3,
            sample_rate,
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
        self.update_params();
    }

    fn update_params(&mut self) {
        // Map decay (0.1..10s) to feedback. Simple mapping.
        let decay = self.decay.clamp(0.1, 10.0);
        let feedback = (0.7 + 0.28 * (1.0 - (-decay / 3.0).exp())).clamp(0.7, 0.98);
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
        self.pre_delay = pre_delay.clamp(0.0, 1.0);
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
        let wet_mix = self.mix * 0.015; // scale to reasonable level
        let dry_mix = 1.0;

        for i in 0..n {
            let in_l = left[i];
            let in_r = right[i];
            let input = (in_l + in_r) * 0.5;

            // Pre-delay
            let pd_idx = (self.pre_delay_write + pd_cap - pd) % pd_cap;
            let pd_in = input;
            self.pre_delay_buf_l[self.pre_delay_write] = pd_in;
            self.pre_delay_buf_r[self.pre_delay_write] = pd_in;
            let rev_in = if pd_cap > 0 { self.pre_delay_buf_l[pd_idx] } else { input };
            self.pre_delay_write = (self.pre_delay_write + 1) % pd_cap;

            // Combs in parallel
            let mut out_l = 0.0f32;
            let mut out_r = 0.0f32;
            for c in &mut self.combs_l { out_l += c.process(rev_in); }
            for c in &mut self.combs_r { out_r += c.process(rev_in); }
            // Allpasses in series
            for a in &mut self.allpasses_l { out_l = a.process(out_l); }
            for a in &mut self.allpasses_r { out_r = a.process(out_r); }

            left[i] = in_l * dry_mix + out_l * wet_mix;
            right[i] = in_r * dry_mix + out_r * wet_mix;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reverb_processes() {
        let mut r = Reverb::new(48000.0);
        r.set_params(true, 0.3, 2.0, 0.05, 0.3);
        let mut left = vec![0.1f32; 1024];
        let mut right = vec![0.1f32; 1024];
        let mut buffers = [left.as_mut_slice(), right.as_mut_slice()];
        r.process(&mut buffers);
        assert!(buffers[0].iter().all(|s| s.is_finite()));
    }
}