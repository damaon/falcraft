//! Procedural soundtrack: marimba-ish ticks on block landings (rising pentatonic as the build
//! grows, which is the "satisfying" part), a whoosh on cuts and a chime when a build completes.

use anyhow::Result;
use std::io::Write;

pub const RATE: u32 = 48_000;

pub struct Track {
    samples: Vec<f32>,
    /// Final level relative to a normalized mix (0.8 = a bit quieter than full scale).
    pub master: f32,
}

/// Major pentatonic ladder in semitones from the root, over three octaves.
const PENTA: [i32; 5] = [0, 2, 4, 7, 9];

impl Track {
    pub fn new(seconds: f32) -> Self {
        Self { samples: vec![0.0; (seconds * RATE as f32) as usize + RATE as usize], master: 1.0 }
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
        let norm = if peak > 0.95 { 0.95 / peak } else { 1.0 } * self.master;
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
