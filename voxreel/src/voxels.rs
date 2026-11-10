//! Build description DSL (what LLMs emit) and its rasterization into a voxel grid.
//!
//! Coordinates: integer voxels, Y is up, the ground is y = 0, `size` is [x, y, z].
//! Ops are applied in order; a later op overwrites earlier voxels, and `"color": null`
//! (or `"op": "erase"`) carves voxels away.

use anyhow::{bail, Context, Result};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSpec {
    #[serde(default)]
    pub title: String,
    pub size: [i32; 3],
    pub ops: Vec<Op>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Box {
        from: [f32; 3],
        to: [f32; 3],
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        hollow: bool,
    },
    Sphere {
        center: [f32; 3],
        radius: f32,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        hollow: bool,
    },
    Ellipsoid {
        center: [f32; 3],
        radii: [f32; 3],
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        hollow: bool,
    },
    Cylinder {
        base: [f32; 3],
        radius: f32,
        height: f32,
        #[serde(default = "default_axis")]
        axis: String,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        hollow: bool,
    },
    Cone {
        base: [f32; 3],
        radius: f32,
        height: f32,
        #[serde(default = "default_axis")]
        axis: String,
        #[serde(default)]
        color: Option<String>,
    },
    Line {
        from: [f32; 3],
        to: [f32; 3],
        #[serde(default = "default_line_radius")]
        radius: f32,
        #[serde(default)]
        color: Option<String>,
    },
    Voxels {
        points: Vec<[i32; 3]>,
        #[serde(default)]
        color: Option<String>,
    },
    Erase {
        from: [f32; 3],
        to: [f32; 3],
    },
}

fn default_axis() -> String {
    "y".into()
}
fn default_line_radius() -> f32 {
    0.5
}

/// Dense voxel grid. `0` = empty, otherwise packed 0x00RRGGBB + 1<<24 marker.
#[derive(Clone)]
pub struct VoxelGrid {
    pub size: IVec3,
    cells: Vec<u32>,
}

const FILLED: u32 = 1 << 24;

impl VoxelGrid {
    pub fn new(size: IVec3) -> Self {
        let n = (size.x * size.y * size.z).max(0) as usize;
        Self { size, cells: vec![0; n] }
    }

    #[inline]
    fn idx(&self, p: IVec3) -> Option<usize> {
        if p.x < 0 || p.y < 0 || p.z < 0 || p.x >= self.size.x || p.y >= self.size.y || p.z >= self.size.z {
            return None;
        }
        Some(((p.y * self.size.z + p.z) * self.size.x + p.x) as usize)
    }

    #[inline]
    pub fn get(&self, p: IVec3) -> Option<[u8; 3]> {
        let c = self.cells[self.idx(p)?];
        (c & FILLED != 0).then(|| [(c >> 16) as u8, (c >> 8) as u8, c as u8])
    }

