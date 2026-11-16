//! "Story" shorts: one world at character scale, characters following a script (walk, carry,
//! work), props that move between places and hands, a camera operator and step captions.
//! The script (`script.json`) is written by `tools/story.py`; this module only plays it back.
//!
//! Coordinates are the world build's own voxel coordinates; y = 0 is the grass (the renderer's
//! ground plane), the world's bottom voxel layer sits just below it.

use crate::actor::{self, Look, Pose};
use crate::anim::BuildAnim;
use crate::audio::Track;
use crate::render::{Instance, Renderer, Theme, View};
use crate::text::{Style, TextCache};
use crate::video::{self, Encoder};
use anyhow::{bail, Context, Result};
use glam::{Quat, Vec3};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct Script {
    pub title: String,
    pub hook: String,
    #[serde(default = "d_kicker")]
    pub kicker: String,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub ambience: Vec<String>,
    pub duration: f32,
    /// The hook question is shown until this time...
    #[serde(default)]
    pub hook_until: f32,
    /// ...over the scene as it is at this story time (the finished product), so the short opens
    /// on the payoff and its end loops back into the start.
    #[serde(default)]
    pub hook_scene: f32,
    pub world: PathBuf,
    /// Size of one world block in voxels (world.json is in blocks; also the ground checker).
    #[serde(default = "d_block")]
    pub block: f32,
    #[serde(default = "d_fov")]
    pub fov: f32,
    pub actors: Vec<ActorScript>,
    #[serde(default)]
    pub props: Vec<PropScript>,
    /// [t, eye x, y, z, target x, y, z, cut]: linear between keys; `cut` = jump to this key.
    pub camera: Vec<[f32; 8]>,
    /// [t0, t1, label, text]
    #[serde(default)]
    pub captions: Vec<(f32, f32, String, String)>,
    /// [t, kind]: fanfare | chime | whoosh | bloop | question | pop
    #[serde(default)]
    pub sfx: Vec<(f32, String)>,
}
fn d_kicker() -> String {
    "HOW IT'S MADE".into()
}
fn d_block() -> f32 {
    16.0 * actor::PX
}
fn d_fov() -> f32 {
    38.0
}

#[derive(Deserialize, Clone)]
pub struct ActorScript {
    #[serde(default = "d_skin")]
    pub skin: String,
    #[serde(default)]
    pub outfit: Option<String>,
    #[serde(default)]
    pub hat: Option<String>,
    /// [t, x, y, z] feet positions, linear in between.
    pub keys: Vec<[f32; 4]>,
    /// [t, x, z]: face this point when standing (until the next look key).
    #[serde(default)]
    pub look: Vec<[f32; 3]>,
    /// [t0, t1, action]
    #[serde(default)]
    pub acts: Vec<(f32, f32, String)>,
    /// [t0, t1, item] small items drawn in the right hand (see actor::item).
    #[serde(default)]
    pub items: Vec<(f32, f32, String)>,
}
fn d_skin() -> String {
    "steve".into()
}

#[derive(Deserialize)]
pub struct PropScript {
    pub model: PathBuf,
    /// [t, "world", x, y, z, yaw_deg] | [t, "hand", actor] | [t, "hide"] | [t, "build", x, y, z, secs]
    pub keys: Vec<Vec<Value>>,
}

enum Key {
    World(Vec3, f32),
    Hand(usize),
    Hide,
    Build(Vec3),
}

struct Prop {
    anim: BuildAnim,
    keys: Vec<(f32, Key)>,
}

struct Cast {
    look: Look,
    s: ActorScript,
    /// Cumulative walked distance at each key.
    walked: Vec<f32>,
}

pub struct Story {
    script: Script,
    world: Vec<Instance>,
    cast: Vec<Cast>,
    props: Vec<Prop>,
    width: u32,
    height: u32,
    fps: u32,
    site: String,
    volume: f32,
    encoder: String,
    theme: Theme,
}

fn num(v: &Value) -> Result<f32> {
    v.as_f64().map(|x| x as f32).context("expected a number")
}

