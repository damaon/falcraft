//! Build timeline: in what order and when each voxel drops into place, and turning a moment
//! in time into the GPU instance list (with per-corner ambient occlusion).

use crate::render::Instance;
use crate::voxels::{VoxelGrid, NEIGHBOURS};
use glam::{IVec3, Vec3};

/// How long one voxel takes to fall into place.
pub const DROP_SECS: f32 = 0.35;
const DROP_HEIGHT: f32 = 4.0;
const GLOW_SECS: f32 = 0.25;

pub struct BuildAnim {
    pub size: IVec3,
    /// Voxels in build order with their drop start time (seconds from build start).
    pub voxels: Vec<(IVec3, [u8; 3], f32)>,
    /// Lookup grid: start time per cell, f32::INFINITY when empty.
    start: Vec<f32>,
    pub duration: f32,
    /// Start time of the first voxel in each layer, for a smooth "current height".
    layer_starts: Vec<f32>,
}

impl BuildAnim {
    /// Layers go bottom-up like a 3D printer; inside a layer the blocks sweep diagonally,
    /// which reads as "being built" instead of popping in a whole slice at once.
    pub fn new(grid: &VoxelGrid, duration: f32) -> Self {
        // Re-base onto the tight bounds so the build stands on the ground and is centred,
        // whatever padding or offset the spec used.
        let (lo, hi) = grid.bounds().unwrap_or((IVec3::ZERO, IVec3::ONE));
        let mut voxels: Vec<(IVec3, [u8; 3], f32)> = grid.iter().map(|(p, c)| (p - lo, c, 0.0)).collect();
        let key = |p: IVec3| -> i64 {
            let layer = p.y as i64 * 1_000_000;
            // Serpentine diagonal sweep: alternate direction per layer to avoid a visible "reset".
            let diag = (p.x + p.z) as i64;
            let diag = if p.y % 2 == 0 { diag } else { -diag };
            layer + diag * 1000 + p.x as i64
        };
        voxels.sort_by_key(|(p, _, _)| key(*p));
        let n = voxels.len().max(1) as f32;
        let span = (duration - DROP_SECS).max(0.1);
        for (i, v) in voxels.iter_mut().enumerate() {
            v.2 = i as f32 / n * span;
        }
        let s = hi - lo;
        let mut start = vec![f32::INFINITY; (s.x * s.y * s.z) as usize];
        for (p, _, t) in &voxels {
            start[((p.y * s.z + p.z) * s.x + p.x) as usize] = *t;
        }
        let mut layer_starts = vec![f32::INFINITY; s.y as usize + 1];
        for (p, _, t) in &voxels {
            let y = p.y as usize;
            layer_starts[y] = layer_starts[y].min(*t);
        }
        Self { size: s, voxels, start, duration, layer_starts }
    }

    #[inline]
    fn landed(&self, p: IVec3, t: f32) -> bool {
        let s = self.size;
        if p.x < 0 || p.y < 0 || p.z < 0 || p.x >= s.x || p.y >= s.y || p.z >= s.z {
            return false;
        }
        self.start[((p.y * s.z + p.z) * s.x + p.x) as usize] + DROP_SECS <= t
    }

    /// Height of the build front at time `t`, interpolated between layer starts so a camera
    /// following it moves smoothly instead of stepping once per layer.
    pub fn height_at(&self, t: f32) -> f32 {
        let n = self.size.y as usize;
        for y in 0..n {
            let (a, b) = (self.layer_starts[y], self.layer_starts.get(y + 1).copied().unwrap_or(f32::INFINITY));
            let b = if b.is_finite() { b } else { a + DROP_SECS };
            if t < b {
                return y as f32 + ((t - a) / (b - a).max(1e-3)).clamp(0.0, 1.0);
            }
        }
        n as f32
    }

    /// Landing times, used to generate the click track.
    pub fn landing_times(&self) -> impl Iterator<Item = f32> + '_ {
        self.voxels.iter().map(|v| v.2 + DROP_SECS)
    }

    /// Instances for build-local time `t` (seconds since this build started), placed at `offset`
    /// in world space (the build's footprint is centred on x/z = 0).
    pub fn instances(&self, t: f32, offset: Vec3, out: &mut Vec<Instance>) {
        let center = Vec3::new(self.size.x as f32 * 0.5, 0.0, self.size.z as f32 * 0.5);
        for &(p, c, s) in &self.voxels {
            if t < s {
                // Voxels are sorted by start time, nothing later is visible yet.
                break;
            }
            let k = ((t - s) / DROP_SECS).min(1.0);
            let landed = k >= 1.0;
            if landed && NEIGHBOURS.iter().all(|d| self.landed(p + *d, t)) {
                continue; // fully buried, never visible
            }
            let ease = 1.0 - (1.0 - k).powi(3);
            let drop = (1.0 - ease) * DROP_HEIGHT;
            let scale = if landed { 1.0 } else { 0.55 + 0.45 * ease };
            let since_land = t - s - DROP_SECS;
            let glow = if since_land >= 0.0 { (1.0 - since_land / GLOW_SECS).max(0.0) * 0.35 } else { 0.0 };
            let pos = p.as_vec3() - center + offset + Vec3::new(0.0, drop, 0.0);
            let ao = if landed { self.ao(p, t) } else { [u32::MAX, u32::MAX >> 16] };
            out.push(Instance::cube(pos, scale, c, glow, ao));
        }
    }

    /// Per-face, per-corner AO packed 2 bits each (3 = fully lit), in the same face/corner order
    /// as `render::cube_vertices`.
    fn ao(&self, p: IVec3, t: f32) -> [u32; 2] {
        let mut bits: u64 = 0;
        for (f, face) in crate::render::FACES.iter().enumerate() {
            let n = face.normal;
            for (ci, corner) in face.corners.iter().enumerate() {
                // Corner in {0,1}^3 -> sign along the two tangent axes.
                let su = face.t1 * (corner.dot(face.t1.as_vec3()) * 2.0 - 1.0).signum() as i32;
                let sv = face.t2 * (corner.dot(face.t2.as_vec3()) * 2.0 - 1.0).signum() as i32;
                let base = p + n;
                let s1 = self.landed(base + su, t);
                let s2 = self.landed(base + sv, t);
                let cc = self.landed(base + su + sv, t);
                let v = if s1 && s2 { 0 } else { 3 - (s1 as u64 + s2 as u64 + cc as u64) };
                bits |= v << ((f * 4 + ci) * 2);
            }
        }
        [bits as u32, (bits >> 32) as u32]
    }
}