    #[inline]
    pub fn set(&mut self, p: IVec3, color: Option<[u8; 3]>) {
        if let Some(i) = self.idx(p) {
            self.cells[i] = match color {
                Some([r, g, b]) => FILLED | (r as u32) << 16 | (g as u32) << 8 | b as u32,
                None => 0,
            };
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (IVec3, [u8; 3])> + '_ {
        let s = self.size;
        (0..s.y).flat_map(move |y| {
            (0..s.z).flat_map(move |z| (0..s.x).filter_map(move |x| {
                let p = IVec3::new(x, y, z);
                self.get(p).map(|c| (p, c))
            }))
        })
    }

    pub fn count(&self) -> usize {
        self.cells.iter().filter(|c| **c & FILLED != 0).count()
    }

    /// Tight bounds of filled voxels (min inclusive, max exclusive).
    pub fn bounds(&self) -> Option<(IVec3, IVec3)> {
        let mut lo = IVec3::MAX;
        let mut hi = IVec3::MIN;
        for (p, _) in self.iter() {
            lo = lo.min(p);
            hi = hi.max(p + IVec3::ONE);
        }
        (lo.x <= hi.x).then_some((lo, hi))
    }

    /// Removes voxels that can never be seen (all 6 neighbours filled) only for counting;
    /// kept here as a helper for stats.
    pub fn surface_count(&self) -> usize {
        self.iter()
            .filter(|(p, _)| NEIGHBOURS.iter().any(|d| self.get(*p + *d).is_none()))
            .count()
    }
}

pub const NEIGHBOURS: [IVec3; 6] = [
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(0, -1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
];

pub fn parse_color(s: &str) -> Result<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    let h = if h.len() == 3 {
        h.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        h.to_string()
    };
    if h.len() != 6 {
        bail!("bad color {s:?}");
    }
    let v = u32::from_str_radix(&h, 16).with_context(|| format!("bad color {s:?}"))?;
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// Max edge length we accept, so a model asking for 1000^3 cannot blow up memory/render time.
pub const MAX_SIZE: i32 = 96;

impl BuildSpec {
    /// Lenient parsing of LLM output: strips markdown fences / prose around the JSON object,
    /// and skips individual ops that fail to parse instead of rejecting the whole build.
    pub fn from_llm_text(text: &str) -> Result<Self> {
        let start = text.find('{').context("no JSON object in response")?;
        let end = text.rfind('}').context("no JSON object in response")?;
        let v: serde_json::Value = serde_json::from_str(&text[start..=end]).context("response is not valid JSON")?;
        let size: [i32; 3] = serde_json::from_value(v.get("size").cloned().context("missing size")?)?;
        let title = v.get("title").and_then(|t| t.as_str()).unwrap_or_default().to_string();
        let mut ops = Vec::new();
        let mut skipped = 0;
        for raw in v.get("ops").and_then(|o| o.as_array()).context("missing ops")? {
            match serde_json::from_value::<Op>(raw.clone()) {
                Ok(op) => ops.push(op),
                Err(_) => skipped += 1,
            }
        }
        if skipped > 0 {
            eprintln!("  warning: skipped {skipped} unparseable ops");
        }
        Ok(Self { title, size, ops })
    }

    pub fn voxelize(&self) -> Result<VoxelGrid> {
        let size = IVec3::from_array(self.size).clamp(IVec3::ONE, IVec3::splat(MAX_SIZE));
        let mut g = VoxelGrid::new(size);
        for op in &self.ops {
            apply(&mut g, op)?;
        }
        Ok(g)
    }
}

fn col(c: &Option<String>) -> Result<Option<[u8; 3]>> {
    c.as_deref().map(parse_color).transpose()
}

/// Visits every voxel center in [lo, hi] (clamped to the grid) and sets it when `inside` says so.
/// `inside` returns Some(depth) for points inside the shape; depth < 1 means "on the shell",
/// which is what `hollow` keeps.
fn fill(g: &mut VoxelGrid, lo: Vec3, hi: Vec3, color: Option<[u8; 3]>, hollow: bool, inside: impl Fn(Vec3) -> Option<f32>) {
    let lo = lo.floor().as_ivec3().max(IVec3::ZERO);
    let hi = hi.ceil().as_ivec3().min(g.size - IVec3::ONE);
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = IVec3::new(x, y, z);
                if let Some(depth) = inside(p.as_vec3() + 0.5) {
                    if !hollow || depth < 1.0 {
                        g.set(p, color);
                    }
                }
            }
        }
    }
}

fn axis_frame(axis: &str) -> (usize, usize, usize) {
    match axis {
        "x" => (0, 1, 2),
        "z" => (2, 0, 1),
        _ => (1, 0, 2),
    }
}

