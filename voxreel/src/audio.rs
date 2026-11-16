//! Procedural soundtrack: marimba-ish ticks on block landings (rising pentatonic as the build
//! grows, which is the "satisfying" part), a whoosh on cuts and a chime when a build completes.

use anyhow::Result;
use std::io::Write;

pub const RATE: u32 = 48_000;

pub struct Track {
    samples: Vec<f32>,
    /// Final level relative to a normalized mix (0.8 = a bit quieter than full scale).
    pub master: f32,
    /// Also raise quiet mixes to the normalized peak (otherwise only loud ones are turned down).
    pub boost: bool,
}

/// Major pentatonic ladder in semitones from the root, over three octaves.
const PENTA: [i32; 5] = [0, 2, 4, 7, 9];

impl Track {
    pub fn new(seconds: f32) -> Self {
        Self { samples: vec![0.0; (seconds * RATE as f32) as usize + RATE as usize], master: 1.0, boost: false }
    }

    fn add(&mut self, start: f32, len: f32, mut f: impl FnMut(f32) -> f32) {
        let s0 = (start.max(0.0) * RATE as f32) as usize;
        let n = (len * RATE as f32) as usize;
        for i in 0..n {
            let Some(s) = self.samples.get_mut(s0 + i) else { break };
            *s += f(i as f32 / RATE as f32);
        }
    }

    pub fn tick(&mut self, t: f32, progress: f32, gain: f32) {
        let step = (progress.clamp(0.0, 1.0) * 14.0) as i32;
        let semis = PENTA[(step % 5) as usize] + 12 * (step / 5);
        let freq = 392.0 * 2f32.powf(semis as f32 / 12.0);
        let tau = std::f32::consts::TAU;
        self.add(t, 0.18, |x| {
            let env = (-x / 0.045).exp();
            let click = (-x / 0.002).exp() * 0.3;
            gain * env * ((tau * freq * x).sin() + 0.25 * (tau * freq * 4.0 * x).sin() * (-x / 0.01).exp() + click)
        });
    }

    /// Mixes pre-rendered samples (e.g. speech) in at `t`.
    pub fn clip(&mut self, t: f32, samples: &[f32], gain: f32) {
        let s0 = (t.max(0.0) * RATE as f32) as usize;
        for (i, v) in samples.iter().enumerate() {
            let Some(s) = self.samples.get_mut(s0 + i) else { break };
            *s += v * gain;
        }
    }

    pub fn chime(&mut self, t: f32) {
        let tau = std::f32::consts::TAU;
        for (i, f) in [784.0f32, 1175.0, 1568.0, 2349.0].iter().enumerate() {
            let delay = i as f32 * 0.06;
            self.add(t + delay, 1.4, |x| 0.16 * (-x / 0.45).exp() * (tau * f * x).sin());
        }
    }

    // ------------------------------------------------------------------ "how it's made" sounds

    /// Block landing: woody "thock" (low body + resonant click), slightly higher as the build grows.
    pub fn thock(&mut self, t: f32, progress: f32, gain: f32, seed: u32) {
        let tau = std::f32::consts::TAU;
        let mut rng = Rng(seed | 1);
        let jitter = 1.0 + (rng.next() - 0.5) * 0.12;
        let f0 = (140.0 + 90.0 * progress) * jitter;
        let mut bp = Resonator::new(1500.0 + 900.0 * progress * jitter, 0.92);
        self.add(t, 0.12, |x| {
            let body = (tau * f0 * x).sin() * (-x / 0.03).exp();
            let click = bp.run(rng.next() * 2.0 - 1.0) * (-x / 0.008).exp();
            gain * (0.8 * body + 1.6 * click)
        });
    }

    /// Ticks as woody thocks, rate-limited like `build_ticks`.
    pub fn build_thocks(&mut self, offset: f32, landings: impl Iterator<Item = f32>, total: usize) {
        let min_gap = 1.0 / 16.0;
        let mut last = f32::NEG_INFINITY;
        for (i, t) in landings.enumerate() {
            if t - last >= min_gap {
                self.thock(offset + t, i as f32 / total.max(1) as f32, 0.22, i as u32 * 2654435761);
                last = t;
            }
        }
    }

