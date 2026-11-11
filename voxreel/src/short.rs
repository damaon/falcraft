//! Turns builds into a vertical short: shot list, camera moves, captions and the matching
//! audio timeline. The same shot list drives video frames and sound so they stay in sync.
//!
//! Layout (1080x1920 reference, scaled to the output height):
//! - captions live in the top ~25% and around y=1350..1520,
//! - the bottom ~20% is left clear because the YouTube Shorts UI covers it.

use crate::anim::BuildAnim;
use crate::audio::Track;
use crate::render::{Instance, Renderer, Theme, View};
use crate::text::{Style, TextCache};
use crate::video::{self, Encoder};
use anyhow::Result;
use glam::{Mat4, Vec3};
use std::path::Path;

pub struct Entry {
    /// What viewers see while the build happens ("#1"), so they can guess before the reveal.
    pub alias: String,
    /// Revealed in the final lineup ("CLAUDE OPUS 5.5").
    pub name: String,
    pub anim: BuildAnim,
}

pub struct Config {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub build_secs: f32,
    pub hold_secs: f32,
    pub lineup_secs: f32,
    /// Small line above the title ("BUILD BATTLE").
    pub kicker: String,
    /// Big title ("A DRAGON").
    pub title: String,
    /// Ask for comments in the lineup ("WHICH ONE WON?").
    pub lineup_question: String,
    pub theme: Theme,
    /// Length of the opening hook (0 disables it). Only used with more than one entry.
    pub teaser_secs: f32,
    /// Closing card with the logo and where to get the builds (0 disables it).
    pub outro_secs: f32,
    /// Shown and spoken in the outro, e.g. "bananini.games".
    pub site: String,
    /// Final mix level (1.0 = normalized).
    pub volume: f32,
}

/// The Chimpanzini Bananini logo shared with the website.
const LOGO_SVG: &[u8] = include_bytes!("../assets/logo.svg");


#[derive(Clone, Copy)]
enum Shot {
    /// Opening hook: the finished builds, anonymous, before we show how they were made.
    Teaser { start: f32 },
    Build { entry: usize, start: f32 },
    Lineup { start: f32 },
    /// Closing card: logo, "visit <site>", spoken brand.
    Outro { start: f32 },
}

pub struct Short {
    pub cfg: Config,
    pub entries: Vec<Entry>,
    shots: Vec<Shot>,
    pub duration: f32,
    /// Camera distance per entry for the full-frame build shot, fitted once so the orbit
    /// doesn't "breathe".
    build_dist: Vec<f32>,
    panel_dist: Vec<f32>,
}

const YELLOW: [u8; 3] = [255, 214, 10];
const WHITE: [u8; 3] = [255, 255, 255];

/// Camera style for a shot: how high it looks down from and where the subject sits in frame.
#[derive(Clone, Copy)]
struct Cam {
    elevation: f32,
    shift: [f32; 2],
}
const BUILD_CAM: Cam = Cam { elevation: 0.26, shift: [0.0, 0.0] };
/// Lineup panels are very wide and short: look more from the side and push the build left so
/// the model name fits on the right.
const PANEL_CAM_LABELLED: Cam = Cam { elevation: 0.22, shift: [-0.22, 0.0] };
/// Without names there is nothing on the right, so the build sits centred.
const PANEL_CAM: Cam = Cam { elevation: 0.22, shift: [0.06, 0.0] };

fn panel_cam(entries: &[Entry]) -> (Cam, [f32; 4]) {
    if entries.iter().any(|e| !e.name.is_empty()) {
        (PANEL_CAM_LABELLED, [-0.62, 0.22, -0.92, 0.90])
    } else {
        (PANEL_CAM, [-0.62, 0.74, -0.92, 0.90])
    }
}
const FOV_Y: f32 = 34.0;
const PLATE_MARGIN: f32 = 1.5;

/// Lineup panels: vertical strip between the question caption and the CTA.
const PANELS_TOP: f32 = 360.0;
const PANELS_BOTTOM: f32 = 1560.0;