fn apply(g: &mut VoxelGrid, op: &Op) -> Result<()> {
    match op {
        Op::Box { from, to, color, hollow } => {
            let a = Vec3::from_array(*from).min(Vec3::from_array(*to));
            let b = Vec3::from_array(*from).max(Vec3::from_array(*to));
            // Box corners are inclusive voxel coordinates, the way people (and LLMs) write them.
            let (a, b) = (a, b + 1.0);
            fill(g, a, b, col(color)?, *hollow, |p| {
                let d = (p - a).min(b - p);
                (d.min_element() > 0.0).then(|| d.min_element())
            });
        }
        Op::Sphere { center, radius, color, hollow } => {
            let c = Vec3::from_array(*center) + 0.5;
            let r = radius.max(0.5);
            fill(g, c - r, c + r, col(color)?, *hollow, |p| {
                let d = r - p.distance(c);
                (d >= 0.0).then_some(d)
            });
        }
        Op::Ellipsoid { center, radii, color, hollow } => {
            let c = Vec3::from_array(*center) + 0.5;
            let r = Vec3::from_array(*radii).max(Vec3::splat(0.5));
            fill(g, c - r, c + r, col(color)?, *hollow, |p| {
                let q = (p - c) / r;
                let len = q.length();
                // Approximate distance to the surface in voxels.
                (len <= 1.0).then(|| (1.0 - len) * r.min_element())
            });
        }
        Op::Cylinder { base, radius, height, axis, color, hollow } => {
            let (ax, u, v) = axis_frame(axis);
            let b = Vec3::from_array(*base) + 0.5;
            let r = radius.max(0.5);
            let mut lo = b - r;
            let mut hi = b + r;
            lo[ax] = b[ax] - 0.5 + height.min(0.0);
            hi[ax] = b[ax] - 0.5 + height.max(0.0);
            let (h0, h1) = (lo[ax], hi[ax]);
            fill(g, lo, hi, col(color)?, *hollow, |p| {
                let dr = r - ((p[u] - b[u]).powi(2) + (p[v] - b[v]).powi(2)).sqrt();
                let dh = (p[ax] - h0).min(h1 - p[ax]);
                (dr >= 0.0 && dh > 0.0).then(|| dr.min(dh))
            });
        }
        Op::Cone { base, radius, height, axis, color } => {
            let (ax, u, v) = axis_frame(axis);
            let b = Vec3::from_array(*base) + 0.5;
            let r = radius.max(0.5);
            let h = if height.abs() < 1.0 { 1.0 } else { *height };
            let mut lo = b - r;
            let mut hi = b + r;
            lo[ax] = b[ax] - 0.5 + h.min(0.0);
            hi[ax] = b[ax] - 0.5 + h.max(0.0);
            let start = b[ax] - 0.5;
            fill(g, lo, hi, col(color)?, false, |p| {
                let t = (p[ax] - start) / h;
                if !(0.0..=1.0).contains(&t) {
                    return None;
                }
                let rr = r * (1.0 - t) + 0.35;
                let d = ((p[u] - b[u]).powi(2) + (p[v] - b[v]).powi(2)).sqrt();
                (d <= rr).then_some(1.0)
            });
        }
        Op::Line { from, to, radius, color } => {
            let a = Vec3::from_array(*from) + 0.5;
            let b = Vec3::from_array(*to) + 0.5;
            let r = radius.max(0.5);
            fill(g, a.min(b) - r, a.max(b) + r, col(color)?, false, |p| {
                let ab = b - a;
                let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
                (p.distance(a + ab * t) <= r).then_some(1.0)
            });
        }
        Op::Voxels { points, color } => {
            let c = col(color)?;
            for p in points {
                g.set(IVec3::from_array(*p), c);
            }
        }
        Op::Erase { from, to } => {
            let a = Vec3::from_array(*from).min(Vec3::from_array(*to));
            let b = Vec3::from_array(*from).max(Vec3::from_array(*to)) + 1.0;
            fill(g, a, b, None, false, |p| ((p - a).min(b - p).min_element() > 0.0).then_some(1.0));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_is_inclusive_and_hollow_works() {
        let spec: BuildSpec = serde_json::from_str(
            r##"{"size":[8,8,8],"ops":[{"op":"box","from":[0,0,0],"to":[2,2,2],"color":"#ff0000"}]}"##,
        )
        .unwrap();
        assert_eq!(spec.voxelize().unwrap().count(), 27);
        let spec: BuildSpec = serde_json::from_str(
            r##"{"size":[8,8,8],"ops":[{"op":"box","from":[0,0,0],"to":[2,2,2],"color":"#f00","hollow":true}]}"##,
        )
        .unwrap();
        assert_eq!(spec.voxelize().unwrap().count(), 26);
    }

    #[test]
    fn llm_text_is_parsed_leniently() {
        let text = "Sure! ```json\n{\"title\":\"t\",\"size\":[4,4,4],\"ops\":[{\"op\":\"box\",\"from\":[0,0,0],\"to\":[0,0,0],\"color\":\"#00ff00\"},{\"op\":\"teleport\"}]}\n```";
        let spec = BuildSpec::from_llm_text(text).unwrap();
        assert_eq!(spec.ops.len(), 1);
        assert_eq!(spec.voxelize().unwrap().count(), 1);
    }

    #[test]
    fn erase_and_null_color_carve() {
        let spec: BuildSpec = serde_json::from_str(
            r##"{"size":[4,4,4],"ops":[{"op":"box","from":[0,0,0],"to":[3,3,3],"color":"#fff"},{"op":"erase","from":[0,0,0],"to":[3,1,3]},{"op":"box","from":[0,3,0],"to":[3,3,3],"color":null}]}"##,
        )
        .unwrap();
        assert_eq!(spec.voxelize().unwrap().count(), 16);
    }
}