    /// Bubbly "bloop" (pitch up), for captions popping in.
    pub fn bloop(&mut self, t: f32, gain: f32) {
        let mut phase = 0.0f32;
        self.add(t, 0.14, |x| {
            let f = 320.0 + 900.0 * (x / 0.14);
            phase += f / RATE as f32;
            gain * (std::f32::consts::TAU * phase).sin() * (x / 0.14 * std::f32::consts::PI).sin()
        });
    }

    /// Short pitch-down pop (picking, plucking).
    pub fn pop(&mut self, t: f32, gain: f32) {
        let mut phase = 0.0f32;
        self.add(t, 0.07, |x| {
            let f = 950.0 - 6000.0 * x;
            phase += f.max(250.0) / RATE as f32;
            gain * (std::f32::consts::TAU * phase).sin() * (-x / 0.025).exp()
        });
    }

    /// Wooden knock (chopping board).
    pub fn knock(&mut self, t: f32, gain: f32, seed: u32) {
        let mut rng = Rng(seed | 1);
        let mut bp = Resonator::new(650.0, 0.96);
        let tau = std::f32::consts::TAU;
        self.add(t, 0.12, |x| {
            gain * (bp.run(rng.next() * 2.0 - 1.0) * 2.0 * (-x / 0.02).exp() + 0.6 * (tau * 110.0 * x).sin() * (-x / 0.04).exp())
        });
    }

    /// Soft dough thump.
    pub fn thump(&mut self, t: f32, gain: f32) {
        let tau = std::f32::consts::TAU;
        self.add(t, 0.12, |x| gain * (tau * (90.0 - 200.0 * x) * x).sin() * (-x / 0.04).exp());
    }

    /// Filtered-noise swish (stirring, waving).
    pub fn swish(&mut self, t: f32, len: f32, center: f32, gain: f32, seed: u32) {
        let mut rng = Rng(seed | 1);
        let mut bp = Resonator::new(center, 0.97);
        self.add(t, len, |x| gain * bp.run(rng.next() * 2.0 - 1.0) * (x / len * std::f32::consts::PI).sin().powi(2));
    }

    /// Pouring / watering: steady bright noise with a bubbly flutter.
    pub fn pour(&mut self, t: f32, len: f32, gain: f32, seed: u32) {
        let mut rng = Rng(seed | 1);
        let mut bp = Resonator::new(2600.0, 0.9);
        let mut bp2 = Resonator::new(900.0, 0.985);
        self.add(t, len, |x| {
            let fade = (x / 0.15).min(1.0) * ((len - x) / 0.2).clamp(0.0, 1.0);
            let flutter = 0.6 + 0.4 * (x * 37.0).sin() * (x * 11.0).sin();
            let n = rng.next() * 2.0 - 1.0;
            gain * fade * (bp.run(n) * 0.7 + bp2.run(n) * flutter * 1.5)
        });
    }

    /// Munch: three quick crunchy bursts.
    pub fn munch(&mut self, t: f32, gain: f32, seed: u32) {
        for k in 0..3 {
            let mut rng = Rng(seed.wrapping_add(k * 7919) | 1);
            let mut bp = Resonator::new(1800.0 - k as f32 * 300.0, 0.8);
            self.add(t + k as f32 * 0.09, 0.07, |x| gain * bp.run(rng.next() * 2.0 - 1.0) * (-x / 0.02).exp());
        }
    }

