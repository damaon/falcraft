//! Blocky characters (Minecraft player proportions: head 8x8x8 px, body 8x12x4, limbs 4x12x4)
//! that walk along a path and then act (stir, knead, water, pick, carry, cheer...) inside a
//! scene. Every skin pixel is a small box; a whole limb shares one rotation, so the GPU rotates
//! the pixels and they stay glued together.

use crate::render::Instance;
use glam::{IVec3, Quat, Vec3};
use serde::Deserialize;

/// Size of one skin pixel in voxels (the character is 32 px = 19 voxels tall).
pub const PX: f32 = 0.6;
pub const HEIGHT: f32 = 32.0 * PX;

/// How a character is placed and what it does, in the build's own voxel coordinates.
#[derive(Deserialize, Clone)]
pub struct ActorSpec {
    /// steve | alex
    #[serde(default = "d_skin")]
    pub skin: String,
    /// default | chef | farmer | beekeeper
    #[serde(default)]
    pub outfit: Option<String>,
    /// chef | straw | bee
    #[serde(default)]
    pub hat: Option<String>,
    /// [x, z] waypoints; the first one is where the character starts.
    pub path: Vec<[f32; 2]>,
    /// Floor height (spec voxels) the feet stand on.
    #[serde(default = "d_y")]
    pub y: f32,
    /// idle | stir | knead | chop | water | pick | carry | cheer | wave | eat
    #[serde(default = "d_action")]
    pub action: String,
    /// spoon | tomato | bucket | watering_can | pizza | rolling_pin | strawberry | scoop | ice_cream | honey_jar | flower
    #[serde(default)]
    pub item: Option<String>,
    /// Point to face once arrived; default keeps the walking direction.
    #[serde(default)]
    pub face: Option<[f32; 2]>,
    /// Seconds into the shot when walking starts.
    #[serde(default)]
    pub start: f32,
    /// Walking speed in voxels per second.
    #[serde(default = "d_speed")]
    pub speed: f32,
}
fn d_skin() -> String {
    "steve".into()
}
fn d_y() -> f32 {
    1.0
}
fn d_action() -> String {
    "idle".into()
}
fn d_speed() -> f32 {
    7.0
}

pub struct Actor {
    spec: ActorSpec,
    /// Waypoints in world space (feet), same frame as `BuildAnim::instances`.
    path: Vec<Vec3>,
    face: Option<Vec3>,
    look: Look,
}

#[derive(Clone, Copy)]
pub struct Look {
    skin: [u8; 3],
    hair: [u8; 3],
    iris: [u8; 3],
    mouth: [u8; 3],
    beard: bool,
    long_hair: bool,
    shirt: [u8; 3],
    long_sleeves: bool,
    pants: [u8; 3],
    shoes: [u8; 3],
    apron: Option<[u8; 3]>,
    straps: Option<[u8; 3]>,
}

fn rgb(hex: u32) -> [u8; 3] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}

pub fn look_of(skin: &str, outfit: Option<&str>) -> Look {
    let mut l = match skin {
        "alex" => Look {
            skin: rgb(0xf1c49b),
            hair: rgb(0xd4682a),
            iris: rgb(0x2f8a3a),
            mouth: rgb(0xd98a7a),
            beard: false,
            long_hair: true,
            shirt: rgb(0x7ab55c),
            long_sleeves: false,
            pants: rgb(0x6b4a2a),
            shoes: rgb(0x4a4a4a),
            apron: None,
            straps: None,
        },
        _ => Look {
            skin: rgb(0xc69072),
            hair: rgb(0x3b2416),
            iris: rgb(0x4a3c9e),
            mouth: rgb(0x6a4030),
            beard: true,
            long_hair: false,
            shirt: rgb(0x1aa5a5),
            long_sleeves: false,
            pants: rgb(0x3b3b9e),
            shoes: rgb(0x555555),
            apron: None,
            straps: None,
        },
    };
    match outfit {
        Some("chef") => {
            l.shirt = rgb(0xf4f4f4);
            l.long_sleeves = true;
            l.apron = Some(rgb(0xe5393a));
            l.pants = rgb(0x333333);
        }
        Some("farmer") => {
            l.pants = rgb(0x3a6fc4);
            l.straps = Some(rgb(0x3a6fc4));
        }
        Some("beekeeper") => {
            l.shirt = rgb(0xf2f2f2);
            l.pants = rgb(0xf2f2f2);
            l.long_sleeves = true;
            l.shoes = rgb(0xd9b46a);
        }
        _ => {}
    }
    l
}