impl Story {
    #[allow(clippy::too_many_arguments)]
    pub fn load(path: &Path, height: u32, fps: u32, site: String, volume: f32, encoder: String, theme: Option<String>) -> Result<Self> {
        let dir = path.parent().unwrap_or(Path::new("."));
        let script: Script = serde_json::from_str(&std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?)
            .with_context(|| format!("parse {}", path.display()))?;
        let load_grid = |p: &Path| -> Result<crate::voxels::VoxelGrid> {
            let text = std::fs::read_to_string(dir.join(p)).with_context(|| format!("read {}", p.display()))?;
            crate::voxels::BuildSpec::from_llm_text(&text)?.voxelize()
        };
        // World: static, in blocks. Block (i, j, k) covers voxels [i, j - 1, k] * block, so layer 0
        // (paths, plaza, water) sits flush under the grass plane and layer 1 stands on it.
        let grid = load_grid(&script.world)?;
        let anim = BuildAnim::new(&grid, 0.01);
        let lo = anim.origin.as_vec3();
        let offset = lo + Vec3::new(anim.size.x as f32 * 0.5, -1.0, anim.size.z as f32 * 0.5);
        let mut world = Vec::new();
        anim.instances(1e6, offset, &mut world);
        let b = script.block;
        for i in &mut world {
            i.pos = (Vec3::from_array(i.pos) * b).to_array();
            i.size = (Vec3::from_array(i.size) * b).to_array();
        }
        eprintln!("  world: {} voxels, {} visible", grid.count(), world.len());

        let cast = script
            .actors
            .iter()
            .map(|a| {
                let mut walked = vec![0.0];
                for w in a.keys.windows(2) {
                    let d = Vec3::new(w[1][1] - w[0][1], w[1][2] - w[0][2], w[1][3] - w[0][3]).length();
                    walked.push(walked.last().unwrap() + d);
                }
                Cast { look: actor::look_of(&a.skin, a.outfit.as_deref()), s: a.clone(), walked }
            })
            .collect();

        let mut props = Vec::new();
        for p in &script.props {
            let mut keys = Vec::new();
            let mut build_secs = 0.8;
            for k in &p.keys {
                let t = num(k.first().context("empty prop key")?)?;
                let kind = k.get(1).and_then(|v| v.as_str()).unwrap_or("hide");
                let v3 = |i: usize| -> Result<Vec3> { Ok(Vec3::new(num(&k[i])?, num(&k[i + 1])?, num(&k[i + 2])?)) };
                let key = match kind {
                    "world" => Key::World(v3(2)?, k.get(5).map(num).transpose()?.unwrap_or(0.0)),
                    "hand" => Key::Hand(k.get(2).and_then(|v| v.as_u64()).context("hand needs an actor index")? as usize),
                    "build" => {
                        build_secs = k.get(5).map(num).transpose()?.unwrap_or(build_secs);
                        Key::Build(v3(2)?)
                    }
                    "hide" => Key::Hide,
                    other => bail!("unknown prop key {other:?}"),
                };
                keys.push((t, key));
            }
            let grid = load_grid(&p.model)?;
            props.push(Prop { anim: BuildAnim::new(&grid, build_secs), keys });
        }
        let theme = Theme::by_name(theme.as_deref().or(script.theme.as_deref()).unwrap_or("day"));
        let height = height - height % 16;
        let width = (height * 9 / 16) & !1;
        Ok(Self { script, world, cast, props, width, height, fps, site, volume, encoder, theme })
    }

    pub fn duration(&self) -> f32 {
        self.script.duration
    }

    // ------------------------------------------------------------------ actors

    fn actor_pos(&self, i: usize, t: f32) -> (Vec3, f32, Option<Vec3>) {
        let c = &self.cast[i];
        let k = &c.s.keys;
        let v = |j: usize| Vec3::new(k[j][1], k[j][2], k[j][3]);
        if k.is_empty() {
            return (Vec3::ZERO, 0.0, None);
        }
        if t <= k[0][0] {
            return (v(0), 0.0, None);
        }
        for j in 0..k.len() - 1 {
            let (t0, t1) = (k[j][0], k[j + 1][0]);
            if t < t1 {
                let f = ((t - t0) / (t1 - t0).max(1e-4)).clamp(0.0, 1.0);
                let pos = v(j).lerp(v(j + 1), f);
                let walked = c.walked[j] + (c.walked[j + 1] - c.walked[j]) * f;
                let d = v(j + 1) - v(j);
                let moving = d.length() / (t1 - t0).max(1e-4) > 0.5;
                return (pos, walked, moving.then_some(d));
            }
        }
        (v(k.len() - 1), *c.walked.last().unwrap(), None)
    }