impl Short {
    pub fn new(cfg: Config, entries: Vec<Entry>) -> Self {
        let mut shots = Vec::new();
        let mut t = 0.0;
        if entries.len() > 1 && cfg.teaser_secs > 0.0 {
            shots.push(Shot::Teaser { start: 0.0 });
            t += cfg.teaser_secs;
        }
        let per = cfg.build_secs + cfg.hold_secs;
        for i in 0..entries.len() {
            shots.push(Shot::Build { entry: i, start: t });
            t += per;
        }
        if entries.len() > 1 {
            shots.push(Shot::Lineup { start: t });
            t += cfg.lineup_secs;
        }
        if cfg.outro_secs > 0.0 {
            shots.push(Shot::Outro { start: t });
            t += cfg.outro_secs;
        }
        let aspect = cfg.width as f32 / cfg.height as f32;
        // Safe area in NDC for the build: below the title block, above the bottom captions.
        let build_dist = entries.iter().map(|e| fit_distance(&e.anim, aspect, [-0.86, 0.86, -0.40, 0.42], BUILD_CAM)).collect();
        let panel_h = panel_height(&cfg, entries.len());
        let panel_aspect = cfg.width as f32 / panel_h as f32;
        let (cam, safe) = panel_cam(&entries);
        let panel_dist = entries.iter().map(|e| fit_distance(&e.anim, panel_aspect, safe, cam)).collect();
        Self { cfg, entries, shots, duration: t, build_dist, panel_dist }
    }

    fn shot_at(&self, t: f32) -> Shot {
        *self
            .shots
            .iter()
            .rev()
            .find(|s| match s {
                Shot::Build { start, .. } | Shot::Lineup { start } | Shot::Teaser { start } | Shot::Outro { start } => *start <= t,
            })
            .unwrap_or(&self.shots[0])
    }

    pub fn soundtrack(&self) -> Result<Track> {
        let mut tr = Track::new(self.duration);
        tr.master = self.cfg.volume;
        for shot in &self.shots {
            match *shot {
                Shot::Build { entry, start } => {
                    let a = &self.entries[entry].anim;
                    if start > 0.0 {
                        tr.whoosh(start - 0.25);
                    }
                    tr.build_ticks(start, a.landing_times(), a.voxels.len());
                    tr.chime(start + a.duration);
                }
                // No sound on the opening hook: starting a video with a loud ping is jarring.
                Shot::Teaser { .. } => {}
                Shot::Outro { start } => {
                    tr.whoosh(start - 0.25);
                    // The logo popping in is the brand moment; the narrator carries the call to action.
                    let site_spoken = self.cfg.site.replace('.', " dot ");
                    let lines = [(format!("[excited] Get these builds at {site_spoken}! [shouts] Bananini Games!"), 0.6, 1.0)];
                    for (text, at, gain) in lines {
                        match crate::audio::speech(crate::audio::TTS_MODEL, crate::audio::TTS_VOICE, &text) {
                            Ok(samples) => tr.clip(start + at, &samples, gain),
                            Err(e) => eprintln!("  warning: no speech ({e:#}), outro stays silent"),
                        }
                    }
                }
                Shot::Lineup { start } => {
                    tr.whoosh(start - 0.25);
                    for i in (0..self.entries.len()).filter(|i| !self.entries[*i].name.is_empty()) {
                        tr.tick(start + reveal_time(i), i as f32 / self.entries.len() as f32 * 0.6 + 0.3, 0.5);
                    }
                }
            }
        }
        Ok(tr)
    }

    fn scene(&self, a: &BuildAnim, t: f32, inst: &mut Vec<Instance>) {
        inst.clear();
        let (w, d) = (a.size.x as f32, a.size.z as f32);
        inst.push(Instance::slab(
            Vec3::new(-w * 0.5 - PLATE_MARGIN, -0.4, -d * 0.5 - PLATE_MARGIN),
            Vec3::new(w * 0.5 + PLATE_MARGIN, 0.0, d * 0.5 + PLATE_MARGIN),
            self.cfg.theme.plate,
        ));
        a.instances(t, Vec3::ZERO, inst);
    }

