// three.js scene that shows one or more voxel builds (side by side) and replays the
// block-by-block build animation from the videos. Browser-only.
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import type { Build } from './voxels';
import type { Theme } from './themes';

export const DROP_SECS = 0.35;
const DROP_HEIGHT = 4;
const GLOW_SECS = 0.25;
const GAP = 6;
const PLATE_MARGIN = 1.5;
const SUN = new THREE.Vector3(-0.45, 0.85, 0.35).normalize();

interface Slot {
	build: Build;
	mesh: THREE.InstancedMesh;
	offset: THREE.Vector3;
	/** performance.now() when the current replay started, null when idle. */
	t0: number | null;
	started: number;
	landed: number;
	glowFrom: number;
	onDone?: () => void;
}

export interface StageOptions {
	/** Drag/zoom with OrbitControls. Off for gallery cards so touch scrolling keeps working. */
	interactive: boolean;
	autoRotate: boolean;
}

function gradientTexture(top: string, horizon: string) {
	const c = document.createElement('canvas');
	c.width = 2;
	c.height = 256;
	const g = c.getContext('2d')!;
	const grad = g.createLinearGradient(0, 0, 0, 256);
	grad.addColorStop(0, top);
	grad.addColorStop(0.55, horizon);
	grad.addColorStop(1, horizon);
	g.fillStyle = grad;
	g.fillRect(0, 0, 2, 256);
	const t = new THREE.CanvasTexture(c);
	t.colorSpace = THREE.SRGBColorSpace;
	return t;
}

function checkerTexture(a: string, b: string) {
	const c = document.createElement('canvas');
	c.width = c.height = 64;
	const g = c.getContext('2d')!;
	g.fillStyle = a;
	g.fillRect(0, 0, 64, 64);
	g.fillStyle = b;
	g.fillRect(0, 0, 32, 32);
	g.fillRect(32, 32, 32, 32);
	const t = new THREE.CanvasTexture(c);
	t.colorSpace = THREE.SRGBColorSpace;
	t.wrapS = t.wrapT = THREE.RepeatWrapping;
	t.anisotropy = 8;
	return t;
}

function cloudTexture() {
	const c = document.createElement('canvas');
	c.width = 256;
	c.height = 128;
	const g = c.getContext('2d')!;
	for (const [x, y, r] of [
		[80, 80, 40],
		[130, 64, 52],
		[180, 82, 38],
		[110, 90, 34],
		[155, 92, 34]
	]) {
		const grad = g.createRadialGradient(x, y, r * 0.3, x, y, r);
		grad.addColorStop(0, 'rgba(255,255,255,0.95)');
		grad.addColorStop(1, 'rgba(255,255,255,0)');
		g.fillStyle = grad;
		g.beginPath();
		g.arc(x, y, r, 0, Math.PI * 2);
		g.fill();
	}
	const t = new THREE.CanvasTexture(c);
	t.colorSpace = THREE.SRGBColorSpace;
	return t;
}

export class VoxelStage {
	private renderer: THREE.WebGLRenderer;
	private scene = new THREE.Scene();
	private camera = new THREE.PerspectiveCamera(35, 1, 0.5, 3000);
	private controls: OrbitControls | null = null;
	private sun: THREE.DirectionalLight;
	private slots: Slot[] = [];
	private center = new THREE.Vector3();
	private radius = 10;
	private angle = 0.7;
	private raf = 0;
	private last = 0;
	private visible = true;
	private disposables: { dispose(): void }[] = [];
	private observers: (IntersectionObserver | ResizeObserver)[] = [];
	private tmp = new THREE.Matrix4();
	private tmpColor = new THREE.Color();
	private white = new THREE.Color('#ffffff');

