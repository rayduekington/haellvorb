//! Signal chain: input -> reverb -> wilhelm screams -> output.

use crate::reverb::Reverb;
use crate::scream::ScreamEngine;

#[derive(Clone, Debug)]
pub struct ChainParams {
    pub in_gain_db: f32,
    pub out_gain_db: f32,
    // Reverb
    pub reverb_on: bool,
    pub reverb_mix: f32,
    pub reverb_decay: f32,
    pub reverb_predelay: f32,
    pub reverb_damping: f32,
    // Scream
    pub scream_on: bool,
    pub scream_sensitivity: f32,
    pub scream_change: f32,
    pub scream_gain: f32,
}

impl Default for ChainParams {
    fn default() -> Self {
        Self {
            in_gain_db: 0.0,
            out_gain_db: 0.0,
            reverb_on: true,
            reverb_mix: 0.35,
            reverb_decay: 2.5,
            reverb_predelay: 0.03,
            reverb_damping: 0.35,
            scream_on: true,
            scream_sensitivity: 0.5,
            scream_change: 0.5,
            scream_gain: 0.7,
        }
    }
}

pub struct Chain {
    sample_rate: f64,
    params: ChainParams,
    reverb: Reverb,
    scream: ScreamEngine,
}

fn db_to_lin(db: f32) -> f32 {
    if !db.is_finite() { return 1.0; }
    10f32.powf(db / 20.0)
}

impl Chain {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            params: ChainParams::default(),
            reverb: Reverb::new(sample_rate),
            scream: ScreamEngine::new(sample_rate),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        if (sample_rate - self.sample_rate).abs() < 0.5 { return; }
        self.sample_rate = sample_rate;
        self.reverb.set_sample_rate(sample_rate);
        self.scream.set_sample_rate(sample_rate);
    }

    pub fn params_mut(&mut self) -> &mut ChainParams { &mut self.params }

    /// Replace scream sample bank (RT-unsafe, call off audio thread).
    pub fn set_scream_samples(&mut self, samples: Vec<(Vec<f32>, f32)>) {
        self.scream.set_samples(samples);
    }

    pub fn reset(&mut self) {
        self.reverb.reset();
        self.scream.reset();
    }

    pub fn process(&mut self, buffers: &mut [&mut [f32]]) {
        if buffers.is_empty() || buffers[0].is_empty() { return; }
        let p = self.params.clone();
        let in_lin = db_to_lin(p.in_gain_db);
        let out_lin = db_to_lin(p.out_gain_db);

        // Input gain
        for buf in buffers.iter_mut() {
            for s in buf.iter_mut() { *s *= in_lin; }
        }

        // Reverb
        self.reverb.set_params(p.reverb_on, p.reverb_mix, p.reverb_decay, p.reverb_predelay, p.reverb_damping);
        self.reverb.process(buffers);

        // Wilhelm screams (random threshold trigger + pitch-fit)
        self.scream.set_params(p.scream_on, p.scream_sensitivity, p.scream_change, p.scream_gain);
        self.scream.process(buffers);

        // Output gain
        for buf in buffers.iter_mut() {
            for s in buf.iter_mut() { *s *= out_lin; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chain_processes() {
        let mut chain = Chain::new(48000.0);
        let mut l = vec![0.1f32; 512];
        let mut r = vec![0.1f32; 512];
        let mut bufs = [l.as_mut_slice(), r.as_mut_slice()];
        chain.process(&mut bufs);
        assert!(bufs[0].iter().all(|v| v.is_finite()));
    }
}
