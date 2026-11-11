// Port of voxreel's build DSL (src/voxels.rs) so the website shows exactly what the videos show.
// Coordinates: integer voxels, Y up, ground at y = 0. Ops apply in order; `color: null` carves.

export type Vec3 = [number, number, number];

export type Op =
	| { op: 'box'; from: Vec3; to: Vec3; color?: string | null; hollow?: boolean }
	| { op: 'sphere'; center: Vec3; radius: number; color?: string | null; hollow?: boolean }
	| { op: 'ellipsoid'; center: Vec3; radii: Vec3; color?: string | null; hollow?: boolean }
	| { op: 'cylinder'; base: Vec3; radius: number; height: number; axis?: string; color?: string | null; hollow?: boolean }
	| { op: 'cone'; base: Vec3; radius: number; height: number; axis?: string; color?: string | null }
	| { op: 'line'; from: Vec3; to: Vec3; radius?: number; color?: string | null }
	| { op: 'voxels'; points: Vec3[]; color?: string | null }
	| { op: 'erase'; from: Vec3; to: Vec3 };

export interface BuildSpec {
	title?: string;
	size: Vec3;
	ops: Op[];
}

/** A voxel in build order: position (re-based so the build stands at the origin), colour, drop start. */
export interface Voxel {
	x: number;
	y: number;
	z: number;
	color: number;
	start: number;
}

export interface Build {
	size: Vec3;
	voxels: Voxel[];
}

const MAX_SIZE = 96;
const EMPTY = -1;

function parseColor(c: string | null | undefined): number {
	if (c == null) return EMPTY;
	let h = c.trim().replace(/^#/, '');
	if (h.length === 3) h = [...h].map((ch) => ch + ch).join('');
	const v = parseInt(h, 16);
	return Number.isNaN(v) || h.length !== 6 ? EMPTY : v;
}

class Grid {
	cells: Int32Array;
	constructor(public sx: number, public sy: number, public sz: number) {
		this.cells = new Int32Array(sx * sy * sz).fill(EMPTY);
	}
	set(x: number, y: number, z: number, c: number) {
		if (x < 0 || y < 0 || z < 0 || x >= this.sx || y >= this.sy || z >= this.sz) return;
		this.cells[(y * this.sz + z) * this.sx + x] = c;
	}
	get(x: number, y: number, z: number) {
		return this.cells[(y * this.sz + z) * this.sx + x];
	}
}

type Inside = (p: Vec3) => number | null;

/** Visits voxel centres inside [lo, hi] and sets them when `inside` returns a depth; `hollow`
 * keeps only depth < 1 (the shell). Mirrors `fill` in voxels.rs. */
function fill(g: Grid, lo: Vec3, hi: Vec3, color: number, hollow: boolean, inside: Inside) {
	const l = lo.map((v) => Math.max(0, Math.floor(v)));
	const h = [Math.min(g.sx - 1, Math.ceil(hi[0])), Math.min(g.sy - 1, Math.ceil(hi[1])), Math.min(g.sz - 1, Math.ceil(hi[2]))];
	for (let y = l[1]; y <= h[1]; y++)
		for (let z = l[2]; z <= h[2]; z++)
			for (let x = l[0]; x <= h[0]; x++) {
				const d = inside([x + 0.5, y + 0.5, z + 0.5]);
				if (d !== null && (!hollow || d < 1)) g.set(x, y, z, color);
			}
}

function axisFrame(axis = 'y'): [number, number, number] {
	if (axis === 'x') return [0, 1, 2];
	if (axis === 'z') return [2, 0, 1];
	return [1, 0, 2];
}

const add = (a: Vec3, s: number): Vec3 => [a[0] + s, a[1] + s, a[2] + s];
const dist = (a: Vec3, b: Vec3) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

function boxInside(a: Vec3, b: Vec3): Inside {
	return (p) => {
		const d = Math.min(p[0] - a[0], p[1] - a[1], p[2] - a[2], b[0] - p[0], b[1] - p[1], b[2] - p[2]);
		return d > 0 ? d : null;
	};
}

function apply(g: Grid, op: Op) {
	switch (op.op) {
		case 'box':
		case 'erase': {
			const a = op.from.map((v, i) => Math.min(v, op.to[i])) as Vec3;
			const b = op.from.map((v, i) => Math.max(v, op.to[i]) + 1) as Vec3;
			const color = op.op === 'erase' ? EMPTY : parseColor(op.color);
			fill(g, a, b, color, op.op === 'box' && !!op.hollow, boxInside(a, b));
			break;
		}
		case 'sphere': {
			const c = add(op.center, 0.5);
			const r = Math.max(op.radius, 0.5);
			fill(g, add(c, -r), add(c, r), parseColor(op.color), !!op.hollow, (p) => {
				const d = r - dist(p, c);
				return d >= 0 ? d : null;
			});
			break;
		}
		case 'ellipsoid': {
			const c = add(op.center, 0.5);
			const r = op.radii.map((v) => Math.max(v, 0.5)) as Vec3;
			const lo = c.map((v, i) => v - r[i]) as Vec3;
			const hi = c.map((v, i) => v + r[i]) as Vec3;
			fill(g, lo, hi, parseColor(op.color), !!op.hollow, (p) => {
				const len = Math.hypot((p[0] - c[0]) / r[0], (p[1] - c[1]) / r[1], (p[2] - c[2]) / r[2]);
				return len <= 1 ? (1 - len) * Math.min(...r) : null;
			});
			break;
		}
		case 'cylinder':
		case 'cone': {
			const [ax, u, v] = axisFrame(op.axis);
			const b = add(op.base, 0.5);
			const r = Math.max(op.radius, 0.5);
			const h = op.op === 'cone' && Math.abs(op.height) < 1 ? 1 : op.height;
			const lo = add(b, -r);
			const hi = add(b, r);
			lo[ax] = b[ax] - 0.5 + Math.min(h, 0);
			hi[ax] = b[ax] - 0.5 + Math.max(h, 0);
			const [h0, h1] = [lo[ax], hi[ax]];
			const radial = (p: Vec3) => Math.hypot(p[u] - b[u], p[v] - b[v]);
			if (op.op === 'cylinder') {
				fill(g, lo, hi, parseColor(op.color), !!op.hollow, (p) => {
					const dr = r - radial(p);
					const dh = Math.min(p[ax] - h0, h1 - p[ax]);
					return dr >= 0 && dh > 0 ? Math.min(dr, dh) : null;
				});
			} else {
				const start = b[ax] - 0.5;
				fill(g, lo, hi, parseColor(op.color), false, (p) => {
					const t = (p[ax] - start) / h;
					if (t < 0 || t > 1) return null;
					return radial(p) <= r * (1 - t) + 0.35 ? 1 : null;
				});
			}
			break;
		}
		case 'line': {
			const a = add(op.from, 0.5);
			const b = add(op.to, 0.5);
			const r = Math.max(op.radius ?? 0.5, 0.5);
			const lo = a.map((x, i) => Math.min(x, b[i]) - r) as Vec3;
			const hi = a.map((x, i) => Math.max(x, b[i]) + r) as Vec3;
			const ab: Vec3 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
			const len2 = ab[0] ** 2 + ab[1] ** 2 + ab[2] ** 2;
			fill(g, lo, hi, parseColor(op.color), false, (p) => {
				const t = len2 > 0 ? Math.min(1, Math.max(0, ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2]) / len2)) : 0;
				return dist(p, [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t]) <= r ? 1 : null;
			});
			break;
		}
		case 'voxels': {
			const c = parseColor(op.color);
			for (const [x, y, z] of op.points) g.set(x, y, z, c);
			break;
		}
	}
}

