//! Shorts-style captions: bold fill with a thick dark outline, pre-rasterized into sprites
//! and alpha-composited onto the CPU frame.

use anyhow::{Context, Result};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;

/// Lilita One (SIL OFL, see assets/fonts/OFL.txt): chunky and rounded, reads well for kids
/// and at phone size. Embedded so renders look the same on every machine.
const DEFAULT_FONT: &[u8] = include_bytes!("../assets/fonts/LilitaOne-Regular.ttf");

pub fn load_font(path: Option<&str>) -> Result<Font> {
    let Some(path) = path else {
        return Font::from_bytes(DEFAULT_FONT, FontSettings::default()).map_err(|e| anyhow::anyhow!("embedded font: {e}"));
    };
    let bytes = std::fs::read(&path).with_context(|| format!("read font {path}"))?;
    Font::from_bytes(bytes, FontSettings::default()).map_err(|e| anyhow::anyhow!("font {path}: {e}"))
}

pub struct Sprite {
    pub w: usize,
    pub h: usize,
    /// Premultiplied RGBA, f32 0..1.
    px: Vec<[f32; 4]>,
}

#[derive(Clone, Copy)]
pub struct Style {
    pub size: f32,
    pub fill: [u8; 3],
    pub outline: [u8; 3],
    pub outline_px: f32,
    pub max_width: f32,
}

impl Style {
    pub fn new(size: f32, fill: [u8; 3]) -> Self {
        Self { size, fill, outline: [34, 24, 82], outline_px: (size * 0.10).max(3.0), max_width: 980.0 }
    }
}