    /// Facing at time t (before smoothing): walking direction, else look target, else last heading.
    fn raw_yaw(&self, i: usize, t: f32) -> f32 {
        let (pos, _, moving) = self.actor_pos(i, t);
        if let Some(d) = moving {
            return d.x.atan2(d.z);
        }
        let look = self.cast[i].s.look.iter().rev().find(|l| l[0] <= t);
        if let Some(l) = look {
            let d = Vec3::new(l[1] - pos.x, 0.0, l[2] - pos.z);
            if d.length_squared() > 0.01 {
                return d.x.atan2(d.z);
            }
        }
        // last movement direction
        let k = &self.cast[i].s.keys;
        for j in (0..k.len().saturating_sub(1)).rev() {
            if k[j + 1][0] <= t {
                let d = Vec3::new(k[j + 1][1] - k[j][1], 0.0, k[j + 1][3] - k[j][3]);
                if d.length_squared() > 0.01 {
                    return d.x.atan2(d.z);
                }
            }
        }
        0.0
    }

    fn carrying(&self, i: usize, t: f32) -> bool {
        self.props.iter().any(|p| matches!(prop_key(p, t), Some((_, Key::Hand(a), _)) if *a == i))
    }

    fn pose(&self, i: usize, t: f32) -> Pose {
        let (pos, walked, moving) = self.actor_pos(i, t);
        let mut p = Pose { pos, ..Default::default() };
        // Smooth turning: average the heading over the last ~0.25 s.
        let (mut sx, mut sz) = (0.0, 0.0);
        for k in 0..6 {
            let y = self.raw_yaw(i, t - k as f32 * 0.05);
            sx += y.sin();
            sz += y.cos();
        }
        p.yaw = sx.atan2(sz);
        let carrying = self.carrying(i, t);
        if moving.is_some() {
            actor::walk_pose(&mut p, walked);
            if carrying {
                p.arms = [(-1.35, -0.2), (-1.35, -0.2)];
            }
            return p;
        }
        match self.cast[i].s.acts.iter().find(|a| a.0 <= t && t < a.1) {
            Some((t0, _, action)) => actor::act_pose(&mut p, action, t - t0, t),
            None => {
                actor::act_pose(&mut p, if carrying { "carry" } else { "idle" }, 0.0, t);
            }
        }
        p
    }

    // ------------------------------------------------------------------ frame

    fn scene(&self, t: f32, inst: &mut Vec<Instance>) {
        inst.clear();
        inst.extend_from_slice(&self.world);
        let poses: Vec<Pose> = (0..self.cast.len()).map(|i| self.pose(i, t)).collect();
        for (i, c) in self.cast.iter().enumerate() {
            let item = c.s.items.iter().find(|it| it.0 <= t && t < it.1).map(|it| it.2.as_str());
            actor::draw(&c.look, c.s.hat.as_deref(), item, &poses[i], inst);
        }
        for p in &self.props {
            match prop_key(p, t) {
                None | Some((_, Key::Hide, _)) => {}
                Some((t0, Key::Build(pos), _)) => p.anim.instances(t - t0, *pos, inst),
                Some((t0, Key::World(pos, yaw), next)) => {
                    let (pos, yaw) = match next {
                        Some((t1, Key::World(p1, y1))) => {
                            let f = ((t - t0) / (t1 - t0).max(1e-4)).clamp(0.0, 1.0);
                            let f = f * f * (3.0 - 2.0 * f);
                            (pos.lerp(*p1, f), yaw + (y1 - yaw) * f)
                        }
                        _ => (*pos, *yaw),
                    };
                    if yaw.abs() < 0.01 {
                        p.anim.instances(1e6, pos, inst);
                    } else {
                        rotated(&p.anim, pos, Quat::from_rotation_y(yaw.to_radians()), inst);
                    }
                }
                Some((_, Key::Hand(a), _)) => {
                    let pose = &poses[*a];
                    let hand = actor::hand(pose);
                    let fwd = Quat::from_rotation_y(pose.yaw);
                    rotated(&p.anim, hand + Vec3::new(0.0, -1.5, 0.0) + fwd * Vec3::new(0.0, 0.0, 0.8), fwd, inst);
                }
            }
        }
    }