	constructor(
		private canvas: HTMLCanvasElement,
		private theme: Theme,
		private opts: StageOptions
	) {
		this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
		this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
		this.renderer.shadowMap.enabled = true;
		this.renderer.shadowMap.type = THREE.PCFShadowMap;

		const bg = gradientTexture(theme.skyTop, theme.skyHorizon);
		this.scene.background = bg;
		this.disposables.push(bg);

		this.scene.add(new THREE.HemisphereLight(theme.night ? '#8890c8' : '#ffffff', theme.ground, theme.night ? 1.6 : 1.9));
		this.sun = new THREE.DirectionalLight(theme.night ? '#c8d0ff' : '#fff1d8', theme.night ? 1.6 : 2.4);
		this.sun.castShadow = true;
		this.sun.shadow.mapSize.set(2048, 2048);
		this.sun.shadow.bias = -0.0004;
		this.sun.shadow.normalBias = 0.03;
		this.scene.add(this.sun, this.sun.target);

		const tex = checkerTexture(theme.ground, theme.ground2);
		tex.repeat.set(250, 250);
		const groundGeo = new THREE.PlaneGeometry(2000, 2000);
		const groundMat = new THREE.MeshLambertMaterial({ map: tex });
		const ground = new THREE.Mesh(groundGeo, groundMat);
		ground.rotation.x = -Math.PI / 2;
		ground.position.y = -0.4;
		ground.receiveShadow = true;
		this.scene.add(ground);
		this.disposables.push(tex, groundGeo, groundMat);

		this.addSkyDecor();

		if (opts.interactive) {
			const c = new OrbitControls(this.camera, canvas);
			c.enableDamping = true;
			c.enablePan = false;
			c.autoRotate = opts.autoRotate;
			c.autoRotateSpeed = 1.4;
			c.maxPolarAngle = Math.PI * 0.47;
			this.controls = c;
		}

		const io = new IntersectionObserver(([e]) => {
			this.visible = e.isIntersecting;
			if (this.visible) this.loop();
		});
		io.observe(canvas);
		const ro = new ResizeObserver(() => this.resize());
		ro.observe(canvas);
		this.observers.push(io, ro);
	}

