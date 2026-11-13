# Haellvorb

Reverb (VST3/CLAP) plugin that injects Wilhelm screams at random, changing level thresholds. Scream playback rate is pitch-fitted to detected input pitch.

## Audio engine
- Algorithmic reverb (Schroeder/comb+allpass) reused/extracted as the core effect.
- RT-safe steady-state processing: scream samples are pre-decoded/resampled; triggering, envelope tracking, and pitch detection run without allocation/IO/locking in the audio path.

## Scream behavior
- Multiple embedded CC0 scream samples.
- Input level envelope is tracked and compared against a threshold that randomizes/changes over time (Sensitivity/Chaos controls).
- On threshold crossing, a random scream is triggered and its playback rate is adjusted (pitch-fit) based on detected input pitch vs. the sample’s estimated base pitch.

## Samples (CC0 / public domain)
- `wilhelm1.ogg`: Wikimedia Commons “Wilhelm Scream.ogg” (CC0 1.0 / public domain dedication).
- `wilhelm4.wav`, `wilhelm_male.wav`, `wilhelm_woman.wav`: USC Optical/Sunset Editorial Sound Effects Library (VOICES), via Internet Archive (CC0).

## Build
Requires Rust toolchain and `cargo-nice-plug` (nice-plug bundler).

```powershell
cargo nice-plug bundle haellvorb-plugin --release
```

Outputs:
- `target/bundled/haellvorb-plugin.vst3`
- `target/bundled/haellvorb-plugin.clap`

## Repo
- Gitea: https://gitea.herzog-lokal.de/slopfactory/haellvorb