/// Pose of one frame (angles in radians, bob in voxels).
#[derive(Default, Clone, Copy)]
pub struct Pose {
    pub pos: Vec3,
    pub yaw: f32,
    pub bob: f32,
    pub lean: f32,
    pub head_pitch: f32,
    /// [character's left (+x), right (-x)]: (pitch, roll outwards)
    pub arms: [(f32, f32); 2],
    pub legs: [f32; 2],
}

/// Legs and arms swinging, `walked` = distance covered so far (voxels).
pub fn walk_pose(p: &mut Pose, walked: f32) {
    let ph = walked * 1.25;
    let s = ph.sin();
    p.legs = [s * 0.7, -s * 0.7];
    p.arms = [(-s * 0.6, 0.05), (s * 0.6, 0.05)];
    p.bob = ph.cos().abs() * 0.25;
}

/// Arms/lean/head for an action `a` seconds after it started (`t`: absolute time for idling).
pub fn act_pose(p: &mut Pose, action: &str, a: f32, t: f32) {
    let idle = (t * 2.0).sin() * 0.04;
    p.arms = [(idle, 0.05), (-idle, 0.05)];
    match action {
        "stir" => {
            p.arms[1] = (-1.15 + 0.12 * (a * 9.0).sin(), 0.25 * (a * 9.0).cos());
            p.arms[0] = (-0.9, 0.1);
            p.head_pitch = 0.4;
        }
        "knead" => {
            let k = (a * 10.0).sin().abs();
            p.arms = [(-1.0 + 0.35 * k, -0.15), (-1.0 + 0.35 * k, -0.15)];
            p.lean = 0.12 + 0.06 * k;
            p.head_pitch = 0.45;
        }
        "chop" => {
            let k = 0.5 + 0.5 * (a * 8.0).sin();
            p.arms[1] = (-2.3 + 1.3 * k, 0.0);
            p.head_pitch = 0.35;
        }
        "water" => {
            p.arms[1] = (-1.25 + 0.08 * (a * 3.0).sin(), 0.1);
            p.lean = 0.1;
            p.head_pitch = 0.4;
        }
        "pick" => {
            let k = 0.5 + 0.5 * (a * 4.0).sin();
            p.arms[1] = (-0.7 - 0.9 * k, 0.1);
            p.arms[0] = (-0.5 - 0.6 * (1.0 - k), 0.1);
            p.lean = 0.25;
            p.head_pitch = 0.5;
        }
        "carry" => {
            p.arms = [(-1.35, -0.2), (-1.35, -0.2)];
        }
        "cheer" => {
            let k = (a * 10.0).sin();
            p.arms = [(-2.9, 0.35 + 0.2 * k), (-2.9, 0.35 - 0.2 * k)];
            p.bob = 1.4 * (a * 6.0).sin().abs();
            p.head_pitch = -0.2;
        }
        "wave" => {
            p.arms[1] = (-2.7, 0.35 + 0.35 * (a * 10.0).sin());
        }
        "eat" => {
            p.arms[1] = (-2.0 + 0.25 * (a * 6.0).sin(), -0.3);
        }
        "push" => {
            // sliding something forward (into the oven)
            let k = (a * 2.5).min(1.0);
            p.arms = [(-1.5, -0.15), (-1.5, -0.15)];
            p.lean = 0.1 + 0.2 * k;
        }
        "look" => {
            p.head_pitch = 0.3;
        }
        _ => {}
    }
}

