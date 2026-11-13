//! Wilvorb: reverb plugin with random Wilhelm scream triggers.

use nice_plug::prelude::*;
use nice_plug_egui::EguiState;
use std::num::NonZeroU32;
use std::sync::Arc;

use wilvorb_dsp::chain::Chain;

mod gui;

const DEFAULT_GUI_SIZE: (u32, u32) = (640, 360);

// Embedded CC0 scream samples (OGG/Vorbis and WAV).
const SCREAM_OGG_1: &[u8] = include_bytes!("../assets/screams/wilhelm1.ogg");
const SCREAM_WAV_4: &[u8] = include_bytes!("../assets/screams/wilhelm4.wav");
const SCREAM_WAV_M: &[u8] = include_bytes!("../assets/screams/wilhelm_male.wav");
const SCREAM_WAV_W: &[u8] = include_bytes!("../assets/screams/wilhelm_woman.wav");

#[derive(Params)]
struct WilvorbParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "in_gain"]
    pub in_gain: FloatParam,
    #[id = "out_gain"]
    pub out_gain: FloatParam,

    // Reverb
    #[id = "rev_on"]
    pub rev_on: BoolParam,
    #[id = "rev_mix"]
    pub rev_mix: FloatParam,
    #[id = "rev_decay"]
    pub rev_decay: FloatParam,
    #[id = "rev_predelay"]
    pub rev_predelay: FloatParam,
    #[id = "rev_damp"]
    pub rev_damp: FloatParam,

    // Scream
    #[id = "scream_on"]
    pub scream_on: BoolParam,
    #[id = "scream_sens"]
    pub scream_sens: FloatParam,
    #[id = "scream_change"]
    pub scream_change: FloatParam,
    #[id = "scream_gain"]
    pub scream_gain: FloatParam,
}

impl Default for WilvorbParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(DEFAULT_GUI_SIZE.0, DEFAULT_GUI_SIZE.1),
            in_gain: FloatParam::new("Input Gain", 0.0, FloatRange::Linear { min: -24.0, max: 24.0 }).with_unit(" dB"),
            out_gain: FloatParam::new("Output Gain", 0.0, FloatRange::Linear { min: -24.0, max: 24.0 }).with_unit(" dB"),

            rev_on: BoolParam::new("Reverb", true),
            rev_mix: FloatParam::new("Mix", 0.35, FloatRange::Linear { min: 0.0, max: 1.0 }),
            rev_decay: FloatParam::new("Decay", 2.5, FloatRange::Linear { min: 0.1, max: 10.0 }).with_unit(" s"),
            rev_predelay: FloatParam::new("Pre-delay", 30.0, FloatRange::Linear { min: 0.0, max: 250.0 }).with_unit(" ms"),
            rev_damp: FloatParam::new("Damping", 0.35, FloatRange::Linear { min: 0.0, max: 1.0 }),

            scream_on: BoolParam::new("Wilhelm", true),
            scream_sens: FloatParam::new("Sensitivity", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            scream_change: FloatParam::new("Chaos", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            scream_gain: FloatParam::new("Scream Level", 0.7, FloatRange::Linear { min: 0.0, max: 2.0 }),
        }
    }
}

struct Wilvorb {
    params: Arc<WilvorbParams>,
    chain: Chain,
    sample_rate: f64,
    samples_loaded: bool,
}

impl Default for Wilvorb {
    fn default() -> Self {
        Self {
            params: Arc::new(WilvorbParams::default()),
            chain: Chain::new(44100.0),
            sample_rate: 44100.0,
            samples_loaded: false,
        }
    }
}

impl Wilvorb {
    fn ensure_samples(&mut self) {
        if self.samples_loaded { return; }
        let mut bank: Vec<(Vec<f32>, f32)> = Vec::new();

        // OGG samples
        if let Some((pcm, base_hz, src_sr)) = wilvorb_dsp::scream::decode_ogg_mono(SCREAM_OGG_1) {
            let pcm = wilvorb_dsp::scream::resample_linear(&pcm, src_sr as f64, self.sample_rate);
            let pitch = if src_sr as f64 > 0.0 { base_hz * (self.sample_rate as f32 / src_sr as f32) } else { base_hz };
            bank.push((pcm, pitch));
        }
        // WAV samples
        for bytes in [SCREAM_WAV_4, SCREAM_WAV_M, SCREAM_WAV_W] {
            if let Some((pcm, base_hz, src_sr)) = wilvorb_dsp::scream::decode_wav_mono(bytes) {
                let pcm = wilvorb_dsp::scream::resample_linear(&pcm, src_sr as f64, self.sample_rate);
                let pitch = if src_sr as f64 > 0.0 { base_hz * (self.sample_rate as f32 / src_sr as f32) } else { base_hz };
                bank.push((pcm, pitch));
            }
        }

        if !bank.is_empty() {
            self.chain.set_scream_samples(bank);
        }
        self.samples_loaded = true;
    }
}

impl Plugin for Wilvorb {
    const NAME: &'static str = "Wilvorb";
    const VENDOR: &'static str = "slopfactory";
    const URL: &'static str = "https://gitea.herzog-lokal.de/slopfactory/wilvorb";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> { self.params.clone() }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        gui::create(self.params.clone(), self.params.editor_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate as f64;
        self.chain.set_sample_rate(self.sample_rate);
        self.samples_loaded = false;
        self.ensure_samples();
        true
    }

    fn reset(&mut self) { self.chain.reset(); }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.ensure_samples();
        let p = &self.params;
        let cp = self.chain.params_mut();
        cp.in_gain_db = p.in_gain.value();
        cp.out_gain_db = p.out_gain.value();
        cp.reverb_on = p.rev_on.value();
        cp.reverb_mix = p.rev_mix.value();
        cp.reverb_decay = p.rev_decay.value();
        cp.reverb_predelay = p.rev_predelay.value() / 1000.0;
        cp.reverb_damping = p.rev_damp.value();
        cp.scream_on = p.scream_on.value();
        cp.scream_sensitivity = p.scream_sens.value();
        cp.scream_change = p.scream_change.value();
        cp.scream_gain = p.scream_gain.value();

        let mut channel_slices = buffer.as_slice();
        let mut refs: Vec<&mut [f32]> = channel_slices.iter_mut().map(|c| &mut c[..]).collect();
        self.chain.process(&mut refs);
        ProcessStatus::Normal
    }
}

impl ClapPlugin for Wilvorb {
    const CLAP_ID: &'static str = "com.slopfactory.wilvorb";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Reverb with random Wilhelm scream triggers");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo, ClapFeature::Reverb];
}

impl Vst3Plugin for Wilvorb {
    const VST3_CLASS_ID: [u8; 16] = *b"WilvorbScreamRev";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx, Vst3SubCategory::Reverb];
}

nice_plug::nice_export_clap!(Wilvorb);
nice_plug::nice_export_vst3!(Wilvorb);