    fn build_view(&self, entry: usize, lt: f32) -> View {
        let a = &self.entries[entry].anim;
        let per = self.cfg.build_secs + self.cfg.hold_secs;
        let full_h = a.size.y as f32;
        // Follow the build front: start close and low, pull back as it grows, then a slow
        // dolly-in during the hold so the shot never sits still.
        let grown = (a.height_at(lt) / full_h).clamp(0.0, 1.0);
        let follow = 0.75 + 0.25 * grown.sqrt();
        let hold = ((lt - a.duration) / (per - a.duration).max(0.1)).clamp(0.0, 1.0);
        let zoom = follow * (1.0 - 0.08 * hold * (2.0 - hold));
        let angle = 0.6 + entry as f32 * 1.3 + lt * 0.42;
        let mut v = orbit_view(a, angle, self.build_dist[entry] * zoom, BUILD_CAM);
        let target_y = (a.height_at(lt).max(3.0) * 0.5).min(full_h * 0.5);
        let delta = Vec3::new(0.0, target_y - v.target.y, 0.0);
        v.target += delta;
        v.eye += delta;
        v
    }

    /// Header lines above the title: the site ("BANANINI.GAMES") and the kicker ("BUILD BATTLE").
    /// Returns the y where the title goes.
    fn header(&self, mut top: f32, frame: &mut [u8], text: &mut TextCache) -> f32 {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let s = h as f32 / 1920.0;
        let cx = w as f32 * 0.5;
        if !self.cfg.site.is_empty() {
            let site = self.cfg.site.to_uppercase();
            let l = text.get(&site, Style::new(50.0 * s, WHITE));
            l.blit(frame, w, h, cx, top, 1.0);
            top += l.h as f32 - 10.0 * s;
        }
        if !self.cfg.kicker.is_empty() {
            let k = text.get(&self.cfg.kicker, Style::new(60.0 * s, YELLOW));
            k.blit(frame, w, h, cx, top, 1.0);
            top += k.h as f32 - 8.0 * s;
        }
        top
    }

    fn overlay_build(&self, entry: usize, lt: f32, frame: &mut [u8], text: &mut TextCache) {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let s = h as f32 / 1920.0;
        let cx = w as f32 * 0.5;
        let e = &self.entries[entry];
        let top = self.header(130.0 * s, frame, text);
        let mut big = Style::new(112.0 * s, WHITE);
        big.max_width = 980.0 * s;
        let big = text.fit(&self.cfg.title, big, 2, 64.0 * s);
        text.get(&self.cfg.title, big).blit(frame, w, h, cx, top, 1.0);

        let fade = (lt / 0.25).clamp(0.0, 1.0);
        if !e.alias.is_empty() {
            let label = text.get(&e.alias, Style::new(104.0 * s, YELLOW));
            label.blit(frame, w, h, cx, 1340.0 * s + (1.0 - fade) * 40.0 * s, fade);
        }
        let placed = e.anim.landing_times().filter(|l| *l <= lt).count();
        let done = placed == e.anim.voxels.len();
        let counter = format!("{placed} BLOCKS");
        let col = if done { YELLOW } else { WHITE };
        text.get(&counter, Style::new(60.0 * s, col)).blit(frame, w, h, cx, 1470.0 * s, 1.0);
    }