/// Sound cues of an action running over [t0, until): (time, length, kind), in sync with `act_pose`.
pub fn action_sounds(action: &str, t0: f32, until: f32) -> Vec<(f32, f32, &'static str)> {
    use std::f32::consts::PI;
    let mut out = Vec::new();
    if t0 >= until {
        return out;
    }
    let mut every = |period: f32, first: f32, len: f32, kind: &'static str| {
        let mut t = t0 + first;
        while t < until - 0.05 {
            out.push((t, len, kind));
            t += period;
        }
    };
    match action {
        "stir" => every(2.0 * PI / 9.0, 0.0, 0.4, "swish"),
        "knead" => every(PI / 10.0, PI / 20.0, 0.1, "thump"),
        "chop" => every(PI / 4.0, PI / 16.0, 0.1, "knock"),
        "pick" => every(PI / 2.0, PI / 8.0, 0.07, "pop"),
        "eat" => every(PI / 3.0, PI / 12.0, 0.3, "munch"),
        "cheer" => every(PI / 6.0, PI / 6.0, 0.1, "hop"),
        "water" => out.push((t0, until - t0, "pour")),
        "push" => out.push((t0, 0.5, "swish")),
        _ => {}
    }
    out
}

impl Actor {
    /// `origin`/`size`: the build's tight bounds (see `BuildAnim`), so spec coordinates line up
    /// with the voxels.
    pub fn new(spec: ActorSpec, origin: IVec3, size: IVec3) -> Self {
        let to_world = |p: [f32; 2]| {
            Vec3::new(
                p[0] + 0.5 - origin.x as f32 - size.x as f32 * 0.5,
                spec.y - origin.y as f32,
                p[1] + 0.5 - origin.z as f32 - size.z as f32 * 0.5,
            )
        };
        let mut path: Vec<Vec3> = spec.path.iter().map(|p| to_world(*p)).collect();
        if path.is_empty() {
            path.push(to_world([size.x as f32 * 0.5, size.z as f32 * 0.5]));
        }
        let face = spec.face.map(to_world);
        let look = look_of(&spec.skin, spec.outfit.as_deref());
        Self { spec, path, face, look }
    }