    /// "Ta-da!": a bright major arpeggio with a held top note.
    pub fn fanfare(&mut self, t: f32) {
        let tau = std::f32::consts::TAU;
        for (i, f) in [523.25f32, 659.25, 783.99, 1046.5].iter().enumerate() {
            let len = if i == 3 { 1.3 } else { 0.35 };
            let f = *f;
            self.add(t + i as f32 * 0.1, len, move |x| {
                let env = (x / 0.01).min(1.0) * (-x / (len * 0.45)).exp();
                let vib = 1.0 + 0.004 * (tau * 5.5 * x).sin() * (x / 0.3).min(1.0);
                let ph = tau * f * vib * x;
                // soft brassy tone: a few odd + even harmonics
                0.25 * env * (ph.sin() + 0.45 * (2.0 * ph).sin() + 0.3 * (3.0 * ph).sin() + 0.12 * (4.0 * ph).sin())
            });
        }
        // sparkle on top
        for k in 0..8 {
            let f = 2000.0 + 380.0 * k as f32;
            self.add(t + 0.35 + k as f32 * 0.05, 0.4, move |x| 0.06 * (tau * f * x).sin() * (-x / 0.12).exp());
        }
    }

    /// Curious two-note "hm?" for the opening question.
    pub fn question(&mut self, t: f32) {
        let tau = std::f32::consts::TAU;
        for (i, f) in [392.0f32, 587.33].iter().enumerate() {
            let f = *f;
            self.add(t + i as f32 * 0.16, 0.3, move |x| 0.24 * (tau * f * x).sin() * (x / 0.01).min(1.0) * (-x / 0.1).exp());
        }
    }

    /// Bird song: groups of quick upward chirps scattered over [from, to).
    pub fn birds(&mut self, from: f32, to: f32, seed: u32) {
        let mut rng = Rng(seed | 1);
        let mut t = from + rng.next() * 1.0;
        let tau = std::f32::consts::TAU;
        while t < to {
            let base = 2600.0 + rng.next() * 1600.0;
            let notes = 2 + (rng.next() * 4.0) as usize;
            let gain = 0.05 + rng.next() * 0.04;
            for n in 0..notes {
                let len = 0.05 + rng.next() * 0.06;
                let f0 = base * (1.0 + 0.08 * n as f32 * if rng.next() > 0.5 { 1.0 } else { -0.6 });
                let mut phase = 0.0f32;
                self.add(t + n as f32 * (len + 0.035), len, move |x| {
                    let k = x / len;
                    let f = f0 * (1.0 + 0.45 * k) * (1.0 + 0.03 * (tau * 60.0 * x).sin());
                    phase += f / RATE as f32;
                    gain * (tau * phase).sin() * (k * std::f32::consts::PI).sin()
                });
            }
            t += 1.0 + rng.next() * 2.4;
        }
    }

    /// Soft wind: slowly breathing low-passed noise.
    pub fn wind(&mut self, from: f32, to: f32, gain: f32, seed: u32) {
        let mut rng = Rng(seed | 1);
        let (mut lp1, mut lp2) = (0.0f32, 0.0f32);
        self.add(from, to - from, |x| {
            lp1 += (rng.next() * 2.0 - 1.0 - lp1) * 0.02;
            lp2 += (lp1 - lp2) * 0.05;
            let breath = 0.6 + 0.4 * (x * 0.7).sin() * (x * 0.23 + 1.0).sin();
            gain * lp2 * 8.0 * breath * (x / 0.5).min(1.0)
        });
    }

    /// Bees: two detuned buzzing voices, swelling in and out.
    pub fn bees(&mut self, from: f32, to: f32, gain: f32) {
        let tau = std::f32::consts::TAU;
        let (mut p1, mut p2) = (0.0f32, 0.0f32);
        let mut lp = 0.0f32;
        self.add(from, to - from, |x| {
            let f1 = 205.0 * (1.0 + 0.04 * (tau * 5.0 * x).sin());
            let f2 = 231.0 * (1.0 + 0.05 * (tau * 3.7 * x + 1.0).sin());
            p1 = (p1 + f1 / RATE as f32).fract();
            p2 = (p2 + f2 / RATE as f32).fract();
            let saw = (p1 * 2.0 - 1.0) + 0.8 * (p2 * 2.0 - 1.0);
            lp += (saw - lp) * 0.25;
            let swell = 0.5 + 0.5 * (x * 0.9).sin() * (x * 0.37 + 2.0).cos();
            gain * lp * swell * (x / 0.5).min(1.0)
        });
    }

