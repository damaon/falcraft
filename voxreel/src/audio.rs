//! Procedural soundtrack: marimba-ish ticks on block landings (rising pentatonic as the build
//! grows, which is the "satisfying" part), a whoosh on cuts and a chime when a build completes.

use anyhow::Result;
use std::io::Write;

pub const RATE: u32 = 48_000;

pub struct Track {
    samples: Vec<f32>,
}

/// Major pentatonic ladder in semitones from the root, over three octaves.
const PENTA: [i32; 5] = [0, 2, 4, 7, 9];

impl Track {
    pub fn new(seconds: f32) -> Self {
        Self { samples: vec![0.0; (seconds * RATE as f32) as usize + RATE as usize] }
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
            lp * env * 0.9
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
        let norm = if peak > 0.95 { 0.95 / peak } else { 1.0 };
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