    fn view(&self, t: f32) -> View {
        let k = &self.script.camera;
        let at = |j: usize| (Vec3::new(k[j][1], k[j][2], k[j][3]), Vec3::new(k[j][4], k[j][5], k[j][6]));
        let (mut eye, mut target) = at(0);
        for j in 0..k.len() {
            if k[j][0] > t {
                break;
            }
            (eye, target) = at(j);
            if let Some(n) = k.get(j + 1) {
                if n[0] > t && n[7] < 0.5 {
                    let f = ((t - k[j][0]) / (n[0] - k[j][0]).max(1e-4)).clamp(0.0, 1.0);
                    let (e1, t1) = at(j + 1);
                    eye = eye.lerp(e1, f);
                    target = target.lerp(t1, f);
                }
            }
        }
        View {
            eye,
            target,
            fov_y_deg: self.script.fov,
            scene_center: target,
            scene_radius: 70.0,
            ground_y: -0.05,
            shift: [0.0, 0.0],
            time: t,
            fog: 2.2,
        }
    }

    fn overlay(&self, t: f32, frame: &mut [u8], text: &mut TextCache) {
        let (w, h) = (self.width as usize, self.height as usize);
        let s = h as f32 / 1920.0;
        let cx = w as f32 * 0.5;
        let white = [255, 255, 255];
        let yellow = [255, 214, 10];
        let mut top = 130.0 * s;
        let site = self.site.to_uppercase();
        let l = text.get(&site, Style::new(50.0 * s, white));
        l.blit(frame, w, h, cx, top, 1.0);
        top += l.h as f32 - 10.0 * s;
        let k = text.get(&self.script.kicker, Style::new(60.0 * s, yellow));
        k.blit(frame, w, h, cx, top, 1.0);
        top += k.h as f32 - 8.0 * s;
        let (big_text, size, lines) =
            if t < self.script.hook_until { (self.script.hook.as_str(), 120.0, 2) } else { (self.script.title.as_str(), 112.0, 1) };
        let mut big = Style::new(size * s, white);
        big.max_width = 1000.0 * s;
        let big = text.fit(big_text, big, lines, 64.0 * s);
        text.get(big_text, big).blit(frame, w, h, cx, top, 1.0);
        if let Some((t0, _, label, line)) = self.script.captions.iter().find(|c| c.0 <= t && t < c.1) {
            let fade = ((t - t0) / 0.25).clamp(0.0, 1.0);
            text.get(label, Style::new(104.0 * s, yellow)).blit(frame, w, h, cx, 1340.0 * s + (1.0 - fade) * 40.0 * s, fade);
            let mut st = Style::new(80.0 * s, white);
            st.max_width = 960.0 * s;
            let st = text.fit(line, st, 2, 52.0 * s);
            text.get(line, st).blit(frame, w, h, cx, 1460.0 * s + (1.0 - fade) * 30.0 * s, fade);
        }
    }

    fn frame(&self, t: f32, r: &mut Renderer, text: &mut TextCache, inst: &mut Vec<Instance>, buf: &mut Vec<u8>) -> Result<()> {
        let scene_t = if t < self.script.hook_until { self.script.hook_scene + t } else { t };
        self.scene(scene_t, inst);
        r.render(inst, &self.view(t), buf)?;
        self.overlay(t, buf, text);
        Ok(())
    }

    // ------------------------------------------------------------------ sound