    fn path_len(&self) -> f32 {
        self.path.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    fn arrive_time(&self) -> f32 {
        self.spec.start + self.path_len() / self.spec.speed.max(0.1)
    }

    /// Points the camera must keep in frame.
    pub fn bounds(&self) -> Vec<Vec3> {
        let mut out = Vec::new();
        for p in &self.path {
            for (dx, dz) in [(-2.5, -2.5), (2.5, -2.5), (2.5, 2.5), (-2.5, 2.5)] {
                out.push(*p + Vec3::new(dx, 0.0, dz));
                out.push(*p + Vec3::new(dx, HEIGHT + 2.0, dz));
            }
        }
        out
    }

    /// Local times of footsteps, for the soundtrack.
    pub fn footsteps(&self) -> Vec<f32> {
        let stride = 3.2;
        let n = (self.path_len() / stride) as usize;
        (0..n).map(|i| self.spec.start + (i as f32 + 0.5) * stride / self.spec.speed.max(0.1)).collect()
    }

    /// Sound cues of the action, in sync with the arm swings in `pose`, from arrival until
    /// `until` (shot-local seconds): (time, length, kind).
    pub fn sound_events(&self, until: f32) -> Vec<(f32, f32, &'static str)> {
        action_sounds(&self.spec.action, self.arrive_time(), until)
    }

    fn pose(&self, t: f32) -> Pose {
        let mut p = Pose::default();
        let walked = ((t - self.spec.start) * self.spec.speed).clamp(0.0, self.path_len());
        // position + heading along the path
        let mut left = walked;
        let mut dir = Vec3::Z;
        p.pos = *self.path.last().unwrap();
        for w in self.path.windows(2) {
            let seg = w[0].distance(w[1]);
            if seg > 1e-3 {
                dir = (w[1] - w[0]) / seg;
            }
            if left <= seg {
                p.pos = w[0] + (w[1] - w[0]) * (left / seg.max(1e-3));
                break;
            }
            left -= seg;
        }
        if self.path.len() >= 2 && walked <= 0.0 {
            dir = (self.path[1] - self.path[0]).normalize_or(Vec3::Z);
        }
        let walk_yaw = dir.x.atan2(dir.z);
        let arrived = t >= self.arrive_time();
        let walking = t >= self.spec.start && !arrived && self.path.len() >= 2;
        p.yaw = walk_yaw;
        if arrived {
            if let Some(f) = self.face {
                let d = f - p.pos;
                if d.length_squared() > 1e-3 {
                    let target = d.x.atan2(d.z);
                    let k = ((t - self.arrive_time()) / 0.25).clamp(0.0, 1.0);
                    p.yaw = walk_yaw + wrap(target - walk_yaw) * k;
                }
            }
        }
        if walking {
            walk_pose(&mut p, walked);
            return p;
        }
        let a = if arrived { t - self.arrive_time() } else { 0.0 };
        act_pose(&mut p, if arrived { self.spec.action.as_str() } else { "idle" }, a, t);
        p
    }

    /// Instances for shot-local time `t`.
    pub fn instances(&self, t: f32, out: &mut Vec<Instance>) {
        draw(&self.look, self.spec.hat.as_deref(), self.spec.item.as_deref(), &self.pose(t), out);
    }
}

/// Body parts: (part, min px, size px, pivot px, rotation, rest pivot px).
fn parts(p: &Pose) -> [(Part, Vec3, Vec3, Vec3, Quat, Vec3); 6] {
    let lean = Quat::from_rotation_x(p.lean);
    let hip = Vec3::new(0.0, 12.0, 0.0);
    let upper = |v: Vec3| hip + lean * (v - hip);
    let arm_rot = |(pitch, roll): (f32, f32), side: f32| lean * Quat::from_rotation_x(pitch) * Quat::from_rotation_z(roll * side);
    let (l_sh, r_sh, neck) = (Vec3::new(6.0, 22.0, 0.0), Vec3::new(-6.0, 22.0, 0.0), Vec3::new(0.0, 24.0, 0.0));
    [
        (Part::Leg, Vec3::new(0.0, 0.0, -2.0), Vec3::new(4.0, 12.0, 4.0), Vec3::new(2.0, 12.0, 0.0), Quat::from_rotation_x(p.legs[0]), Vec3::new(2.0, 12.0, 0.0)),
        (Part::Leg, Vec3::new(-4.0, 0.0, -2.0), Vec3::new(4.0, 12.0, 4.0), Vec3::new(-2.0, 12.0, 0.0), Quat::from_rotation_x(p.legs[1]), Vec3::new(-2.0, 12.0, 0.0)),
        (Part::Body, Vec3::new(-4.0, 12.0, -2.0), Vec3::new(8.0, 12.0, 4.0), hip, lean, hip),
        (Part::Arm, Vec3::new(4.0, 12.0, -2.0), Vec3::new(4.0, 12.0, 4.0), upper(l_sh), arm_rot(p.arms[0], 1.0), l_sh),
        (Part::Arm, Vec3::new(-8.0, 12.0, -2.0), Vec3::new(4.0, 12.0, 4.0), upper(r_sh), arm_rot(p.arms[1], -1.0), r_sh),
        (Part::Head, Vec3::new(-4.0, 24.0, -4.0), Vec3::new(8.0, 8.0, 8.0), upper(neck), lean * Quat::from_rotation_x(p.head_pitch), neck),
    ]
}

/// World position of a body-local pixel point of part `i` (see `parts`).
fn place(p: &Pose, part: &(Part, Vec3, Vec3, Vec3, Quat, Vec3), local_px: Vec3) -> Vec3 {
    let (_, _, _, pivot, rot, rest) = part;
    let v = *pivot + *rot * (local_px - *rest);
    p.pos + Vec3::Y * p.bob + Quat::from_rotation_y(p.yaw) * (v * PX)
}

/// Where carried things go: between the hands when carrying, else the right hand.
pub fn hand(p: &Pose) -> Vec3 {
    let ps = parts(p);
    let right = place(p, &ps[4], Vec3::new(-6.0, 12.0, 0.0));
    let left = place(p, &ps[3], Vec3::new(6.0, 12.0, 0.0));
    if (p.arms[0].0 - p.arms[1].0).abs() < 0.05 {
        (right + left) * 0.5
    } else {
        right
    }
}

/// Instances of a character in pose `p`.
pub fn draw(l: &Look, hat_kind: Option<&str>, item_kind: Option<&str>, p: &Pose, out: &mut Vec<Instance>) {
    let root_rot = Quat::from_rotation_y(p.yaw);
    for (i, part) in parts(p).iter().enumerate() {
        let (kind, min, size, _, rot, _) = part;
        let world_rot = root_rot * *rot;
        let (w, h, d) = (size.x as i32, size.y as i32, size.z as i32);
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    if x > 0 && x < w - 1 && y > 0 && y < h - 1 && z > 0 && z < d - 1 {
                        continue;
                    }
                    let c = l.color(*kind, x, y, z, w, h, d);
                    out.push(Instance::rotated(place(p, part, *min + Vec3::new(x as f32, y as f32, z as f32)), Vec3::splat(PX), world_rot, c));
                }
            }
        }
        if matches!(kind, Part::Head) {
            for (bmin, bsize, c) in hat(hat_kind) {
                out.push(Instance::rotated(place(p, part, *min + bmin), bsize * PX, world_rot, c));
            }
        }
        if matches!(kind, Part::Arm) && i == 4 {
            for (bmin, bsize, c) in item(item_kind) {
                out.push(Instance::rotated(place(p, part, *min + bmin), bsize * PX, world_rot, c));
            }
        }
    }
}