    /// Soft footstep: a short low thump.
    pub fn footstep(&mut self, t: f32) {
        let tau = std::f32::consts::TAU;
        let mut seed: u32 = 0x2545_f491 ^ (t * 1000.0) as u32;
        self.add(t, 0.09, |x| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let env = (-x / 0.025).exp();
            0.12 * env * ((tau * 150.0 * x).sin() + 0.35 * noise)
        });
    }

    pub fn whoosh(&mut self, t: f32) {
        // Filtered noise with a rising-then-falling cutoff.
        let mut seed: u32 = 0x9e37_79b9 ^ (t * 1000.0) as u32;
        let mut lp = 0.0f32;
        let len = 0.5;
        self.add(t, len, |x| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let k = x / len;
            let env = (k * std::f32::consts::PI).sin().powi(2);
            let cutoff = 0.02 + 0.25 * env;
            lp += (noise - lp) * cutoff;
            // Soft: transitions should be felt, not startle (it's also the first sound of a battle).
            lp * env * 0.5
        });
    }

    /// Ticks for a list of landing times; rate-limited so thousands of blocks don't turn into noise.
    pub fn build_ticks(&mut self, offset: f32, landings: impl Iterator<Item = f32>, total: usize) {
        let min_gap = 1.0 / 14.0;
        let mut last = f32::NEG_INFINITY;
        for (i, t) in landings.enumerate() {
            if t - last >= min_gap {
                self.tick(offset + t, i as f32 / total.max(1) as f32, 0.22);
                last = t;
            }
        }
    }

    pub fn write_wav(&self, path: &std::path::Path) -> Result<()> {
        let peak = self.samples.iter().fold(0f32, |m, s| m.max(s.abs())).max(1e-6);
        let norm = if peak > 0.95 || self.boost { 0.95 / peak } else { 1.0 } * self.master;
        let data: Vec<u8> = self
            .samples
            .iter()
            .flat_map(|s| (((s * norm).clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes())
            .collect();
        let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
        let len = data.len() as u32;
        f.write_all(b"RIFF")?;
        f.write_all(&(36 + len).to_le_bytes())?;
        f.write_all(b"WAVEfmt ")?;
        f.write_all(&16u32.to_le_bytes())?;
        f.write_all(&1u16.to_le_bytes())?; // PCM
        f.write_all(&1u16.to_le_bytes())?; // mono
        f.write_all(&RATE.to_le_bytes())?;
        f.write_all(&(RATE * 2).to_le_bytes())?;
        f.write_all(&2u16.to_le_bytes())?;
        f.write_all(&16u16.to_le_bytes())?;
        f.write_all(b"data")?;
        f.write_all(&len.to_le_bytes())?;
        f.write_all(&data)?;
        Ok(())
    }
}

/// Narrator used by every video: ElevenLabs via OpenRouter's speech endpoint (same voice as the
/// "Let it Plow" Roblox short). Emotion tags like `[excited]` / `[shouts]` work with this model.
pub const TTS_MODEL: &str = "elevenlabs/eleven-v4";
pub const TTS_VOICE: &str = "will";

/// Text-to-speech through OpenRouter (`OPENROUTER_API_KEY`), decoded to mono f32 at RATE with
/// leading/trailing silence trimmed. Cached in the temp dir so re-renders don't pay again.
pub fn speech(model: &str, voice: &str, text: &str) -> Result<Vec<f32>> {
    use anyhow::{bail, Context};
    let mut h: u64 = 0xcbf29ce484222325;
    for b in model.bytes().chain([0]).chain(voice.bytes()).chain([0]).chain(text.bytes()) {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    let dir = std::env::temp_dir().join("voxreel-speech");
    std::fs::create_dir_all(&dir)?;
    let mp3 = dir.join(format!("{h:016x}.mp3"));
    if !mp3.exists() {
        let key = std::env::var("OPENROUTER_API_KEY").context("set OPENROUTER_API_KEY for the narrator")?;
        let resp = ureq::post("https://openrouter.ai/api/v1/audio/speech")
            .set("Authorization", &format!("Bearer {key}"))
            .timeout(std::time::Duration::from_secs(120))
            .send_json(serde_json::json!({ "model": model, "voice": voice, "input": text, "response_format": "mp3" }));
        let resp = match resp {
            Ok(r) => r,
            Err(ureq::Error::Status(code, r)) => bail!("TTS HTTP {code}: {}", r.into_string().unwrap_or_default()),
            Err(e) => return Err(e).context("TTS request"),
        };
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut resp.into_reader(), &mut bytes)?;
        std::fs::write(&mp3, bytes)?;
    }
    let trim = "silenceremove=start_periods=1:start_threshold=-45dB,areverse,silenceremove=start_periods=1:start_threshold=-45dB,areverse";
    let out = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&mp3)
        .args(["-af", trim, "-f", "f32le", "-ac", "1", "-ar", &RATE.to_string(), "-"])
        .output()
        .context("ffmpeg")?;
    if !out.status.success() {
        bail!("ffmpeg could not decode speech");
    }
    Ok(out.stdout.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

/// Tiny xorshift for reproducible noise.
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32
    }
}