	/** Far-away clouds (day themes) or stars (night themes) for depth. */
	private addSkyDecor() {
		if (this.theme.night) {
			const pts: number[] = [];
			for (let i = 0; i < 900; i++) {
				const az = Math.random() * Math.PI * 2;
				const el = Math.random() * 0.6 + 0.02;
				pts.push(Math.cos(az) * Math.cos(el) * 900, Math.sin(el) * 900, Math.sin(az) * Math.cos(el) * 900);
			}
			const geo = new THREE.BufferGeometry();
			geo.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3));
			const mat = new THREE.PointsMaterial({ color: '#ffffff', size: 2, sizeAttenuation: false, fog: false });
			this.scene.add(new THREE.Points(geo, mat));
			this.disposables.push(geo, mat);
		} else {
			const tex = cloudTexture();
			const mat = new THREE.SpriteMaterial({ map: tex, fog: false, depthWrite: false, opacity: 0.9 });
			this.disposables.push(tex, mat);
			for (let i = 0; i < 14; i++) {
				const s = new THREE.Sprite(mat);
				const az = (i / 14) * Math.PI * 2 + Math.random() * 0.3;
				const r = 700;
				s.position.set(Math.cos(az) * r, 60 + Math.random() * 110, Math.sin(az) * r);
				const w = 180 + Math.random() * 160;
				s.scale.set(w, w / 2, 1);
				this.scene.add(s);
			}
		}
	}

	setBuilds(builds: Build[]) {
		for (const s of this.slots) {
			this.scene.remove(s.mesh);
			s.mesh.geometry.dispose();
			(s.mesh.material as THREE.Material).dispose();
		}
		this.slots = [];
		const widths = builds.map((b) => Math.max(b.size[0], b.size[2]));
		const total = widths.reduce((a, b) => a + b, 0) + GAP * (builds.length - 1);
		let x = -total / 2;
		const box = new THREE.Box3();
		builds.forEach((build, i) => {
			const offset = new THREE.Vector3(x + widths[i] / 2, 0, 0);
			x += widths[i] + GAP;
			const geo = new THREE.BoxGeometry(1, 1, 1);
			const mat = new THREE.MeshLambertMaterial();
			const mesh = new THREE.InstancedMesh(geo, mat, Math.max(1, build.voxels.length));
			mesh.castShadow = mesh.receiveShadow = true;
			mesh.frustumCulled = false;
			build.voxels.forEach((v, k) => mesh.setColorAt(k, this.tmpColor.setHex(v.color)));
			this.scene.add(mesh);

			const [w, h, d] = build.size;
			const plateGeo = new THREE.BoxGeometry(w + PLATE_MARGIN * 2, 0.4, d + PLATE_MARGIN * 2);
			const plateMat = new THREE.MeshLambertMaterial({ color: this.theme.plate });
			const plate = new THREE.Mesh(plateGeo, plateMat);
			plate.position.set(offset.x, -0.2, offset.z);
			plate.receiveShadow = true;
			this.scene.add(plate);
			this.disposables.push(plateGeo, plateMat);

			box.expandByPoint(new THREE.Vector3(offset.x - w / 2 - PLATE_MARGIN, -0.4, -d / 2 - PLATE_MARGIN));
			box.expandByPoint(new THREE.Vector3(offset.x + w / 2 + PLATE_MARGIN, h, d / 2 + PLATE_MARGIN));
			const slot: Slot = { build, mesh, offset, t0: null, started: 0, landed: 0, glowFrom: 0 };
			this.finish(slot);
			this.slots.push(slot);
		});

		const sphere = box.getBoundingSphere(new THREE.Sphere());
		this.center.copy(sphere.center);
		this.radius = sphere.radius;
		const r = this.radius * 1.1;
		this.sun.position.copy(this.center).addScaledVector(SUN, r * 2);
		this.sun.target.position.copy(this.center);
		const cam = this.sun.shadow.camera;
		cam.left = cam.bottom = -r;
		cam.right = cam.top = r;
		cam.near = 0.1;
		cam.far = r * 4;
		cam.updateProjectionMatrix();
		this.scene.fog = new THREE.Fog(this.theme.skyHorizon, this.radius * 4, this.radius * 14);
		this.resize();
		this.loop();
	}

	private place(slot: Slot, k: number, drop: number, scale: number) {
		const v = slot.build.voxels[k];
		const [w, , d] = slot.build.size;
		this.tmp.makeScale(scale, scale, scale);
		this.tmp.setPosition(v.x - w / 2 + 0.5 + slot.offset.x, v.y + 0.5 + drop, v.z - d / 2 + 0.5 + slot.offset.z);
		slot.mesh.setMatrixAt(k, this.tmp);
	}

	private finish(slot: Slot) {
		const n = slot.build.voxels.length;
		for (let k = 0; k < n; k++) {
			this.place(slot, k, 0, 1);
			slot.mesh.setColorAt(k, this.tmpColor.setHex(slot.build.voxels[k].color));
		}
		slot.mesh.count = n;
		slot.mesh.instanceMatrix.needsUpdate = true;
		if (slot.mesh.instanceColor) slot.mesh.instanceColor.needsUpdate = true;
		slot.t0 = null;
	}

	/** Replays the build animation for one build, or all of them when `index` is omitted. */
	play(index?: number, onDone?: () => void) {
		const now = performance.now();
		this.slots.forEach((s, i) => {
			if (index !== undefined && i !== index) return;
			s.t0 = now;
			s.started = s.landed = s.glowFrom = 0;
			s.mesh.count = 0;
			s.onDone = onDone;
		});
		this.loop();
	}

	private animate(slot: Slot, now: number) {
		if (slot.t0 === null) return;
		const t = (now - slot.t0) / 1000;
		const vs = slot.build.voxels;
		while (slot.started < vs.length && vs[slot.started].start <= t) slot.started++;
		for (let k = slot.landed; k < slot.started; k++) {
			const p = Math.min(1, (t - vs[k].start) / DROP_SECS);
			if (p >= 1) {
				this.place(slot, k, 0, 1);
				if (k === slot.landed) slot.landed++;
			} else {
				const ease = 1 - (1 - p) ** 3;
				this.place(slot, k, (1 - ease) * DROP_HEIGHT, 0.55 + 0.45 * ease);
			}
		}
		// Landed blocks flash briefly, like the videos.
		for (let k = slot.glowFrom; k < slot.landed; k++) {
			const g = 1 - (t - vs[k].start - DROP_SECS) / GLOW_SECS;
			this.tmpColor.setHex(vs[k].color);
			if (g <= 0) {
				if (k === slot.glowFrom) slot.glowFrom++;
			} else {
				this.tmpColor.lerp(this.white, g * 0.45);
			}
			slot.mesh.setColorAt(k, this.tmpColor);
		}
		slot.mesh.count = slot.started;
		slot.mesh.instanceMatrix.needsUpdate = true;
		if (slot.mesh.instanceColor) slot.mesh.instanceColor.needsUpdate = true;
		if (slot.landed >= vs.length && slot.glowFrom >= vs.length) {
			slot.t0 = null;
			slot.onDone?.();
		}
	}

	private fitDistance() {
		const vfov = THREE.MathUtils.degToRad(this.camera.fov);
		const hfov = 2 * Math.atan(Math.tan(vfov / 2) * this.camera.aspect);
		return (this.radius / Math.sin(Math.min(vfov, hfov) / 2)) * 0.82;
	}

	private resize() {
		const w = this.canvas.clientWidth;
		const h = this.canvas.clientHeight;
		if (!w || !h) return;
		this.renderer.setSize(w, h, false);
		this.camera.aspect = w / h;
		this.camera.updateProjectionMatrix();
		const dist = this.fitDistance();
		if (this.controls) {
			this.controls.target.copy(this.center);
			this.controls.minDistance = dist * 0.35;
			this.controls.maxDistance = dist * 1.8;
			if (!this.placed) this.setCamera(dist);
			this.controls.update();
		} else {
			this.setCamera(dist);
		}
		this.render();
	}

	private placed = false;

	private setCamera(dist: number) {
		const el = 0.38;
		this.camera.position.set(
			this.center.x + Math.sin(this.angle) * Math.cos(el) * dist,
			this.center.y + Math.sin(el) * dist,
			this.center.z + Math.cos(this.angle) * Math.cos(el) * dist
		);
		this.camera.lookAt(this.center);
		this.placed = true;
	}

	private render() {
		this.renderer.render(this.scene, this.camera);
	}

	private loop = () => {
		cancelAnimationFrame(this.raf);
		if (!this.visible) return;
		const now = performance.now();
		const dt = this.last ? Math.min(0.1, (now - this.last) / 1000) : 0;
		this.last = now;
		for (const s of this.slots) this.animate(s, now);
		if (this.controls) {
			this.controls.update(dt);
		} else if (this.opts.autoRotate) {
			this.angle += dt * 0.35;
			this.setCamera(this.fitDistance());
		}
		this.render();
		const busy = this.opts.autoRotate || this.slots.some((s) => s.t0 !== null) || !!this.controls;
		if (busy) this.raf = requestAnimationFrame(this.loop);
		else this.last = 0;
	};

	dispose() {
		cancelAnimationFrame(this.raf);
		this.visible = false;
		for (const o of this.observers) o.disconnect();
		this.controls?.dispose();
		for (const s of this.slots) {
			s.mesh.geometry.dispose();
			(s.mesh.material as THREE.Material).dispose();
		}
		for (const d of this.disposables) d.dispose();
		this.renderer.dispose();
		this.renderer.forceContextLoss();
	}
}