    /// Lineup: one horizontal panel per build, stacked vertically, each orbiting in sync.
    fn render_lineup(&self, lt: f32, teaser: bool, panel: &mut Renderer, frame: &mut [u8], text: &mut TextCache, inst: &mut Vec<Instance>, buf: &mut Vec<u8>) -> Result<()> {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let s = h as f32 / 1920.0;
        let n = self.entries.len();
        let ph = panel.height as usize;
        let top = (PANELS_TOP * s) as usize;
        let gap = ((PANELS_BOTTOM - PANELS_TOP) * s) as usize / n - ph;
        let fade = |x: f32| (x / 0.25).clamp(0.0, 1.0);
        for (i, e) in self.entries.iter().enumerate() {
            self.scene(&e.anim, 1e6, inst);
            let angle = 0.9 + lt * 0.35;
            let mut view = orbit_view(&e.anim, angle, self.panel_dist[i], panel_cam(&self.entries).0);
            view.time = lt;
            panel.render(inst, &view, buf)?;
            let y0 = top + i * (ph + gap);
            for row in 0..ph {
                let dst = ((y0 + row) * w) * 4;
                frame[dst..dst + w * 4].copy_from_slice(&buf[row * w * 4..(row + 1) * w * 4]);
            }
            // Number on the left, name revealed after a beat, staggered so each lands on its own.
            let num = text.get(&format!("{}", i + 1), Style::new(150.0 * s, YELLOW));
            let num_h = num.h as f32;
            num.blit(frame, w, h, 120.0 * s, y0 as f32 + ph as f32 * 0.5 - num_h * 0.5, 1.0);
            let reveal = lt - reveal_time(i);
            if e.name.is_empty() {
                // Nothing to reveal: anonymous builders.
            } else if teaser {
                let q = text.get("???", Style::new(80.0 * s, WHITE));
                let qh = q.h as f32;
                q.blit(frame, w, h, w as f32 * 0.83, y0 as f32 + ph as f32 * 0.5 - qh * 0.5, 1.0);
            } else if reveal > 0.0 {
                let mut st = Style::new(64.0 * s, WHITE);
                st.max_width = 300.0 * s;
                let name = text.get(&e.name, st);
                let y = y0 as f32 + ph as f32 * 0.5 - name.h as f32 * 0.5 + (1.0 - fade(reveal)) * 20.0 * s;
                name.blit(frame, w, h, w as f32 * 0.83, y, fade(reveal));
            }
        }
        if teaser {
            let top = self.header(70.0 * s, frame, text);
            let mut big = Style::new(96.0 * s, WHITE);
            big.max_width = 1000.0 * s;
            let big = text.fit(&self.cfg.title, big, 1, 40.0 * s);
            text.get(&self.cfg.title, big).blit(frame, w, h, w as f32 * 0.5, top, 1.0);
            let sub = "WHO BUILT IT BEST?".to_string();
            text.get(&sub, Style::new(70.0 * s, YELLOW)).blit(frame, w, h, w as f32 * 0.5, PANELS_BOTTOM * s + 10.0 * s, 1.0);
        } else {
            let mut qs = Style::new(110.0 * s, WHITE);
            qs.max_width = 1000.0 * s;
            let qs = text.fit(&self.cfg.lineup_question, qs, 1, 60.0 * s);
            let q = text.get(&self.cfg.lineup_question, qs);
            q.blit(frame, w, h, w as f32 * 0.5, 200.0 * s, 1.0);
            let nums: Vec<String> = (1..=n).map(|i| i.to_string()).collect();
            let cta = format!("{} OR {}?", nums[..n - 1].join(", "), nums[n - 1]);
            text.get(&cta, Style::new(70.0 * s, YELLOW)).blit(frame, w, h, w as f32 * 0.5, PANELS_BOTTOM * s + 10.0 * s, 1.0);
        }
        Ok(())
    }