fn wrap(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

#[derive(Clone, Copy)]
enum Part {
    Head,
    Body,
    Arm,
    Leg,
}

impl Look {
    #[allow(clippy::too_many_arguments)]
    fn color(&self, part: Part, x: i32, y: i32, z: i32, w: i32, h: i32, d: i32) -> [u8; 3] {
        let front = z == d - 1;
        match part {
            Part::Head => {
                let side = x == 0 || x == w - 1;
                let back = z == 0;
                if y == h - 1 {
                    return self.hair;
                }
                if back {
                    return if self.long_hair || y >= 2 { self.hair } else { self.skin };
                }
                if side {
                    let hair_to = if self.long_hair { 1 } else { 5 };
                    return if y >= hair_to || (z <= 1 && y >= 2) { self.hair } else { self.skin };
                }
                if front {
                    if y >= 7 || (y == 6 && (x == 0 || x == 7 || (self.long_hair && x <= 2))) {
                        return self.hair;
                    }
                    if y == 4 {
                        return match x {
                            1 | 6 => [255, 255, 255],
                            2 | 5 => self.iris,
                            _ => self.skin,
                        };
                    }
                    if y == 3 && (3..=4).contains(&x) {
                        return shade(self.skin, 0.85);
                    }
                    if y == 2 && (2..=5).contains(&x) {
                        return if self.beard || (3..=4).contains(&x) { self.mouth } else { self.skin };
                    }
                    if y == 1 && self.beard && (2..=5).contains(&x) {
                        return shade(self.mouth, 1.15);
                    }
                }
                self.skin
            }
            Part::Body => {
                if let Some(s) = self.straps {
                    if y <= 5 || ((x == 1 || x == 6) && (front || z == 0)) {
                        return s;
                    }
                }
                if let Some(a) = self.apron {
                    if front && y <= 9 && (1..=6).contains(&x) {
                        return if y == 9 { shade(a, 0.8) } else { a };
                    }
                }
                if front && y == h - 1 && (3..=4).contains(&x) && self.apron.is_none() {
                    return self.skin;
                }
                self.shirt
            }
            Part::Arm => {
                let sleeve = if self.long_sleeves { 2 } else { 8 };
                if y >= sleeve { self.shirt } else { self.skin }
            }
            Part::Leg => {
                if y <= 1 {
                    self.shoes
                } else {
                    self.pants
                }
            }
        }
    }
}

fn shade(c: [u8; 3], k: f32) -> [u8; 3] {
    c.map(|v| (v as f32 * k).clamp(0.0, 255.0) as u8)
}

/// Hat boxes in head-local px (head spans 0..8 on every axis).
fn hat(kind: Option<&str>) -> Vec<(Vec3, Vec3, [u8; 3])> {
    let b = |x: f32, y: f32, z: f32, w: f32, h: f32, d: f32, c: u32| (Vec3::new(x, y, z), Vec3::new(w, h, d), rgb(c));
    match kind {
        Some("chef") => vec![b(0.0, 8.0, 0.0, 8.0, 2.0, 8.0, 0xf6f6f6), b(-1.0, 10.0, -1.0, 10.0, 4.0, 10.0, 0xffffff)],
        Some("straw") => vec![b(-3.0, 8.0, -3.0, 14.0, 1.0, 14.0, 0xe6c35c), b(0.0, 9.0, 0.0, 8.0, 1.0, 8.0, 0xb5452e), b(0.0, 10.0, 0.0, 8.0, 2.0, 8.0, 0xe6c35c)],
        Some("bee") => vec![
            b(-2.0, 8.0, -2.0, 12.0, 1.0, 12.0, 0xf2f2f2),
            b(0.0, 9.0, 0.0, 8.0, 3.0, 8.0, 0xf2f2f2),
            // veil sides (the face stays visible)
            b(-2.0, 3.0, -2.0, 0.6, 5.0, 12.0, 0x50505a),
            b(9.4, 3.0, -2.0, 0.6, 5.0, 12.0, 0x50505a),
            b(-2.0, 3.0, -2.0, 12.0, 5.0, 0.6, 0x50505a),
        ],
        _ => vec![],
    }
}

/// Held item boxes in right-arm-local px (arm spans 0..4 x 0..12 x 0..4, hand at y = 0).
fn item(kind: Option<&str>) -> Vec<(Vec3, Vec3, [u8; 3])> {
    let b = |x: f32, y: f32, z: f32, w: f32, h: f32, d: f32, c: u32| (Vec3::new(x, y, z), Vec3::new(w, h, d), rgb(c));
    match kind {
        Some("spoon") => vec![b(1.5, -9.0, 1.5, 1.0, 9.0, 1.0, 0xc8c8d0), b(0.5, -11.0, 0.5, 3.0, 2.0, 3.0, 0xc8c8d0)],
        Some("tomato") => vec![b(0.0, -4.0, 0.0, 4.0, 4.0, 4.0, 0xe5393a), b(1.5, -4.6, 1.5, 1.0, 0.6, 1.0, 0x5fbf4f)],
        Some("strawberry") => vec![b(0.5, -3.5, 0.5, 3.0, 3.5, 3.0, 0xe5393a), b(0.5, -0.6, 0.5, 3.0, 0.6, 3.0, 0x5fbf4f)],
        Some("bucket") => vec![b(-1.0, -7.0, -1.0, 6.0, 6.0, 6.0, 0xb9bcc4), b(-0.5, -1.2, -0.5, 5.0, 0.4, 5.0, 0xf7f4ee)],
        Some("watering_can") => vec![b(-1.0, -7.0, -0.5, 6.0, 6.0, 5.0, 0x3a8be0), b(1.5, -6.0, 4.5, 1.0, 1.0, 5.0, 0x3a8be0)],
        Some("pizza") => vec![b(-4.0, -2.0, -3.0, 12.0, 1.0, 10.0, 0xf2c14e), b(-3.0, -1.0, -2.0, 10.0, 0.4, 8.0, 0xe5393a)],
        Some("rolling_pin") => vec![b(-5.0, -2.5, 0.5, 14.0, 3.0, 3.0, 0xc08552)],
        Some("scoop") => vec![b(1.5, -7.0, 1.5, 1.0, 7.0, 1.0, 0xc8c8d0), b(0.0, -10.0, 0.0, 4.0, 3.0, 4.0, 0xff7eb6)],
        Some("ice_cream") => vec![b(1.0, -5.0, 1.0, 2.0, 5.0, 2.0, 0xe0a75e), b(0.0, -8.0, 0.0, 4.0, 3.0, 4.0, 0xff7eb6)],
        Some("honey_jar") => vec![b(0.0, -6.0, 0.0, 4.0, 6.0, 4.0, 0xffb81c), b(-0.3, -6.8, -0.3, 4.6, 0.8, 4.6, 0xe5393a)],
        Some("flower") => vec![b(1.5, -6.0, 1.5, 1.0, 6.0, 1.0, 0x3f9a3a), b(0.5, -8.0, 0.5, 3.0, 2.0, 3.0, 0xff7eb6)],
        _ => vec![],
    }
}