    fn soundtrack(&self) -> Track {
        let dur = self.script.duration;
        let mut tr = Track::new(dur);
        tr.master = self.volume;
        tr.boost = true;
        for bed in &self.script.ambience {
            match bed.as_str() {
                "birds" => tr.birds(0.4, dur, 7),
                "wind" => tr.wind(0.0, dur, 0.04, 11),
                "bees" => tr.bees(0.0, dur, 0.04),
                other => eprintln!("  warning: unknown ambience {other:?}"),
            }
        }
        for (i, c) in self.cast.iter().enumerate() {
            // a footstep every 3.2 voxels walked (walked distance only grows while moving)
            let k = &c.s.keys;
            let mut next = 1.6;
            for j in 0..k.len().saturating_sub(1) {
                let (w0, w1) = (c.walked[j], c.walked[j + 1]);
                while next <= w1 {
                    let f = (next - w0) / (w1 - w0).max(1e-4);
                    tr.footstep(k[j][0] + (k[j + 1][0] - k[j][0]) * f);
                    next += 3.2;
                }
            }
            for (n, (t0, t1, action)) in c.s.acts.iter().enumerate() {
                for (m, (t, len, kind)) in actor::action_sounds(action, *t0, *t1).into_iter().enumerate() {
                    let seed = (i * 104729 + n * 7919 + m * 31) as u32;
                    match kind {
                        "swish" => tr.swish(t, len, 1100.0, 0.16, seed),
                        "thump" => tr.thump(t, 0.27),
                        "knock" => tr.knock(t, 0.12, seed),
                        "pop" => tr.pop(t, 0.22),
                        "pour" => tr.pour(t, len, 0.09, seed),
                        "munch" => tr.munch(t, 0.18, seed),
                        "hop" => tr.footstep(t),
                        _ => {}
                    }
                }
            }
        }
        for p in &self.props {
            for (t0, key) in &p.keys {
                if let Key::Build(..) = key {
                    tr.build_thocks(*t0, p.anim.landing_times(), p.anim.voxels.len());
                }
            }
        }
        for (t, kind) in &self.script.sfx {
            match kind.as_str() {
                "fanfare" => tr.fanfare(*t),
                "chime" => tr.chime(*t),
                "whoosh" => tr.whoosh(*t),
                "bloop" => tr.bloop(*t, 0.16),
                "question" => tr.question(*t),
                "pop" => tr.pop(*t, 0.22),
                other => eprintln!("  warning: unknown sfx {other:?}"),
            }
        }
        tr
    }

    // ------------------------------------------------------------------ output

    pub fn render_video(&self, text: &mut TextCache, out: &Path) -> Result<()> {
        let mut r = Renderer::new(self.width, self.height, self.theme)?;
        r.ground_block = self.script.block;
        let wav = out.with_extension("wav");
        self.soundtrack().write_wav(&wav)?;
        let mut enc = Encoder::start(out, self.width, self.height, self.fps, Some(&wav), &self.encoder)?;
        let frames = (self.script.duration * self.fps as f32).ceil() as u32;
        let (mut inst, mut buf) = (Vec::new(), Vec::new());
        let started = std::time::Instant::now();
        for f in 0..frames {
            let t = f as f32 / self.fps as f32;
            self.frame(t, &mut r, text, &mut inst, &mut buf)?;
            enc.frame(&buf)?;
            if f % self.fps == 0 {
                eprint!("\r  frame {f}/{frames} ({:.0} fps)  ", f as f32 / started.elapsed().as_secs_f32().max(1e-3));
            }
        }
        enc.finish()?;
        let _ = std::fs::remove_file(&wav);
        eprintln!("\r  {frames} frames in {:.1}s -> {}", started.elapsed().as_secs_f32(), out.display());
        Ok(())
    }

    pub fn render_still(&self, text: &mut TextCache, t: f32, out: &Path) -> Result<()> {
        let mut r = Renderer::new(self.width, self.height, self.theme)?;
        r.ground_block = self.script.block;
        let (mut inst, mut buf) = (Vec::new(), Vec::new());
        self.frame(t, &mut r, text, &mut inst, &mut buf)?;
        video::write_png(out, &buf, self.width, self.height)
    }
}

/// The prop's active key at `t`, with the key after it (for sliding between world positions).
fn prop_key(p: &Prop, t: f32) -> Option<(f32, &Key, Option<(f32, &Key)>)> {
    let i = p.keys.iter().rposition(|(k, _)| *k <= t)?;
    let next = p.keys.get(i + 1).map(|(t1, k)| (*t1, k));
    Some((p.keys[i].0, &p.keys[i].1, next))
}

/// Prop voxels rotated about its bottom centre placed at `pos` (no AO; props are small).
fn rotated(a: &BuildAnim, pos: Vec3, rot: Quat, out: &mut Vec<Instance>) {
    let center = Vec3::new(a.size.x as f32 * 0.5, 0.0, a.size.z as f32 * 0.5);
    for (p, c, _) in &a.voxels {
        out.push(Instance::rotated(pos + rot * (p.as_vec3() - center), Vec3::ONE, rot, *c));
    }
}