    fn overlay_outro(&self, lt: f32, frame: &mut [u8], text: &mut TextCache) -> Result<()> {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let s = h as f32 / 1920.0;
        let cx = w as f32 * 0.5;
        let fade = |x: f32| (x / 0.3).clamp(0.0, 1.0);
        // Logo pops in with a little overshoot, then bobs gently.
        let k = (lt / 0.55).clamp(0.0, 1.0);
        let back = 1.0 + 2.2 * (k - 1.0).powi(3) + 1.2 * (k - 1.0).powi(2);
        let size = (640.0 * s * back.max(0.05)) as u32;
        let bob = (lt * 2.6).sin() * 12.0 * s * k;
        let logo = text.svg("logo", LOGO_SVG, size.max(8))?;
        let lh = logo.h as f32;
        logo.blit(frame, w, h, cx, 560.0 * s - lh * 0.5 + bob, 1.0);

        let line1 = text.get("GET THESE BUILDS", Style::new(96.0 * s, WHITE));
        line1.blit(frame, w, h, cx, 930.0 * s, fade(lt - 0.5));
        let line2 = text.get("FOR MINECRAFT!", Style::new(96.0 * s, WHITE));
        line2.blit(frame, w, h, cx, 1035.0 * s, fade(lt - 0.7));
        let visit = text.get("VISIT", Style::new(70.0 * s, WHITE));
        visit.blit(frame, w, h, cx, 1200.0 * s, fade(lt - 1.0));
        let mut big = Style::new(132.0 * s, YELLOW);
        big.max_width = 940.0 * s;
        let site = self.cfg.site.to_uppercase();
        let big = text.fit(&site, big, 1, 70.0 * s);
        text.get(&site, big).blit(frame, w, h, cx, 1280.0 * s, fade(lt - 1.1));
        Ok(())
    }

    /// Empty scene (sky + ground) behind the panels so the gaps match the rest of the video.
    fn lineup_background(&self, main: &mut Renderer, inst: &mut Vec<Instance>, buf: &mut Vec<u8>) -> Result<()> {
        inst.clear();
        let view = View {
            eye: Vec3::new(0.0, 30.0, 60.0),
            target: Vec3::ZERO,
            fov_y_deg: FOV_Y,
            scene_center: Vec3::ZERO,
            scene_radius: 10.0,
            ground_y: -0.4,
            shift: [0.0, 0.0],
            time: 0.0,
        };
        main.render(inst, &view, buf)
    }

    fn frame(&self, t: f32, main: &mut Renderer, panel: &mut Option<Renderer>, text: &mut TextCache, inst: &mut Vec<Instance>, buf: &mut Vec<u8>) -> Result<()> {
        match self.shot_at(t) {
            Shot::Build { entry, start } => {
                let lt = t - start;
                self.scene(&self.entries[entry].anim, lt, inst);
                let mut view = self.build_view(entry, lt);
                view.time = t;
                main.render(inst, &view, buf)?;
                self.overlay_build(entry, lt, buf, text);
            }
            Shot::Outro { start } => {
                // Look up at the sky so the logo sits on clouds, with a slow drift.
                inst.clear();
                let lt = t - start;
                let view = View {
                    eye: Vec3::new(lt * 1.5, 3.0, 80.0),
                    target: Vec3::new(lt * 1.5, 16.0, 0.0),
                    fov_y_deg: FOV_Y * 1.6,
                    scene_center: Vec3::ZERO,
                    scene_radius: 10.0,
                    ground_y: -0.4,
                    shift: [0.0, 0.0],
                    time: t,
                };
                main.render(inst, &view, buf)?;
                self.overlay_outro(t - start, buf, text)?;
            }
            Shot::Lineup { start } | Shot::Teaser { start } => {
                let teaser = matches!(self.shot_at(t), Shot::Teaser { .. });
                let panel = match panel {
                    Some(p) => p,
                    None => panel.insert(Renderer::new(self.cfg.width, panel_height(&self.cfg, self.entries.len()), self.cfg.theme)?),
                };
                self.lineup_background(main, inst, buf)?;
                let mut tmp = Vec::new();
                // The teaser keeps spinning from where the lineup will later continue.
                let lt = if teaser { t - start + 20.0 } else { t - start };
                self.render_lineup(lt, teaser, panel, buf, text, inst, &mut tmp)?;
            }
        }
        Ok(())
    }

