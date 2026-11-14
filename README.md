# Haellvorb

Haellvorb is a dark, reverb-first **VST3/CLAP** audio plugin for Linux with an ominous algorithmic space, input-reactive particles, and optional Wilhelm scream injections.

![Haellvorb UI](docs/screenshot.webp)

## Features
- **Algorithmic reverb:** comb/allpass tank with calibrated decay (RT60-based feedback), pre-delay, damping, and dry/wet mix.
- **Tone/texture:** post-tank mid-band distortion (~300–2500 Hz) and random dark particle bursts gated by input presence.
- **Wilhelm screams (optional):** embedded CC0 samples triggered from input activity with adjustable **Scream** amount, **Sensitivity**, and **Chaos**; playback is gated to actual input presence.
- **Controls:** big **Decay** knob plus **Mix**, **Pre-delay**, **Damping**, **Input Gain**, **Output Gain**, **Sensitivity**, **Chaos**, and **Scream**. Knob values are shown only while adjusting.
- **GUI:** fixed-base layout with lower-right corner scaling; UI and plugin window scale together while preserving the startup aspect ratio.
- **RT-minded processing:** steady-state audio path avoids allocation/IO/locking (samples pre-decoded/resampled).

## Samples (CC0 / public domain)
- `wilhelm1.ogg`: Wikimedia Commons “Wilhelm Scream.ogg” (CC0 1.0 / public domain dedication).
- `wilhelm4.wav`, `wilhelm_male.wav`, `wilhelm_woman.wav`: USC Optical/Sunset Editorial Sound Effects Library (VOICES), via Internet Archive (CC0).

## Build (Linux)
Requires Rust toolchain and `cargo-nice-plug` (nice-plug bundler).

```bash
# Debug bundle (as used for current releases)
cargo nice-plug bundle haellvorb-plugin

# Release bundle
cargo nice-plug bundle haellvorb-plugin --release
```

Outputs:
- `target/bundled/haellvorb-plugin.vst3`
- `target/bundled/haellvorb-plugin.clap`

## Releases
- Latest: **v0.3.0 (Linux, debug)** — https://gitea.herzog-lokal.de/slopfactory/haellvorb/releases/tag/v0.3.0
- Repo: https://gitea.herzog-lokal.de/slopfactory/haellvorb