/** Voxelizes a spec and orders it the way the videos build it: bottom-up layers with a
 * serpentine diagonal sweep inside each layer. `duration` spreads drop starts over seconds. */
export function buildFromSpec(spec: BuildSpec, duration: number, dropSecs: number): Build {
	const s = spec.size.map((v) => Math.min(MAX_SIZE, Math.max(1, Math.round(v))));
	const g = new Grid(s[0], s[1], s[2]);
	for (const op of spec.ops) apply(g, op);

	const raw: Voxel[] = [];
	let lo: Vec3 = [Infinity, Infinity, Infinity];
	let hi: Vec3 = [-Infinity, -Infinity, -Infinity];
	for (let y = 0; y < g.sy; y++)
		for (let z = 0; z < g.sz; z++)
			for (let x = 0; x < g.sx; x++) {
				const c = g.get(x, y, z);
				if (c === EMPTY) continue;
				raw.push({ x, y, z, color: c, start: 0 });
				lo = [Math.min(lo[0], x), Math.min(lo[1], y), Math.min(lo[2], z)];
				hi = [Math.max(hi[0], x + 1), Math.max(hi[1], y + 1), Math.max(hi[2], z + 1)];
			}
	if (!raw.length) return { size: [1, 1, 1], voxels: [] };
	for (const v of raw) {
		v.x -= lo[0];
		v.y -= lo[1];
		v.z -= lo[2];
	}
	const key = (v: Voxel) => {
		const diag = v.y % 2 === 0 ? v.x + v.z : -(v.x + v.z);
		return v.y * 1e6 + diag * 1000 + v.x;
	};
	raw.sort((a, b) => key(a) - key(b));
	const span = Math.max(0.1, duration - dropSecs);
	raw.forEach((v, i) => (v.start = (i / raw.length) * span));
	return { size: [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]], voxels: raw };
}