fn wrap(font: &Font, text: &str, size: f32, max_width: f32) -> Vec<String> {
    let width = |s: &str| -> f32 { s.chars().map(|c| font.metrics(c, size).advance_width).sum() };
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if width(&candidate) > max_width && !line.is_empty() {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

impl Sprite {
    /// Rasterizes an SVG so its larger side is `size` pixels.
    pub fn svg(data: &[u8], size: u32) -> Result<Self> {
        let tree = resvg::usvg::Tree::from_data(data, &resvg::usvg::Options::default())?;
        let ts = tree.size();
        let scale = size as f32 / ts.width().max(ts.height());
        let (w, h) = ((ts.width() * scale).ceil() as u32, (ts.height() * scale).ceil() as u32);
        let mut pm = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1)).context("pixmap")?;
        resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
        // tiny-skia pixels are premultiplied RGBA, which is what blit expects.
        let px = pm.data().chunks_exact(4).map(|c| [0, 1, 2, 3].map(|k| c[k] as f32 / 255.0)).collect();
        Ok(Self { w: w as usize, h: h as usize, px })
    }

    pub fn text(font: &Font, text: &str, st: Style) -> Self {
        let lines = wrap(font, text, st.size, st.max_width);
        let pad = st.outline_px.ceil() as usize + 2;
        // Soft drop shadow below the outline gives the "sticker" look kids' channels use.
        let shadow_dy = (st.size * 0.07).round().max(2.0) as usize;
        let line_h = (st.size * 1.08).ceil() as usize;
        let lm = font.horizontal_line_metrics(st.size);
        let ascent = lm.map(|m| m.ascent).unwrap_or(st.size * 0.8);
        let widths: Vec<f32> = lines
            .iter()
            .map(|l| l.chars().map(|c| font.metrics(c, st.size).advance_width).sum())
            .collect();
        let w = widths.iter().cloned().fold(1.0, f32::max).ceil() as usize + pad * 2;
        let h = line_h * lines.len().max(1) + pad * 2 + shadow_dy;
        let mut cov = vec![0f32; w * h];
        for (li, line) in lines.iter().enumerate() {
            let mut x = pad as f32 + (w as f32 - 2.0 * pad as f32 - widths[li]) * 0.5;
            let baseline = pad as f32 + li as f32 * line_h as f32 + ascent;
            for c in line.chars() {
                let (m, bmp) = font.rasterize(c, st.size);
                let gx = (x + m.xmin as f32).round() as i64;
                let gy = (baseline - m.height as f32 - m.ymin as f32).round() as i64;
                for by in 0..m.height {
                    for bx in 0..m.width {
                        let (px, py) = (gx + bx as i64, gy + by as i64);
                        if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                            let i = py as usize * w + px as usize;
                            cov[i] = cov[i].max(bmp[by * m.width + bx] as f32 / 255.0);
                        }
                    }
                }
                x += m.advance_width;
            }
        }
        // Outline = max of coverage sampled on rings around each pixel (round, cheap enough
        // because sprites are cached and only rebuilt when their text changes).
        let r = st.outline_px;
        let mut offsets = Vec::new();
        let ri = r.ceil() as i64;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                if d <= r {
                    offsets.push((dx, dy, (r - d + 0.5).clamp(0.0, 1.0)));
                }
            }
        }
        let mut outline = vec![0f32; w * h];
        for y in 0..h as i64 {
            for x in 0..w as i64 {
                let mut m = 0f32;
                for &(dx, dy, wgt) in &offsets {
                    let (sx, sy) = (x + dx, y + dy);
                    if sx >= 0 && sy >= 0 && (sx as usize) < w && (sy as usize) < h {
                        m = m.max(cov[sy as usize * w + sx as usize] * wgt);
                        if m >= 1.0 {
                            break;
                        }
                    }
                }
                outline[y as usize * w + x as usize] = m;
            }
        }
        let f = st.fill.map(|c| c as f32 / 255.0);
        let o = st.outline.map(|c| c as f32 / 255.0);
        let px = (0..w * h)
            .map(|i| {
                let (c, ol) = (cov[i], outline[i]);
                let sh = if i >= shadow_dy * w { outline[i - shadow_dy * w] * 0.45 } else { 0.0 };
                let a = ol.max(c);
                let rgb = [0, 1, 2].map(|k| f[k] * c + o[k] * (a - c));
                // Shadow (black, premultiplied) only where the glyph+outline doesn't cover.
                let a_total = a + sh * (1.0 - a);
                [rgb[0], rgb[1], rgb[2], a_total]
            })
            .collect();
        Self { w, h, px }
    }

    /// Composites centred horizontally on `cx` with the top edge at `top`.
    pub fn blit(&self, frame: &mut [u8], fw: usize, fh: usize, cx: f32, top: f32, alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        let x0 = (cx - self.w as f32 * 0.5).round() as i64;
        let y0 = top.round() as i64;
        for y in 0..self.h {
            let fy = y0 + y as i64;
            if fy < 0 || fy >= fh as i64 {
                continue;
            }
            for x in 0..self.w {
                let fx = x0 + x as i64;
                if fx < 0 || fx >= fw as i64 {
                    continue;
                }
                let s = self.px[y * self.w + x];
                let a = s[3] * alpha;
                if a <= 0.0 {
                    continue;
                }
                let i = (fy as usize * fw + fx as usize) * 4;
                for k in 0..3 {
                    let d = frame[i + k] as f32 / 255.0;
                    frame[i + k] = ((s[k] * alpha + d * (1.0 - a)) * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

/// Caches sprites by (text, style) so per-frame captions cost only a blit.
pub struct TextCache {
    font: Font,
    sprites: HashMap<String, Sprite>,
}

impl TextCache {
    pub fn new(font: Font) -> Self {
        Self { font, sprites: HashMap::new() }
    }

    /// Shrinks the style until the text wraps to at most `max_lines`, so long prompts stay
    /// big without eating the whole frame.
    pub fn fit(&self, text: &str, mut st: Style, max_lines: usize, min_size: f32) -> Style {
        let too_big = |st: &Style| {
            let lines = wrap(&self.font, text, st.size, st.max_width);
            // A single long word can't wrap, so also check the widest line.
            let widest = lines.iter().map(|l| l.chars().map(|c| self.font.metrics(c, st.size).advance_width).sum::<f32>()).fold(0.0, f32::max);
            lines.len() > max_lines || widest > st.max_width
        };
        while st.size > min_size && too_big(&st) {
            st = Style { size: st.size * 0.92, outline_px: st.outline_px * 0.92, ..st };
        }
        st
    }

    /// SVG sprite cached per size (sizes are bucketed so animations don't rasterize every frame).
    pub fn svg(&mut self, key: &str, data: &[u8], size: u32) -> Result<&Sprite> {
        let size = (size / 4).max(1) * 4;
        let k = format!("svg:{key}:{size}");
        if !self.sprites.contains_key(&k) {
            self.sprites.insert(k.clone(), Sprite::svg(data, size)?);
        }
        Ok(&self.sprites[&k])
    }

    pub fn get(&mut self, text: &str, st: Style) -> &Sprite {
        let key = format!("{text}\u{0}{}|{:?}|{:?}|{}|{}", st.size, st.fill, st.outline, st.outline_px, st.max_width);
        if self.sprites.len() > 512 {
            self.sprites.clear();
        }
        let font = &self.font;
        self.sprites.entry(key).or_insert_with(|| Sprite::text(font, text, st))
    }
}