/// Two-pole resonant band-pass (r close to 1 = narrow).
struct Resonator {
    a1: f32,
    a2: f32,
    gain: f32,
    y1: f32,
    y2: f32,
}
impl Resonator {
    fn new(freq: f32, r: f32) -> Self {
        let w = std::f32::consts::TAU * freq / RATE as f32;
        Self { a1: -2.0 * r * w.cos(), a2: r * r, gain: 1.0 - r, y1: 0.0, y2: 0.0 }
    }
    fn run(&mut self, x: f32) -> f32 {
        let y = self.gain * x - self.a1 * self.y1 - self.a2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Peak and RMS (dBFS) of each "how it's made" sound on its own, to balance the mix:
    /// `cargo test --release sfx_levels -- --nocapture`
    #[test]
    fn sfx_levels() {
        let db = |v: f32| 20.0 * v.max(1e-9).log10();
        let cases: Vec<(&str, f32, Box<dyn Fn(&mut Track)>)> = vec![
            ("thock", 0.12, Box::new(|t| t.thock(0.0, 0.5, 0.22, 7))),
            ("tick (battle)", 0.18, Box::new(|t| t.tick(0.0, 0.5, 0.22))),
            ("chime", 1.4, Box::new(|t| t.chime(0.0))),
            ("bloop", 0.14, Box::new(|t| t.bloop(0.0, 0.16))),
            ("pop", 0.07, Box::new(|t| t.pop(0.0, 0.22))),
            ("knock", 0.12, Box::new(|t| t.knock(0.0, 0.12, 3))),
            ("thump", 0.12, Box::new(|t| t.thump(0.0, 0.27))),
            ("swish", 0.4, Box::new(|t| t.swish(0.0, 0.4, 1100.0, 0.16, 5))),
            ("pour", 2.0, Box::new(|t| t.pour(0.0, 2.0, 0.09, 9))),
            ("munch", 0.3, Box::new(|t| t.munch(0.0, 0.18, 11))),
            ("fanfare", 1.6, Box::new(|t| t.fanfare(0.0))),
            ("question", 0.5, Box::new(|t| t.question(0.0))),
            ("footstep", 0.09, Box::new(|t| t.footstep(0.0))),
            ("whoosh", 0.5, Box::new(|t| t.whoosh(0.0))),
            ("birds", 6.0, Box::new(|t| t.birds(0.0, 6.0, 7))),
            ("wind", 6.0, Box::new(|t| t.wind(0.0, 6.0, 0.04, 11))),
            ("bees", 6.0, Box::new(|t| t.bees(0.0, 6.0, 0.04))),
        ];
        for (name, len, f) in cases {
            let mut t = Track::new(len + 0.1);
            f(&mut t);
            let n = (len * RATE as f32) as usize;
            let s = &t.samples[..n.min(t.samples.len())];
            let peak = s.iter().fold(0f32, |m, v| m.max(v.abs()));
            let rms = (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt();
            println!("{name:>14}: peak {:6.1} dB  rms {:6.1} dB", db(peak), db(rms));
        }
    }
}