    pub fn render_video(&self, renderer: &mut Renderer, text: &mut TextCache, out: &Path) -> Result<()> {
        let wav = out.with_extension("wav");
        self.soundtrack()?.write_wav(&wav)?;
        let mut enc = Encoder::start(out, self.cfg.width, self.cfg.height, self.cfg.fps, Some(&wav))?;
        let frames = (self.duration * self.cfg.fps as f32).ceil() as u32;
        let (mut inst, mut buf, mut panel) = (Vec::new(), Vec::new(), None);
        let started = std::time::Instant::now();
        for f in 0..frames {
            let t = f as f32 / self.cfg.fps as f32;
            self.frame(t, renderer, &mut panel, text, &mut inst, &mut buf)?;
            enc.frame(&buf)?;
            if f % self.cfg.fps == 0 {
                eprint!("\r  frame {f}/{frames} ({:.0} fps)  ", f as f32 / started.elapsed().as_secs_f32().max(1e-3));
            }
        }
        enc.finish()?;
        let _ = std::fs::remove_file(&wav);
        eprintln!("\r  {frames} frames in {:.1}s -> {}", started.elapsed().as_secs_f32(), out.display());
        Ok(())
    }

    pub fn render_still(&self, renderer: &mut Renderer, text: &mut TextCache, t: f32, out: &Path) -> Result<()> {
        let (mut inst, mut buf, mut panel) = (Vec::new(), Vec::new(), None);
        self.frame(t, renderer, &mut panel, text, &mut inst, &mut buf)?;
        video::write_png(out, &buf, self.cfg.width, self.cfg.height)
    }
}

fn reveal_time(i: usize) -> f32 {
    1.3 + i as f32 * 0.45
}

fn panel_height(cfg: &Config, n: usize) -> u32 {
    let s = cfg.height as f32 / 1920.0;
    let slot = (PANELS_BOTTOM - PANELS_TOP) * s / n.max(1) as f32;
    ((slot - 16.0 * s) as u32) & !1
}

fn target_of(a: &BuildAnim) -> Vec3 {
    Vec3::new(0.0, a.size.y as f32 * 0.5, 0.0)
}

fn orbit_view(a: &BuildAnim, angle: f32, dist: f32, cam: Cam) -> View {
    let target = target_of(a);
    let e = cam.elevation;
    let dir = Vec3::new(angle.sin() * e.cos(), e.sin(), angle.cos() * e.cos());
    let (w, h, d) = (a.size.x as f32, a.size.y as f32, a.size.z as f32);
    View {
        eye: target + dir * dist,
        target,
        fov_y_deg: FOV_Y,
        scene_center: target,
        scene_radius: 0.5 * (w * w + h * h + d * d).sqrt() + PLATE_MARGIN * 2.0,
        ground_y: -0.4,
        shift: cam.shift,
        time: 0.0,
    }
}

/// Smallest camera distance at which the build (its voxels plus the plate corners) stays inside
/// the NDC rectangle `[x_min, x_max, y_min, y_max]` from every orbit angle. Fitting to voxels
/// rather than the bounding box matters for spiky shapes (wings, tails) that leave the box mostly empty.
fn fit_distance(a: &BuildAnim, aspect: f32, safe: [f32; 4], cam: Cam) -> f32 {
    let (w, d) = (a.size.x as f32, a.size.z as f32);
    let center = Vec3::new(w * 0.5, 0.0, d * 0.5);
    let stride = (a.voxels.len() / 6000).max(1);
    let mut corners: Vec<Vec3> = a.voxels.iter().step_by(stride).map(|(p, _, _)| p.as_vec3() + 0.5 - center).collect();
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        corners.push(Vec3::new(sx * (w * 0.5 + PLATE_MARGIN), -0.4, sz * (d * 0.5 + PLATE_MARGIN)));
    }
    let fits = |dist: f32| {
        (0..24).all(|k| {
            let v = orbit_view(a, k as f32 / 24.0 * std::f32::consts::TAU, dist, cam);
            let vp = v.projection(aspect) * Mat4::look_at_rh(v.eye, v.target, Vec3::Y);
            corners.iter().all(|c| {
                let p = vp.project_point3(*c);
                p.x >= safe[0] && p.x <= safe[1] && p.y >= safe[2] && p.y <= safe[3] && p.z < 1.0
            })
        })
    };
    let (mut near, mut far) = (1.0f32, 3000.0f32);
    for _ in 0..30 {
        let mid = 0.5 * (near + far);
        if fits(mid) {
            far = mid;
        } else {
            near = mid;
        }
    }
    far
}
