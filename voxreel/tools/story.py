"""Scripted "how it's made" stories: characters walk between stations, carry things and work,
props change place and hands, a camera operator follows them. Plays with `voxreel story`.

    uv run --no-project tools/story.py pizza
    cargo run --release -- story story/pizza/script.json            # -> out/story/pizza.mp4
    cargo run --release -- story story/pizza/script.json --still 12 --height 960

Units: everything is in voxels (y up, z towards the default viewer), except the world, which is
built in blocks of B voxels (1 block = character height / 2, like Minecraft). Block layer 0 sits
flush under the grass (paths, plaza, farmland), layer 1 stands on it.
"""
from __future__ import annotations

import json
import math
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from vox import Canvas  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent / "story"
B = 9.6  # voxels per block (16 skin pixels of 0.6)


# =============================================================== script DSL
BODY = 3.5  # clearance around a character (half its width plus a little air)


def seg_hits_rect(a, b, r, shrink=0.3):
    """Does segment a-b cross rectangle r = (x0, z0, x1, z1) (shrunk a bit so grazing a corner is ok)?"""
    x0, z0, x1, z1 = r[0] + shrink, r[1] + shrink, r[2] - shrink, r[3] - shrink
    (ax, az), (bx, bz) = a, b
    t0, t1 = 0.0, 1.0
    for p, q in ((-(bx - ax), ax - x0), (bx - ax, x1 - ax), (-(bz - az), az - z0), (bz - az, z1 - az)):
        if abs(p) < 1e-9:
            if q < 0:
                return False
        else:
            t = q / p
            if p < 0:
                t0 = max(t0, t)
            else:
                t1 = min(t1, t)
            if t0 > t1:
                return False
    return True


def route(a, b, rects):
    """Shortest waypoints from a to b around the rectangles: Dijkstra over a visibility graph of
    the rectangle corners (a leg is allowed when it crosses no rectangle)."""
    import heapq

    a, b = tuple(a), tuple(b)
    clear = lambda p, q: not any(seg_hits_rect(p, q, r) for r in rects)
    if clear(a, b):
        return [b]
    inside = lambda c: any(r[0] < c[0] < r[2] and r[1] < c[1] < r[3] for r in rects)
    nodes = [a, b] + [c for (x0, z0, x1, z1) in rects for c in ((x0, z0), (x1, z0), (x1, z1), (x0, z1)) if not inside(c)]
    dist = {a: 0.0}
    prev = {}
    heap = [(0.0, a)]
    while heap:
        d, n = heapq.heappop(heap)
        if n == b:
            break
        if d > dist.get(n, math.inf):
            continue
        for m in nodes:
            if m != n and clear(n, m):
                nd = d + math.dist(n, m)
                if nd < dist.get(m, math.inf):
                    dist[m], prev[m] = nd, n
                    heapq.heappush(heap, (nd, m))
    if b not in prev:
        return [b]  # no way around: walk straight (the check will report it)
    path = [b]
    while path[-1] != a:
        path.append(prev[path[-1]])
    return path[::-1][1:]


class Actor:
    def __init__(self, story, idx, skin, x, z, outfit=None, hat=None, speed=10.0):
        self.story, self.idx, self.speed = story, idx, speed
        self.d = {"skin": skin, "keys": [[0.0, x, 0.0, z]], "look": [], "acts": [], "items": []}
        if outfit:
            self.d["outfit"] = outfit
        if hat:
            self.d["hat"] = hat
        self.clock = 0.0

    @property
    def pos(self):
        k = self.d["keys"][-1]
        return k[1], k[3]

    def _key(self, t, x, z):
        self.d["keys"].append([round(t, 3), x, 0.0, z])

    def until(self, t):
        """Stand still until time t."""
        if t > self.clock:
            x, z = self.pos
            self._key(t, x, z)
            self.clock = t
        return self

    def wait(self, secs):
        return self.until(self.clock + secs)

    def walk(self, *points, speed=None, avoid=True):
        """Walk through the points; with `avoid`, each leg goes around the story's obstacles."""
        sp = speed or self.speed
        rects = [(x0 - BODY, z0 - BODY, x1 + BODY, z1 + BODY) for (x0, z0, x1, z1, _) in self.story.obstacles] if avoid else []
        for target in points:
            for (x, z) in route(self.pos, tuple(target), rects):
                px, pz = self.pos
                self.clock += math.hypot(x - px, z - pz) / sp
                self._key(self.clock, x, z)
        return self

    def face(self, x, z):
        self.d["look"].append([round(self.clock, 3), x, z])
        return self

    def act(self, action, secs, item=None, face=None):
        if face:
            self.face(*face)
        t0 = self.clock
        self.d["acts"].append([round(t0, 3), round(t0 + secs, 3), action])
        if item:
            self.d["items"].append([round(t0, 3), round(t0 + secs, 3), item])
        return self.wait(secs)

    def position_at(self, t):
        keys = self.d["keys"]
        if t <= keys[0][0]:
            return keys[0][1], keys[0][3]
        for a, b in zip(keys, keys[1:]):
            if t < b[0]:
                f = (t - a[0]) / max(b[0] - a[0], 1e-4)
                return a[1] + (b[1] - a[1]) * f, a[3] + (b[3] - a[3]) * f
        return keys[-1][1], keys[-1][3]


class Prop:
    def __init__(self, story, name, canvas):
        self.name, self.canvas, self.keys = name, canvas, []
        story.props.append(self)

    def show(self, t, x, y, z, yaw=0):
        self.keys.append([round(t, 3), "world", x, y, z, yaw])
        return self

    def build(self, t, x, y, z, secs=1.0):
        self.keys.append([round(t, 3), "build", x, y, z, secs])
        return self

    def hand(self, t, actor):
        self.keys.append([round(t, 3), "hand", actor.idx])
        return self

    def hide(self, t):
        self.keys.append([round(t, 3), "hide"])
        return self


class Camera:
    """Camera operator: shots are sampled every 0.1 s; the first key of a shot is a cut.

    Each sample checks whether a character (other than the shot's subjects) or an obstacle
    stands between the camera and the subject; if so it swings sideways (smallest angle that
    clears the view), and the swing is smoothed so the move looks deliberate."""

    def __init__(self, story):
        self.story = story
        self.keys = []

    def _solid_at(self, p):
        world = self.story.world.v if self.story.world else {}
        if (math.floor(p[0] / B), math.floor(p[1] / B) + 1, math.floor(p[2] / B)) in world:
            return True
        return any(x0 <= p[0] <= x1 and z0 <= p[2] <= z1 and p[1] < h for (x0, z0, x1, z1, h) in self.story.obstacles)

    def _near_clutter(self, eye, tgt):
        """Big things right in front of the lens: probe the frame edges for the first ~25 voxels."""
        f = [tgt[i] - eye[i] for i in range(3)]
        n = math.sqrt(sum(v * v for v in f))
        f = [v / n for v in f]
        r = [f[2], 0.0, -f[0]]
        rn = math.hypot(r[0], r[2]) or 1.0
        r = [v / rn for v in r]
        u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]]
        dirs = []
        for (a, axis) in ((0.45, u), (-0.45, u), (0.27, r), (-0.27, r)):
            ca, sa = math.cos(a), math.sin(a)
            dirs.append([f[i] * ca + axis[i] * sa for i in range(3)])
        for dvec in dirs:
            for k in range(1, 14):
                dist = k * 2.0
                if self._solid_at([eye[i] + dvec[i] * dist for i in range(3)]):
                    return True
        return False

    def _blocked(self, eye, tgt, t, subjects):
        world = self.story.world.v if self.story.world else {}
        d = math.dist(eye, tgt)
        if self._near_clutter(eye, tgt):
            return True
        for k in range(0, 19):
            f = k / 20
            p = [eye[i] + (tgt[i] - eye[i]) * f for i in range(3)]
            # world blocks along the line (and the eye itself)
            if (math.floor(p[0] / B), math.floor(p[1] / B) + 1, math.floor(p[2] / B)) in world:
                return True
            if k < 2:
                continue
            # the frame widens with distance from the eye (narrow 9:16 lens: ~16 deg each side)
            reach = 3.5 + f * d * 0.28
            for a in self.story.actors:
                ax, az = a.position_at(t)
                # anyone right at the focus point is the action itself; anyone else in view blocks it
                if math.hypot(tgt[0] - ax, tgt[2] - az) < 6 or (a in subjects and math.hypot(tgt[0] - ax, tgt[2] - az) < 9):
                    continue
                if math.hypot(p[0] - ax, p[2] - az) < reach and p[1] < 21 + f * d * 0.4:
                    return True
            for (x0, z0, x1, z1, h) in self.story.obstacles:
                if x0 - 1 <= p[0] <= x1 + 1 and z0 - 1 <= p[2] <= z1 + 1 and p[1] < h:
                    return True
        return False

    def shot(self, t0, t1, target, dist, elev_deg, yaw0_deg, yaw1_deg=None, height=11.0, dist1=None, smooth=0.25, subjects=()):
        yaw1_deg = yaw0_deg if yaw1_deg is None else yaw1_deg
        dist1 = dist if dist1 is None else dist1
        n = max(2, int((t1 - t0) / 0.1) + 1)
        sx = sz = None
        samples = []
        for i in range(n):
            t = t0 + (t1 - t0) * i / (n - 1)
            x, z = target(t) if callable(target) else target
            # follow with a little lag so the camera feels operated, not glued
            if sx is None:
                sx, sz = x, z
            else:
                sx += (x - sx) * smooth
                sz += (z - sz) * smooth
            f = i / (n - 1)
            f = f * f * (3 - 2 * f)
            samples.append((t, sx, sz, yaw0_deg + (yaw1_deg - yaw0_deg) * f, dist + (dist1 - dist) * f))
        e = math.radians(elev_deg)

        def eye_of(sx, sz, yaw_deg, d):
            y = math.radians(yaw_deg)
            return [sx + math.sin(y) * math.cos(e) * d, height + math.sin(e) * d, sz + math.cos(y) * math.cos(e) * d]

        # smallest sideways swing that clears the view, per sample
        swing = []
        for (t, sx, sz, yaw, d) in samples:
            best = 0.0
            for off in (0, 12, -12, 24, -24, 36, -36, 50, -50):
                if not self._blocked(eye_of(sx, sz, yaw + off, d), [sx, height, sz], t, subjects):
                    best = off
                    break
            swing.append(best)
        # smooth it (moving average) so the camera glides around the obstacle; smoothing can pull a
        # frame back behind the obstacle, so re-check and pin those frames, with a shrinking window
        smooth_swing = swing
        for w in (5, 3, 1, 0):
            smooth_swing = [sum(swing[max(0, i - w): i + w + 1]) / len(swing[max(0, i - w): i + w + 1]) for i in range(len(swing))]
            bad = [i for i, (t, sx, sz, yaw, d) in enumerate(samples)
                   if self._blocked(eye_of(sx, sz, yaw + smooth_swing[i], d), [sx, height, sz], t, subjects)]
            if not bad:
                break
            for i in bad:
                swing[i] = swing[i] if swing[i] else 0.0
        for i, (t, sx, sz, yaw, d) in enumerate(samples):
            eye = eye_of(sx, sz, yaw + smooth_swing[i], d)
            self.keys.append([round(t, 3), *[round(v, 2) for v in eye], round(sx, 2), height, round(sz, 2), 1 if i == 0 else 0])


class Story:
    def __init__(self, slug, title, hook, theme="day", ambience=("birds", "wind")):
        self.slug, self.title, self.hook, self.theme, self.ambience = slug, title, hook, theme, list(ambience)
        self.actors, self.props, self.captions, self.sfx = [], [], [], []
        # (x0, z0, x1, z1, height) in voxels: characters walk around these, the camera avoids them
        self.obstacles = []
        self.cam = Camera(self)
        self.world = None
        self.duration = 0.0
        self.hook_until = 0.0
        self.hook_scene = 0.0

    def actor(self, skin, x, z, **kw):
        a = Actor(self, len(self.actors), skin, x, z, **kw)
        self.actors.append(a)
        return a

    def prop(self, name, canvas):
        return Prop(self, name, canvas)

    def solid(self, x0, z0, x1, z1, h):
        self.obstacles.append((min(x0, x1), min(z0, z1), max(x0, x1), max(z0, z1), h))

    def check(self):
        """Print moments where two characters overlap or one stands inside an obstacle."""
        problems = []
        t = 0.0
        while t <= self.duration:
            for i, a in enumerate(self.actors):
                ax, az = a.position_at(t)
                for b in self.actors[i + 1:]:
                    bx, bz = b.position_at(t)
                    if math.hypot(ax - bx, az - bz) < 2 * BODY - 1:
                        problems.append(f"{t:5.2f}s: {a.d['skin']} and {b.d['skin']} overlap at ({ax:.0f}, {az:.0f})")
                for (x0, z0, x1, z1, _) in self.obstacles:
                    if x0 + 0.5 < ax < x1 - 0.5 and z0 + 0.5 < az < z1 - 0.5:
                        problems.append(f"{t:5.2f}s: {a.d['skin']} inside obstacle ({x0:.0f},{z0:.0f})-({x1:.0f},{z1:.0f})")
            t += 0.05
        # one line per problem kind and second
        seen = set()
        for p in problems:
            key = (p[:4], p[8:])
            if key not in seen:
                seen.add(key)
                print("  !", p)
        return not problems

    def step(self, t0, t1, label, text):
        self.captions.append([round(t0, 3), round(t1, 3), label.upper(), text.upper()])
        self.sfx.append([round(t0, 3), "bloop"])
        if t0 > 0.5:
            self.sfx.append([round(t0 - 0.25, 3), "whoosh"])

    def write(self):
        out = ROOT / self.slug
        (out / "props").mkdir(parents=True, exist_ok=True)
        self.world.dump(out / "world.json")
        models = {}
        for p in self.props:
            if p.name not in models:
                p.canvas.dump(out / "props" / f"{p.name}.json")
                models[p.name] = True
        for a in self.actors:
            a.until(self.duration)
        script = {
            "title": self.title.upper(),
            "hook": self.hook.upper(),
            "theme": self.theme,
            "ambience": self.ambience,
            "duration": round(self.duration, 3),
            "hook_until": self.hook_until,
            "hook_scene": self.hook_scene,
            "world": "world.json",
            "block": B,
            "fov": 55,
            "actors": [a.d for a in self.actors],
            "props": [{"model": f"props/{p.name}.json", "keys": sorted(p.keys, key=lambda k: k[0])} for p in self.props],
            "camera": self.cam.keys,
            "captions": self.captions,
            "sfx": sorted(self.sfx),
        }
        (out / "script.json").write_text(json.dumps(script, separators=(",", ":")))
        print(f"wrote {out / 'script.json'} ({self.duration:.1f}s)")


# =============================================================== world (blocks)
COBBLE, COBBLE_D, MOSSY = "#8a8a8a", "#747474", "#6f8a5a"
PLANK, PLANK_D, LOG, LOG_D = "#b08a52", "#8f6d3d", "#6b5133", "#4a3520"
GLASS, ROOF, ROOF_D, BRICK = "#c4e6f3", "#9b4a34", "#7a3628", "#a5503a"
LEAVES, LEAVES_D, BIRCH, BIRCH_L = "#3f8f30", "#2f7526", "#e3ddcc", "#6aae3c"
PATH, FARM, WATER, SAND, HAY = "#b39a63", "#5b3c22", "#3a73d8", "#dccf9a", "#d9b23d"
WHITE, STONE = "#e9e5da", "#9c9c9c"


def world_house(w: Canvas, x0, z0, wx=7, wz=6, floors=2, rng=None, door="z"):
    """Two-storey Minecraft-style house: cobble footing, plank walls, log corners, glass, gable roof."""
    h = 1 + floors * 3
    w.box((x0, 1, z0), (x0 + wx - 1, 1, z0 + wz - 1), COBBLE)
    w.box((x0, 2, z0), (x0 + wx - 1, h, z0 + wz - 1), PLANK, hollow=True)
    for (x, z) in [(x0, z0), (x0 + wx - 1, z0), (x0, z0 + wz - 1), (x0 + wx - 1, z0 + wz - 1)]:
        w.box((x, 1, z), (x, h, z), LOG)
    for f in range(floors):
        y = 3 + f * 3
        w.box((x0, y + 2, z0), (x0 + wx - 1, y + 2, z0 + wz - 1), LOG, hollow=True) if f < floors - 1 else None
        for x in range(x0 + 2, x0 + wx - 2, 2):
            w.set((x, y, z0), GLASS)
            w.set((x, y, z0 + wz - 1), GLASS)
        for z in range(z0 + 2, z0 + wz - 2, 2):
            w.set((x0, y, z), GLASS)
            w.set((x0 + wx - 1, y, z), GLASS)
    mid = x0 + wx // 2
    dz = z0 + wz - 1 if door == "z" else z0
    w.box((mid, 2, dz), (mid, 3, dz), LOG_D)
    w.set((mid, 4, dz), PLANK_D)
    k = 0
    while wz - 2 * k > 0:
        w.box((x0 - 1, h + 1 + k, z0 - 1 + k), (x0 + wx, h + 1 + k, z0 + wz - k), ROOF if k % 2 else ROOF_D)
        if wz - 2 * k > 2:
            w.box((x0, h + 1 + k, z0 + k), (x0, h + 1 + k, z0 + wz - 1 - k), PLANK)
            w.box((x0 + wx - 1, h + 1 + k, z0 + k), (x0 + wx - 1, h + 1 + k, z0 + wz - 1 - k), PLANK)
        k += 1
    w.box((x0 + wx - 2, h + 1, z0 + 1), (x0 + wx - 2, h + k + 1, z0 + 1), COBBLE_D)


def world_oak(w: Canvas, rng, x, z):
    h = rng.randint(4, 6)
    w.box((x, 1, z), (x, h, z), LOG)
    for dy, r in [(-1, 2), (0, 2), (1, 1), (2, 1)]:
        for dx in range(-r, r + 1):
            for dz in range(-r, r + 1):
                if abs(dx) == r and abs(dz) == r and (r == 1 or rng.random() < 0.6):
                    continue
                if dx == 0 and dz == 0 and dy <= 0:
                    continue
                w.set((x + dx, h + dy, z + dz), LEAVES_D if rng.random() < 0.3 else LEAVES)


def world_birch(w: Canvas, rng, x, z):
    h = rng.randint(5, 7)
    w.box((x, 1, z), (x, h, z), BIRCH)
    for dy, r in [(-1, 2), (0, 2), (1, 1), (2, 0)]:
        for dx in range(-r, r + 1):
            for dz in range(-r, r + 1):
                if abs(dx) == r and abs(dz) == r and r == 2 and rng.random() < 0.7:
                    continue
                if dx == 0 and dz == 0 and dy <= 0:
                    continue
                w.set((x + dx, h + dy, z + dz), BIRCH_L)


def world_well(w: Canvas, x, z):
    w.box((x - 1, 1, z - 1), (x + 1, 1, z + 1), COBBLE)
    w.set((x, 1, z), WATER)
    for dx, dz in [(-1, -1), (1, -1), (-1, 1), (1, 1)]:
        w.box((x + dx, 2, z + dz), (x + dx, 3, z + dz), PLANK_D)
    w.box((x - 1, 4, z - 1), (x + 1, 4, z + 1), ROOF_D)
    w.set((x, 5, z), ROOF)


def village_world(seed=1) -> Canvas:
    """44 x 44 blocks; plaza in the middle (blocks 15..28), houses and trees around it."""
    w = Canvas("Pizza village", seed=seed, keep_origin=True)
    rng = random.Random(seed)
    N, C0, C1 = 44, 15, 28
    for x in range(C0, C1 + 1):
        for z in range(C0, C1 + 1):
            w.set((x, 0, z), COBBLE_D if rng.random() < 0.3 else (MOSSY if rng.random() < 0.06 else COBBLE))
    for i in range(0, N):  # paths out of the plaza
        for j in (21, 22):
            for (x, z) in [(i, j), (j, i)]:
                if (x, 0, z) not in w.v:
                    w.set((x, 0, z), PATH)
    # garden beds (farmland) on the west side of the plaza, the props stand on it
    for x in range(C0 + 1, C0 + 5):
        for z in range(17, 19):
            w.set((x, 0, z), FARM)
    # hedges around the plaza with gaps for the paths
    for i in range(C0 - 1, C1 + 2):
        for (x, z) in [(i, C0 - 1), (i, C1 + 1), (C0 - 1, i), (C1 + 1, i)]:
            if not (20 <= i <= 23):
                w.set((x, 1, z), LEAVES_D)
    world_well(w, 17, 27)  # quiet south-west corner, off the camera's paths
    # houses around the outside
    for (x0, z0, door) in [(4, 4, "z"), (14, 3, "z"), (31, 4, "z"), (4, 32, "x"), (15, 34, "x"), (33, 33, "x"), (3, 17, "z"), (35, 15, "z")]:
        world_house(w, x0, z0, wx=rng.choice([7, 8]), wz=6, floors=rng.choice([1, 2, 2]), rng=rng, door=door)
    # trees in the gaps
    spots = [(11, 11), (32, 11), (11, 31), (31, 30), (2, 26), (41, 25), (24, 2), (25, 41), (9, 40), (40, 6), (39, 40), (1, 2), (20, 0), (0, 13)]
    for (x, z) in spots:
        (world_oak if rng.random() < 0.65 else world_birch)(w, rng, x, z)
    # hay bales by the farm side
    for (x, z) in [(12, 26), (12, 27), (13, 26)]:
        w.set((x, 1, z), HAY)
    w.set((12, 2, 26), HAY)
    return w


# =============================================================== props (voxels)
RED, RED_D, YELLOW, CREAM, CRUST = "#e5393a", "#a8262a", "#ffd23f", "#f3e3c3", "#e2b36e"
BAKED, BAKED_D, GREEN, GREEN_D, BASIL = "#d48f3f", "#a8652a", "#3f9a3a", "#2f7d2e", "#5fbf4f"
WOODP, WOODP_L, WOODP_D, GREY, GREY_D, DARK = "#9a6338", "#c08552", "#6e4326", "#b9bcc4", "#80858f", "#3a3033"
SACK, SACK_D, ORANGE = "#e3c88f", "#c9a970", "#ff8a2a"


def table_prop():
    c = Canvas("table")
    c.box((0, 9, 0), (29, 9, 13), WOODP_L)
    c.box((0, 8, 0), (29, 8, 13), WOODP)
    for x, z in [(1, 1), (27, 1), (1, 11), (27, 11)]:
        c.box((x, 0, z), (x + 1, 7, z + 1), WOODP_D)
    return c


def bowl_prop():
    c = Canvas("bowl")
    c.cyl((5, 0, 5), 3.0, 4, "#5aa9dc", hollow=True, r_end=4.6)
    c.cyl((5, 0, 5), 3.0, 1, "#5aa9dc")
    return c


def dough_ball_prop():
    c = Canvas("dough_ball")
    c.ellipsoid((3, 1.6, 3), (3.2, 1.8, 3.2), CREAM)
    return c


def board_prop():
    c = Canvas("board")
    c.cyl((7, 0, 7), 7.5, 1, WOODP)
    return c


def pizza_layers(r=6.2):
    base, sauce, top = Canvas("pizza_base"), Canvas("pizza_sauce", seed=4), Canvas("pizza_top", seed=5)
    cx = cz = 7
    base.cyl((cx, 0, cz), r, 1, CREAM)
    base.cyl((cx, 1, cz), r, 1, CRUST, hollow=True)
    sauce.cyl((cx, 1, cz), r - 1, 1, RED)
    rng = top.rng
    for x in range(int(cx - r), int(cx + r) + 1):
        for z in range(int(cz - r), int(cz + r) + 1):
            if math.hypot(x - cx, z - cz) <= r - 1.5 and rng.random() < 0.55:
                top.set((x, 2, z), YELLOW)
    for a in range(0, 360, 72):
        for rr in (r * 0.3, r * 0.65):
            top.cyl((cx + math.cos(math.radians(a + rr * 20)) * rr, 2, cz + math.sin(math.radians(a + rr * 20)) * rr), 1.2, 1, RED_D)
    for a in (30, 150, 270):
        top.set((cx + math.cos(math.radians(a)) * r * 0.5, 3, cz + math.sin(math.radians(a)) * r * 0.5), BASIL)
    return base, sauce, top


def merged(name, *parts, recolor=None):
    c = Canvas(name)
    for p in parts:
        c.place(p)
    if recolor:
        c.paint(recolor)
    return c


def tomato_plant_prop(with_fruit=True, seed=1):
    c = Canvas("tomato_plant" if with_fruit else "tomato_plant_picked", seed=seed)
    c.line((5, 0, 4), (5, 12, 4), 0.5, WOODP_L)
    c.sphere((4, 4, 4), 3.0, GREEN)
    c.sphere((4.5, 8, 4), 2.6, GREEN_D)
    c.sphere((4, 11, 4.5), 1.7, GREEN)
    if with_fruit:
        for dx, dy, dz in [(2.6, 4, 1.2), (-2.3, 6, 1.6), (0.6, 8.5, 2.5), (-1.2, 3, -2.7), (2.1, 9.5, -1.4)]:
            c.sphere((4 + dx, dy, 4 + dz), 1.1, RED)
    return c


def basket_prop(full):
    c = Canvas("basket_full" if full else "basket_empty", seed=7)
    c.box((0, 0, 0), (6, 3, 4), WOODP_L, hollow=True)
    c.erase((1, 1, 1), (5, 3, 3))
    c.paint(lambda x, y, z, col: WOODP if y % 2 == 0 else None)
    if full:
        c.box((1, 1, 1), (5, 3, 3), RED)
        for p in [(2, 4, 2), (4, 4, 2), (3, 4, 1), (3, 4, 3)]:
            c.set(p, RED)
        c.set((3, 5, 2), GREEN)
    return c


def sack_prop():
    c = Canvas("sack")
    c.ellipsoid((3, 3.5, 2.5), (3, 3.6, 2.4), SACK)
    c.box((2, 7, 2), (4, 7, 3), SACK_D)
    c.box((1, 3, 4), (5, 4, 4), "#8fbce6")
    return c


def cart_prop():
    c = Canvas("flour_cart")
    c.box((0, 4, 0), (19, 5, 11), WOODP)
    c.box((0, 6, 0), (19, 7, 0), WOODP_L), c.box((0, 6, 11), (19, 7, 11), WOODP_L)
    for x in (3, 16):
        c.cyl((x, 3, -1), 3, 1, WOODP_D, axis="z")
        c.cyl((x, 3, 12), 3, 1, WOODP_D, axis="z", hollow=False)
    c.line((19, 5, 6), (27, 2, 6), 0.6, WOODP_D)
    for (x, z) in [(3, 3), (9, 3), (3, 8), (9, 8), (15, 5)]:
        c.ellipsoid((x + 2, 9, z + 1), (2.8, 3.2, 2.2), SACK)
        c.box((x + 1, 12, z + 1), (x + 3, 12, z + 1), SACK_D)
    return c


def oven_prop():
    c = Canvas("oven")
    BR, BR_D, STONE_L = "#c4553a", "#9a3f2b", "#e6dccb"
    c.box((0, 0, 0), (27, 9, 27), BR_D)
    c.sphere((13.5, 10, 13.5), 12.5, "#c4553a", where=lambda x, y, z: y >= 10)
    c.paint(lambda x, y, z, col: "#b44a33" if col == BR and (y % 3 == 0 or (x + y // 3 * 2) % 5 == 0) else None)
    # wide opening on the -x side (facing the table), deep and dark inside
    c.box((0, 10, 4), (3, 20, 23), STONE_L)
    c.erase((0, 10, 6), (17, 17, 21))
    c.box((18, 10, 6), (18, 17, 21), DARK)
    c.box((0, 9, 6), (18, 9, 21), "#5a4a44")
    c.box((11, 20, 11), (16, 34, 16), BR_D)  # chimney
    for x in (14, 16):
        c.box((x, 10, 11), (x, 10, 16), WOODP_D)
    # firewood pile
    for i, (x, y) in enumerate([(24, 1), (26, 1), (25, 3)]):
        c.cyl((x + 4, y, 21), 1.2, 6, WOODP_D, axis="z")
    return c


def fire_prop(phase):
    c = Canvas(f"fire_{phase}")
    h = 6 if phase == 0 else 5
    c.cone((3, 0, 3.5), 2.6, h, ORANGE)
    c.cone((3, 0, 3.5), 1.4, h - 2 if phase == 0 else h, YELLOW)
    c.set((3 + (1 if phase else -1), h - 1, 3), ORANGE)
    return c


def smoke_prop():
    c = Canvas("smoke")
    c.sphere((2, 2, 2), 2.2, "#eef2f7")
    return c


def flowers_prop(seed):
    c = Canvas("flowers", seed=seed)
    rng = c.rng
    cols = ["#ff5a5a", "#ffd23f", "#ffffff", "#b58cff", "#ff8a2a"]
    for _ in range(9):
        x, z = rng.randint(0, 12), rng.randint(0, 12)
        h = rng.randint(1, 3)
        c.box((x, 0, z), (x, h, z), GREEN)
        col = rng.choice(cols)
        for dx, dz in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]:
            c.set((x + dx, h + 1, z + dz), col if (dx or dz) else YELLOW)
    return c


# =============================================================== the pizza story
def pizza():
    s = Story("pizza", "Pizza", "How is pizza made?")
    s.world = village_world()
    v = lambda b: b * B  # block -> voxel
    CX, CZ = v(21.5), v(21.5)  # plaza centre (voxels)
    TABLE = (CX, CZ)  # table centre
    TY = 10  # table top height
    GARDEN_Z = v(18)  # the tomato row runs along x on the farmland
    OVEN = (CX + 52, CZ)
    CART = (CX, CZ - 50)

    # static props
    s.prop("table", table_prop()).show(0, TABLE[0], 0, TABLE[1])
    s.prop("oven", oven_prop()).show(0, OVEN[0] + 14, 0, OVEN[1])
    s.prop("flour_cart", cart_prop()).show(0, CART[0], 0, CART[1], yaw=90)
    s.solid(TABLE[0] - 15, TABLE[1] - 7, TABLE[0] + 15, TABLE[1] + 7, 10)
    s.solid(OVEN[0], OVEN[1] - 14, OVEN[0] + 28, OVEN[1] + 14, 35)
    s.solid(CART[0] - 7, CART[1] - 14, CART[0] + 7, CART[1] + 14, 15)
    s.solid(v(16), v(26), v(19), v(29), 48)  # well (world blocks)
    for i, (x, z) in enumerate([(v(18), v(17)), (v(26), v(18)), (v(17.5), v(26.5)), (v(25), v(26))]):
        s.prop(f"flowers{i}", flowers_prop(i)).show(0, x, 0, z)
    bowl = s.prop("bowl", bowl_prop()).show(0, TABLE[0] - 8, TY, TABLE[1])
    board = s.prop("board", board_prop()).show(0, TABLE[0] + 7, TY, TABLE[1])

    # tomato plants on the farmland: (full, picked)
    plants = []
    for i, x in enumerate([v(16.6), v(17.9), v(19.2)]):
        full = s.prop("tomato_plant", tomato_plant_prop(True, i))
        picked = s.prop("tomato_plant_picked", tomato_plant_prop(False, i))
        full.show(0, x, 0, GARDEN_Z)
        plants.append((full, picked, x))
    s.solid(v(16.6) - 4, GARDEN_Z - 3.5, v(19.2) + 4, GARDEN_Z + 3.5, 12)

    alex = s.actor("alex", v(15.5), GARDEN_Z - 12, outfit="farmer", hat="straw", speed=11)
    steve = s.actor("steve", CART[0] + 18, CART[1] + 6, outfit="chef", hat="chef", speed=11)

    s.hook_until = 1.6
    T = s.hook_until

    # ---- STEP 1: pick the tomatoes
    basket_e = s.prop("basket_empty", basket_prop(False))
    basket_f = s.prop("basket_full", basket_prop(True))
    basket_e.hand(0, alex)
    alex.until(T)
    # Alex works from behind the row, facing the camera over knee-high plants
    for k, (full, picked, x) in enumerate(plants):
        alex.walk((x + 1, GARDEN_Z - 8))
        alex.act("pick", 0.8, item="tomato", face=(x, GARDEN_Z))
        full.hide(alex.clock - 0.3)
        picked.show(alex.clock - 0.3, x, 0, GARDEN_Z)
    basket_e.hide(alex.clock)
    basket_f.hand(alex.clock, alex)
    t1 = alex.clock
    s.step(T, t1 + 0.4, "Step 1", "Pick the tomatoes")
    s.cam.shot(T, t1 + 0.4, lambda t: alex.position_at(t), dist=66, elev_deg=20, yaw0_deg=20, yaw1_deg=-10, subjects=(alex,))

    # ---- STEP 2: mix the dough (Steve brings flour; Alex brings the tomatoes)
    t2 = t1 + 0.4
    alex.walk((TABLE[0] - 4, TABLE[1] + 16))
    basket_f.show(alex.clock, TABLE[0] + 2, TY, TABLE[1] - 4)
    alex.face(TABLE[0], TABLE[1])

    sack = s.prop("sack", sack_prop())
    sack.show(0, CART[0] + 12, 0, CART[1] + 7)
    steve.until(t2 - 2.2)
    steve.act("pick", 0.5, face=(CART[0] + 12, CART[1] + 7))
    sack.hand(steve.clock, steve)
    steve.walk((TABLE[0] - 8, TABLE[1] - 15))
    sack.show(steve.clock, TABLE[0] - 13, TY, TABLE[1] - 4)
    steve.face(TABLE[0] - 8, TABLE[1])
    steve.until(max(steve.clock, t2 + 0.3))
    dough = s.prop("dough_ball", dough_ball_prop())
    steve.act("stir", 2.0, item="spoon", face=(TABLE[0] - 8, TABLE[1]))
    dough.build(steve.clock - 1.7, TABLE[0] - 8, TY + 1, TABLE[1], secs=1.4)
    t3 = steve.clock + 0.2
    s.step(t2, t3, "Step 2", "Mix the dough")
    s.cam.shot(t2, t3, lambda t: (TABLE[0] - 4, TABLE[1] - 6), dist=82, elev_deg=22, yaw0_deg=-15, yaw1_deg=12, subjects=(steve, alex))

    # ---- STEP 3: sauce and cheese
    base, sauce, top = pizza_layers()
    p_base, p_sauce, p_top = s.prop("pizza_base", base), s.prop("pizza_sauce", sauce), s.prop("pizza_top", top)
    steve.until(t3)
    steve.walk((TABLE[0] + 6, TABLE[1] - 15))
    dough.hide(steve.clock)
    steve.act("knead", 1.4, face=(TABLE[0] + 7, TABLE[1]))
    p_base.build(steve.clock - 1.3, TABLE[0] + 7, TY + 1, TABLE[1], secs=1.1)
    alex.until(steve.clock - 0.4)
    alex.walk((TABLE[0] + 7, TABLE[1] + 15))
    alex.act("chop", 1.3, face=(TABLE[0] + 7, TABLE[1]))
    p_sauce.build(alex.clock - 1.2, TABLE[0] + 7, TY + 1, TABLE[1], secs=0.9)
    basket_f.hide(alex.clock - 1.2)
    basket_e.show(alex.clock - 1.2, TABLE[0] + 2, TY, TABLE[1] - 4)
    # step aside to the west end of the table (out of the oven shots), wait there for the finale
    alex.walk((TABLE[0] - 20, TABLE[1] + 3))
    alex.face(TABLE[0], TABLE[1])
    steve.until(alex.clock - 0.2)
    steve.act("pick", 1.4, face=(TABLE[0] + 7, TABLE[1]))
    p_top.build(steve.clock - 1.3, TABLE[0] + 7, TY + 1, TABLE[1], secs=1.2)
    t4 = steve.clock + 0.2
    s.step(t3, t4, "Step 3", "Add sauce and cheese")
    s.cam.shot(t3, t4, lambda t: (TABLE[0] + 6, TABLE[1]), dist=66, elev_deg=30, yaw0_deg=105, yaw1_deg=75, height=10, subjects=(steve, alex))

    # ---- STEP 4: bake it
    raw = s.prop("pizza_raw", merged("pizza_raw", base, sauce, top))
    baked = s.prop("pizza_baked", merged("pizza_baked", base, sauce, top, recolor=lambda x, y, z, col: {CREAM: "#f0cf8a", CRUST: BAKED, YELLOW: "#ffc53a"}.get(col)))
    steve.until(t4)
    steve.act("pick", 0.4, face=(TABLE[0] + 7, TABLE[1]))
    for p in (p_base, p_sauce, p_top):
        p.hide(steve.clock)
    raw.hand(steve.clock, steve)
    oven_mouth = (OVEN[0] - 1, OVEN[1])
    steve.walk((oven_mouth[0] - 12, oven_mouth[1] - 6), (oven_mouth[0] - 12, oven_mouth[1]))
    steve.face(OVEN[0] + 10, OVEN[1])
    steve.act("push", 0.6)
    raw.show(steve.clock - 0.6, oven_mouth[0] - 6, TY, oven_mouth[1]).show(steve.clock, OVEN[0] + 7, TY, OVEN[1])
    # fire flickers while it bakes, smoke puffs from the chimney
    bake0 = steve.clock
    fire_a, fire_b = s.prop("fire_0", fire_prop(0)), s.prop("fire_1", fire_prop(1))
    fx, fz = OVEN[0] + 15, OVEN[1]
    t = 0.0
    while t < 40:
        (fire_a if int(t / 0.18) % 2 == 0 else fire_b).show(t, fx, TY, fz)
        (fire_b if int(t / 0.18) % 2 == 0 else fire_a).hide(t)
        t += 0.18
    for k in range(60):
        puff = s.prop("smoke", smoke_prop())
        t0 = k * 0.7
        cx, cz = OVEN[0] + 13.5, OVEN[1]
        puff.show(t0, cx, 35, cz).show(t0 + 2.6, cx + 6, 52, cz - 3).hide(t0 + 2.6)
    steve.act("look", 1.6, face=(OVEN[0] + 10, OVEN[1]))
    raw.hide(steve.clock)
    baked.show(steve.clock, OVEN[0] + 7, TY, OVEN[1]).show(steve.clock + 0.5, oven_mouth[0] - 6, TY, oven_mouth[1])
    s.sfx.append([round(steve.clock, 3), "chime"])
    steve.act("pick", 0.5)
    baked.hand(steve.clock, steve)
    steve.walk((TABLE[0] + 21, TABLE[1] - 1))
    baked.show(steve.clock, TABLE[0] + 7, TY + 1, TABLE[1])
    steve.face(TABLE[0], TABLE[1])
    t5 = steve.clock + 0.2
    s.step(t4, t5, "Step 4", "Bake it in the oven")
    s.cam.shot(t4, bake0 - 0.4, lambda t: steve.position_at(t), dist=68, elev_deg=24, yaw0_deg=-15, yaw1_deg=5, subjects=(steve,))
    # look into the oven mouth (it faces -x, towards the table)
    s.cam.shot(bake0 - 0.4, t5 - 1.4, (OVEN[0] - 4, OVEN[1]), dist=64, elev_deg=16, yaw0_deg=-62, yaw1_deg=-48, height=12, subjects=(steve,))
    s.cam.shot(t5 - 1.4, t5, lambda t: steve.position_at(t), dist=72, elev_deg=20, yaw0_deg=-20, yaw1_deg=-5, subjects=(steve,))

    # ---- TA-DA
    alex.until(t5 - 0.6)
    alex.walk((TABLE[0] - 20, TABLE[1] + 3))  # west end of the table: camera sees both and the pizza
    alex.face(TABLE[0], TABLE[1])
    for a in (alex, steve):
        a.until(t5)
        a.act("cheer", 3.4)
    s.sfx.append([round(t5, 3), "fanfare"])
    end = t5 + 3.4
    s.step(t5, end, "Ta-da!", "Pizza is ready!")
    s.cam.shot(t5, end, (TABLE[0] + 1, TABLE[1]), dist=82, elev_deg=20, yaw0_deg=12, yaw1_deg=-12, height=12, subjects=(alex, steve))
    s.duration = end

    # ---- hook: the finished scene, then the story starts
    s.hook_scene = t5 + 0.6
    s.cam.shot(0, T, (TABLE[0] + 1, TABLE[1]), dist=72, elev_deg=20, yaw0_deg=-8, yaw1_deg=-14, height=12, subjects=(alex, steve))
    s.sfx.append([0.15, "question"])
    # equal times: the cut key last, so a cut wins over the previous shot's final key
    s.cam.keys.sort(key=lambda k: (k[0], k[7]))
    s.check()
    s.write()


# =============================================================== shared world pieces
def ring_houses_and_trees(w: Canvas, rng, houses, trees):
    for (x0, z0, door) in houses:
        world_house(w, x0, z0, wx=rng.choice([7, 8]), wz=6, floors=rng.choice([1, 2, 2]), rng=rng, door=door)
    for (x, z) in trees:
        (world_oak if rng.random() < 0.65 else world_birch)(w, rng, x, z)


def paths_from_center(w: Canvas, n=44, skip=lambda x, z: False):
    for i in range(n):
        for j in (21, 22):
            for (x, z) in [(i, j), (j, i)]:
                if (x, 0, z) not in w.v and not skip(x, z):
                    w.set((x, 0, z), PATH)


def fence_posts(w: Canvas, x0, z0, x1, z1, gap=None):
    """Block fence: log posts on the corners and every 2 blocks, plank rails between."""
    for x in range(x0, x1 + 1):
        for z in range(z0, z1 + 1):
            if x in (x0, x1) or z in (z0, z1):
                if gap and gap[0] <= x <= gap[2] and gap[1] <= z <= gap[3]:
                    continue
                post = (x - x0) % 2 == 0 and (z - z0) % 2 == 0
                w.set((x, 1, z), LOG if post else PLANK_D)


def world_barn(w: Canvas, x0, z0, wx=9, wz=7):
    RED, RED_D = "#b23a2e", "#8c2b22"
    w.box((x0, 1, z0), (x0 + wx - 1, 4, z0 + wz - 1), RED, hollow=True)
    for (x, z) in [(x0, z0), (x0 + wx - 1, z0), (x0, z0 + wz - 1), (x0 + wx - 1, z0 + wz - 1)]:
        w.box((x, 1, z), (x, 4, z), WHITE)
    mid = x0 + wx // 2
    w.box((mid - 1, 1, z0 + wz - 1), (mid + 1, 3, z0 + wz - 1), RED_D)
    w.box((mid - 1, 4, z0 + wz - 1), (mid + 1, 4, z0 + wz - 1), WHITE)
    k = 0
    while wz - 2 * k > 0:
        w.box((x0 - 1, 5 + k, z0 - 1 + k), (x0 + wx, 5 + k, z0 + wz - k), "#5a5f69")
        if wz - 2 * k > 2:
            w.box((x0, 5 + k, z0 + k), (x0, 5 + k, z0 + wz - 1 - k), RED)
            w.box((x0 + wx - 1, 5 + k, z0 + k), (x0 + wx - 1, 5 + k, z0 + wz - 1 - k), RED)
        k += 1


def world_silo(w: Canvas, x, z, h=8):
    for y in range(1, h + 1):
        for dx in range(-1, 2):
            for dz in range(-1, 2):
                if abs(dx) + abs(dz) < 2 or y % 3 == 0:
                    w.set((x + dx, y, z + dz), "#c9cdd4" if y % 3 else "#9aa0aa")
    w.set((x, h + 1, z), "#9aa0aa")


def world_pond(w: Canvas, x0, z0, x1, z1):
    for x in range(x0 - 1, x1 + 2):
        for z in range(z0 - 1, z1 + 2):
            w.set((x, 0, z), WATER if x0 <= x <= x1 and z0 <= z <= z1 else SAND)


def farm_world(seed=2) -> Canvas:
    w = Canvas("Farm", seed=seed, keep_origin=True)
    rng = random.Random(seed)
    C0, C1 = 15, 28
    for x in range(C0, C1 + 1):  # dirt farmyard
        for z in range(C0, C1 + 1):
            w.set((x, 0, z), "#8f7a55" if rng.random() < 0.25 else PATH)
    paths_from_center(w)
    fence_posts(w, 15, 15, 19, 19, gap=(17, 19, 18, 19))  # cow pen, gate on the south side
    for x in range(15, 20):  # strawberry bed
        for z in range(24, 26):
            w.set((x, 0, z), FARM)
    world_barn(w, 17, 4)
    world_silo(w, 28, 6)
    for (x, z) in [(30, 26), (30, 27), (31, 26)]:
        w.set((x, 1, z), HAY)
    w.set((30, 2, 26), HAY)
    ring_houses_and_trees(w, rng, [(4, 4, "z"), (32, 3, "z"), (4, 32, "x"), (15, 34, "x"), (33, 33, "x"), (35, 15, "z")],
                          [(11, 11), (32, 11), (11, 31), (31, 30), (2, 26), (41, 25), (24, 41), (9, 40), (40, 6), (39, 40), (1, 2), (0, 13), (13, 2)])
    return w


def meadow_world(seed=3) -> Canvas:
    w = Canvas("Meadow", seed=seed, keep_origin=True)
    rng = random.Random(seed)
    paths_from_center(w, skip=lambda x, z: z > 23 and x == 21)
    world_pond(w, 27, 24, 31, 28)
    world_house(w, 18, 6, wx=7, wz=6, floors=1, rng=rng, door="z")  # beekeeper's cabin, north
    for x in range(17, 26):  # flower bed edge (low hedge) along the field
        w.set((x, 1, 30), LEAVES_D)
    trees = []
    for k in range(40):  # forest ring
        a = k / 40 * math.tau + rng.uniform(-0.05, 0.05)
        r = rng.uniform(14.5, 21)
        trees.append((int(22 + math.cos(a) * r), int(22 + math.sin(a) * r)))
    ring_houses_and_trees(w, rng, [], trees)
    return w


# =============================================================== props: ice cream
COW_W, COW_B, COW_P = "#f2efe6", "#3a3033", "#f0a6b8"
PINK, PINK_L, WAFFLE, WAFFLE_D, BERRY = "#ff7eb6", "#ffc2dc", "#e0a75e", "#b97a3a", "#e5393a"


def cow_prop():
    """Minecraft-sized cow (~1.4 blocks tall), facing +x."""
    c = Canvas("cow")
    c.box((0, 6, 0), (15, 13, 8), COW_W)
    c.paint(lambda x, y, z, col: COW_B if col == COW_W and ((x // 3 * 5 + y // 3 * 3 + z // 3 * 7) % 4 == 0) else None)
    for x, z in [(1, 0), (12, 0), (1, 6), (12, 6)]:
        c.box((x, 0, z), (x + 2, 5, z + 2), COW_W)
        c.box((x, 0, z), (x + 2, 0, z + 2), COW_B)
    c.box((6, 4, 3), (9, 5, 5), COW_P)  # udder
    c.box((16, 8, 1), (21, 14, 7), COW_W)  # head
    c.box((21, 8, 2), (22, 11, 6), COW_P)
    c.set((22, 10, 3), COW_B), c.set((22, 10, 5), COW_B)
    for z in (2, 6):
        c.set((21, 13, z), COW_B)
    c.box((17, 15, 1), (17, 16, 1), "#e9e0c8"), c.box((17, 15, 7), (17, 16, 7), "#e9e0c8")
    c.line((0, 12, 4), (-2, 6, 4), 0.5, COW_W)
    return c


def bucket_prop(full):
    c = Canvas("bucket_full" if full else "bucket_empty")
    c.cyl((3, 0, 3), 2.6, 5, "#b9bcc4", hollow=True)
    c.cyl((3, 0, 3), 2.6, 1, "#9aa0aa")
    if full:
        c.cyl((3, 1, 3), 1.8, 4, "#ffffff")
    c.line((0.5, 5, 3), (5.5, 5, 3), 0.5, "#80858f")
    return c


def berry_bush_prop(full, seed=1):
    c = Canvas("berry_bush" if full else "berry_bush_picked", seed=seed)
    c.ellipsoid((4, 2.5, 4), (4, 2.8, 4), GREEN)
    c.ellipsoid((4, 4, 4), (2.6, 1.6, 2.6), GREEN_D)
    if full:
        for dx, dy, dz in [(3.5, 2, 1), (-3.2, 2.5, 1.5), (0.5, 4.5, 3), (-1, 1.5, -3.3), (2.5, 3.5, -2.5), (-2.5, 4, -1)]:
            c.box((4 + dx, dy, 4 + dz), (4 + dx, dy + 1, 4 + dz), BERRY)
            c.set((4 + dx, dy + 2, 4 + dz), "#5fbf4f")
    return c


def berry_basket_prop(full):
    c = basket_prop(False)
    c.title = "berry_basket" if full else "berry_basket_empty"
    if full:
        c.box((1, 1, 1), (5, 3, 3), BERRY)
        for p in [(2, 4, 2), (4, 4, 1), (3, 4, 3)]:
            c.set(p, BERRY)
            c.set((p[0], 5, p[2]), "#5fbf4f")
    return c


def machine_prop():
    c = Canvas("ice_machine")
    c.box((0, 0, 0), (11, 7, 9), "#c9cdd4")
    c.box((0, 0, 9), (11, 7, 9), "#e3e7ee")
    for x, col in [(2, RED), (5, "#7ad957"), (8, "#3a8be0")]:
        c.box((x, 5, 10), (x + 1, 6, 10), col)
    c.cyl((6, 8, 5), 4.6, 4, "#e3e7ee", hollow=True)
    c.cyl((6, 8, 5), 4.6, 1, "#c9cdd4")
    c.box((5, 8, -2), (6, 16, -1), "#80858f")  # mixer arm
    c.box((5, 15, -1), (6, 16, 5), "#80858f")
    return c


def paddle_prop(phase):
    c = Canvas(f"paddle_{phase}")
    c.line((2, 0, 2), (2, 5, 2), 0.5, "#80858f")
    if phase == 0:
        c.box((0, 0, 2), (4, 1, 2), "#80858f")
    else:
        c.box((2, 0, 0), (2, 1, 4), "#80858f")
    return c


def mix_prop(color, name):
    c = Canvas(name)
    c.cyl((4, 0, 4), 3.8, 2, color)
    c.ellipsoid((4, 2, 4), (2.8, 1.2, 2.8), color)
    return c


def cone_stand_prop():
    c = Canvas("cone_stand")
    c.box((0, 0, 0), (9, 2, 6), WOODP_L)
    for x in (2, 7):
        c.cyl((x, 1, 3), 1.6, 6, WAFFLE, r_end=0.4)
    return c


def cone_prop(scoops):
    c = Canvas("cone_full" if scoops else "cone")
    c.cyl((3, 0, 3), 0.5, 5, WAFFLE, r_end=2.0)
    c.paint(lambda x, y, z, col: WAFFLE_D if col == WAFFLE and (x + y) % 3 == 0 else None)
    if scoops:
        c.sphere((3, 6.2, 3), 2.2, PINK)
        c.sphere((3, 9.2, 3), 1.8, "#fff3d6")
        c.set((3, 11, 3), BERRY)
        rng = c.rng
        for k, col in list(c.v.items()):
            if col in (PINK, "#fff3d6") and rng.random() < 0.1:
                c.v[k] = rng.choice([RED, YELLOW, "#3a8be0", "#7ad957", "#ffffff"])
    return c


def scoop_ball_prop():
    c = Canvas("scoop_ball")
    c.sphere((3, 2.5, 3), 2.8, PINK)
    c.sphere((3, 6.5, 3), 2.3, "#fff3d6")
    c.set((3, 9, 3), BERRY)
    return c


# =============================================================== props: honey
HONEY, HONEY_D, WAX = "#ffb81c", "#e08a00", "#ffe17a"


def bloom_prop(seed):
    c = Canvas(f"bloom{seed}", seed=seed)
    rng = c.rng
    cols = ["#ff5a5a", "#ffd23f", "#ffffff", "#b58cff", "#ff8a2a", "#ff7eb6", "#5ab4ff"]
    for _ in range(7):
        x, z = rng.randint(0, 14), rng.randint(0, 14)
        h = rng.randint(2, 5)
        c.box((x, 0, z), (x, h, z), GREEN)
        c.set((x + 1, h - 1, z), "#5fbf4f")
        col = rng.choice(cols)
        for dx, dz in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]:
            c.set((x + dx, h + 1, z + dz), col if (dx or dz) else YELLOW)
    return c


def bee_prop():
    c = Canvas("bee")
    c.box((0, 1, 0), (3, 3, 2), YELLOW)
    c.paint(lambda x, y, z, col: DARK if x % 2 == 1 else None)
    c.box((4, 1, 0), (5, 3, 2), DARK)
    c.box((1, 4, -1), (2, 4, 0), "#ecf6ff"), c.box((1, 4, 2), (2, 4, 3), "#ecf6ff")
    return c


def hive_prop():
    c = Canvas("hive")
    c.box((0, 0, 0), (9, 2, 9), WOODP_D)
    for i, col in enumerate([WHITE, "#ffe17a", WHITE]):
        c.box((0, 3 + i * 4, 0), (9, 6 + i * 4, 9), col)
    c.box((-1, 15, -1), (10, 16, 10), "#3a8be0")
    c.box((3, 3, 10), (6, 3, 11), WOODP_L)
    c.box((3, 4, 9), (6, 4, 9), DARK)
    return c


def comb_frame_prop():
    c = Canvas("comb_frame")
    c.box((0, 0, 0), (13, 10, 1), WOODP)
    c.erase((1, 1, 0), (12, 9, 1))
    rng = c.rng
    for x in range(1, 13):
        for y in range(1, 10):
            hexwall = (x + (y // 2) % 2) % 3 == 0 or y % 2 == 0 and x % 3 == 1
            c.set((x, y, 0), WAX if hexwall else (HONEY if rng.random() < 0.8 else HONEY_D))
            c.set((x, y, 1), WAX if hexwall else HONEY)
    return c


def jar_prop(full, i=0):
    c = Canvas("jar_full" if full else "jar_empty")
    c.cyl((3, 0, 3), 2.5, 7, "#e6f6ff", hollow=True)
    c.cyl((3, 0, 3), 2.5, 1, "#e6f6ff")
    if full:
        c.cyl((3, 0, 3), 2.5, 6, HONEY)
        c.paint(lambda x, y, z, col: "#fff6d8" if col == HONEY and x == 1 and y % 2 else None)
        c.cyl((3, 7, 3), 2.8, 1, RED)
        c.paint(lambda x, y, z, col: "#ffffff" if col == RED and (x + z) % 2 == 0 else None)
    return c


def honey_pot_prop():
    c = Canvas("honey_pot")
    c.ellipsoid((4, 4, 4), (4, 4, 4), "#c9733a")
    c.cyl((4, 7, 4), 2.6, 2, "#9c5428")
    c.ellipsoid((4, 8.5, 4), (2.4, 0.8, 2.4), HONEY)
    return c


# =============================================================== the ice cream story
def ice_cream():
    s = Story("ice-cream", "Ice cream", "How is ice cream made?")
    s.world = farm_world()
    v = lambda b: b * B
    CX, CZ = v(21.5), v(21.5)
    TABLE = (CX + 12, CZ + 2)
    TY = 10
    COW = (v(17.5), v(17.4))  # cow centre inside the pen (blocks 15..19, interior 16..18)
    BED_Z = v(24.9)
    s.prop("table", table_prop()).show(0, TABLE[0], 0, TABLE[1])
    s.solid(TABLE[0] - 15, TABLE[1] - 7, TABLE[0] + 15, TABLE[1] + 7, 10)
    s.prop("ice_machine", machine_prop()).show(0, TABLE[0] - 7, TY, TABLE[1])
    stand = (TABLE[0] + 8, TABLE[1] - 2)
    s.prop("cone_stand", cone_stand_prop()).show(0, stand[0], TY, stand[1])
    s.prop("cow", cow_prop()).show(0, COW[0], 0, COW[1])
    # pen fence: four walls (gate in the south wall at blocks 17..18) and the cow itself
    s.solid(v(15), v(15), v(20), v(16), 10)
    s.solid(v(15), v(15), v(16), v(20), 10)
    s.solid(v(19), v(15), v(20), v(20), 10)
    s.solid(v(15), v(19), v(17), v(20), 10)
    s.solid(COW[0] - 11, COW[1] - 4.5, COW[0] + 11, COW[1] + 4.5, 15)
    for i, (x, z) in enumerate([(CX + 30, CZ + 30), (CX - 5, CZ + 45), (CX + 40, CZ - 35)]):
        s.prop(f"flowers{i}", flowers_prop(i)).show(0, x, 0, z)

    bushes = []
    for i, x in enumerate([v(15.8), v(17.4), v(19.0)]):
        full = s.prop("berry_bush", berry_bush_prop(True, i)).show(0, x, 0, BED_Z)
        bushes.append((full, s.prop("berry_bush_picked", berry_bush_prop(False, i)), x))
    s.solid(v(15), BED_Z - 4.5, v(20), BED_Z + 4.5, 7)

    steve = s.actor("steve", v(19.5), v(22.5), outfit="farmer", hat="straw", speed=11)
    alex = s.actor("alex", v(13.5), v(27.5), outfit="farmer", hat="straw", speed=11)
    s.hook_until = 1.6
    T = s.hook_until

    # ---- STEP 1: milk the cow (in through the gate, bucket under the udder)
    b_empty, b_full = s.prop("bucket_empty", bucket_prop(False)), s.prop("bucket_full", bucket_prop(True))
    b_empty.hand(0, steve)
    steve.until(T)
    gate = (v(18), v(20.6))
    steve.walk(gate)
    steve.walk((COW[0] + 2, COW[1] + 10))  # in through the gate
    steve.face(COW[0] + 2, COW[1])
    steve.act("pick", 0.4)
    b_empty.show(steve.clock, COW[0] + 1, 0, COW[1] + 3)
    steve.act("pick", 1.2, face=(COW[0] + 1, COW[1] + 3))
    b_empty.hide(steve.clock)
    b_full.show(steve.clock, COW[0] + 1, 0, COW[1] + 3)
    s.sfx.append([round(steve.clock, 3), "pop"])
    steve.act("pick", 0.4)
    b_full.hand(steve.clock, steve)
    t1 = steve.clock + 0.3
    s.step(T, t1, "Step 1", "Milk the cow")
    milk0 = b_empty.keys[1][0] - 0.4  # when Steve reaches the cow
    s.cam.shot(T, milk0, lambda t: steve.position_at(t), dist=64, elev_deg=22, yaw0_deg=35, yaw1_deg=15, subjects=(steve,))
    # from the north side: his face over the cow's back
    s.cam.shot(milk0, t1, (COW[0] + 2, COW[1] + 5), dist=58, elev_deg=24, yaw0_deg=160, yaw1_deg=190, height=12, subjects=(steve,))

    # ---- STEP 2: strawberries (Alex); Steve brings the milk to the stand meanwhile
    steve.walk(gate)
    steve.walk((TABLE[0] - 7, TABLE[1] - 15))
    b_full.show(steve.clock, TABLE[0] - 15, TY, TABLE[1] - 4)
    steve.face(TABLE[0] - 7, TABLE[1])
    bk_e, bk_f = s.prop("berry_basket_empty", berry_basket_prop(False)), s.prop("berry_basket", berry_basket_prop(True))
    bk_e.hand(0, alex)
    alex.until(t1 - 0.8)
    for (full, picked, x) in bushes:
        alex.walk((x, BED_Z + 9))
        alex.act("pick", 0.8, item="strawberry", face=(x, BED_Z))
        full.hide(alex.clock - 0.3)
        picked.show(alex.clock - 0.3, x, 0, BED_Z)
    bk_e.hide(alex.clock)
    bk_f.hand(alex.clock, alex)
    t2 = alex.clock + 0.3
    s.step(t1, t2, "Step 2", "Pick the strawberries")
    s.cam.shot(t1, t2, lambda t: alex.position_at(t), dist=62, elev_deg=22, yaw0_deg=200, yaw1_deg=170, subjects=(alex,))

    # ---- STEP 3: mix and freeze
    alex.walk((TABLE[0] - 6, TABLE[1] + 15))
    alex.face(TABLE[0] - 7, TABLE[1])
    mix_w, mix_p = s.prop("mix_white", mix_prop("#ffffff", "mix_white")), s.prop("mix_pink", mix_prop(PINK_L, "mix_pink"))
    steve.until(t2)
    steve.act("pick", 0.4, face=(TABLE[0] - 15, TABLE[1] - 4))
    b_full.hand(steve.clock, steve)
    steve.act("water", 1.4, face=(TABLE[0] - 7, TABLE[1]))
    mix_w.build(steve.clock - 1.2, TABLE[0] - 7, TY + 9, TABLE[1], secs=1.0)
    b_full.hide(steve.clock)
    b_empty.show(steve.clock, TABLE[0] - 15, TY, TABLE[1] - 4)
    alex.until(steve.clock - 0.2)
    bk_f.show(alex.clock, TABLE[0] - 1, TY, TABLE[1] + 4)
    alex.act("pick", 1.2, item="strawberry", face=(TABLE[0] - 7, TABLE[1]))
    bk_f.hide(alex.clock)
    bk_e.show(alex.clock, TABLE[0] - 1, TY, TABLE[1] + 4)
    mix_w.hide(alex.clock)
    mix_p.build(alex.clock, TABLE[0] - 7, TY + 9, TABLE[1], secs=1.0)
    # the paddle spins while it freezes
    p0, p1 = s.prop("paddle_0", paddle_prop(0)), s.prop("paddle_1", paddle_prop(1))
    t = alex.clock
    t_spin_end = t + 1.2
    k = 0
    while t < t_spin_end:
        (p0 if k % 2 == 0 else p1).show(t, TABLE[0] - 7, TY + 10, TABLE[1])
        (p1 if k % 2 == 0 else p0).hide(t)
        t += 0.12
        k += 1
    p0.hide(t), p1.hide(t)
    steve.act("knead", 1.2, face=(TABLE[0] - 7, TABLE[1]))  # pressing the machine's buttons
    t3 = max(steve.clock, t_spin_end) + 0.2
    s.step(t2, t3, "Step 3", "Mix it and freeze it")
    s.cam.shot(t2, t3, (TABLE[0] - 5, TABLE[1]), dist=66, elev_deg=26, yaw0_deg=-100, yaw1_deg=-75, height=12, subjects=(steve, alex))

    # ---- STEP 4: scoop it up, hand it over
    cone, cone_full, ball = s.prop("cone", cone_prop(False)), s.prop("cone_full", cone_prop(True)), s.prop("scoop_ball", scoop_ball_prop())
    steve.until(t3)
    steve.walk((TABLE[0] + 4, TABLE[1] - 15))
    steve.act("pick", 0.4, face=stand)
    cone.hand(steve.clock, steve)
    steve.act("stir", 1.3, item="scoop", face=(TABLE[0] - 7, TABLE[1]))
    cone.hide(steve.clock)
    cone_full.hand(steve.clock, steve)
    s.sfx.append([round(steve.clock, 3), "pop"])
    alex.until(t3 + 0.6)
    alex.walk((TABLE[0] + 4, TABLE[1] + 15))
    alex.face(TABLE[0] + 4, TABLE[1])
    # over the counter: Steve reaches out, Alex takes it
    steve.act("carry", 0.6, face=(TABLE[0] + 4, TABLE[1] + 15))
    cone_full.hand(steve.clock, alex)
    t4 = steve.clock + 0.3
    s.step(t3, t4, "Step 4", "Scoop it up")
    s.cam.shot(t3, t4, (TABLE[0] + 4, TABLE[1]), dist=62, elev_deg=24, yaw0_deg=105, yaw1_deg=80, height=12, subjects=(steve, alex))

    # ---- TA-DA
    alex.until(t4)
    alex.walk((TABLE[0] - 2, TABLE[1] + 22))
    alex.face(TABLE[0] - 2, TABLE[1] + 40)
    alex.act("eat", 3.0)
    steve.until(t4 + 0.2)
    steve.walk((TABLE[0] + 22, TABLE[1] + 4))
    steve.face(TABLE[0], TABLE[1] + 20)
    steve.act("cheer", 2.6)
    end = max(alex.clock, steve.clock)
    s.sfx.append([round(t4 + 0.6, 3), "fanfare"])
    s.step(t4, end, "Ta-da!", "Ice cream time!")
    s.cam.shot(t4, end, (TABLE[0] + 6, TABLE[1] + 14), dist=72, elev_deg=18, yaw0_deg=20, yaw1_deg=-8, height=12, subjects=(alex, steve))
    s.duration = end

    s.hook_scene = end - 1.6
    s.cam.shot(0, T, (TABLE[0] + 6, TABLE[1] + 14), dist=68, elev_deg=18, yaw0_deg=-2, yaw1_deg=-8, height=12, subjects=(alex, steve))
    s.sfx.append([0.15, "question"])
    s.cam.keys.sort(key=lambda k: (k[0], k[7]))
    s.check()
    s.write()


# =============================================================== the honey story
def honey():
    s = Story("honey", "Honey", "How is honey made?", ambience=("birds", "bees", "wind"))
    s.world = meadow_world()
    v = lambda b: b * B
    CX, CZ = v(21.5), v(21.5)
    TABLE = (v(21.5), v(15.2))  # workbench in front of the cabin
    TY = 10
    HIVES = [(v(15.5), v(19)), (v(15.5), v(23))]
    FIELD = [(v(16.5) + dx * 14, v(25.5) + dz * 14) for dz in range(2) for dx in range(3)]
    s.prop("table", table_prop()).show(0, TABLE[0], 0, TABLE[1])
    s.solid(TABLE[0] - 15, TABLE[1] - 7, TABLE[0] + 15, TABLE[1] + 7, 10)
    for (x, z) in HIVES:
        s.prop("hive", hive_prop()).show(0, x, 0, z)
        s.solid(x - 6, z - 6, x + 6, z + 6, 17)
    s.solid(v(18) - 2, v(6) - 2, v(25) + 2, v(12) + 2, 80)  # cabin
    s.solid(v(26), v(23), v(33), v(30), 2)  # pond (keep feet dry)

    blooms = [s.prop(f"bloom{i}", bloom_prop(i)) for i in range(len(FIELD))]
    alex = s.actor("alex", FIELD[0][0] - 6, FIELD[0][1] - 16, speed=10)
    steve = s.actor("steve", TABLE[0] - 20, TABLE[1] + 14, outfit="beekeeper", hat="bee", speed=10)
    s.hook_until = 1.6
    T = s.hook_until

    # ---- STEP 1: flowers bloom while Alex waters the field
    alex.until(T)
    # two watering stops; the flowers bloom in a wave around her
    for stop, group in (((FIELD[0][0] + 14, FIELD[0][1] - 10), [0, 1, 3, 4]), ((FIELD[2][0] + 6, FIELD[2][1] - 10), [2, 5])):
        alex.walk(stop)
        alex.act("water", 1.4, item="watering_can", face=(stop[0], stop[1] + 20))
        for k, i in enumerate(group):
            x, z = FIELD[i]
            blooms[i].build(alex.clock - 1.3 + k * 0.25, x + 7, 0, z + 7, secs=0.7)
    t1 = alex.clock + 0.3
    s.step(T, t1, "Step 1", "Flowers bloom")
    s.cam.shot(T, t1, lambda t: alex.position_at(t), dist=66, elev_deg=24, yaw0_deg=-30, yaw1_deg=-5, subjects=(alex,))

    # ---- STEP 2: bees carry nectar from the flowers to the hives (they keep flying after)
    for b in range(6):
        bee = s.prop("bee", bee_prop())
        hx, hz = HIVES[b % 2]
        t = t1 - 0.8 + b * 0.35
        k = b
        while t < 60:
            fx, fz = FIELD[(k * 3) % len(FIELD)]
            flower = (fx + 7 + (b % 3) * 2, 8 + (b % 2) * 2, fz + 7)
            hive = (hx + 3, 12 + (b % 3), hz + 6)
            yaw_out = math.degrees(math.atan2(flower[0] - hive[0], flower[2] - hive[2])) - 90
            bee.show(t, *hive, yaw=yaw_out + 180).show(t + 1.1, *flower, yaw=yaw_out).show(t + 1.6, *flower, yaw=yaw_out).show(t + 2.7, *hive, yaw=yaw_out + 180)
            t += 2.9
            k += 1
    steve.until(T + 1.0)
    steve.walk((HIVES[0][0] + 12, HIVES[0][1] + 2))
    steve.until(t1)
    steve.act("look", 2.6, face=HIVES[0])
    t2 = steve.clock + 0.2
    s.step(t1, t2, "Step 2", "Bees bring the nectar home")
    s.cam.shot(t1, t2, (HIVES[0][0] + 6, (HIVES[0][1] + HIVES[1][1]) / 2), dist=64, elev_deg=20, yaw0_deg=70, yaw1_deg=100, height=12, subjects=(steve,))

    # ---- STEP 3: the frame full of honeycomb goes to the workbench
    frame = s.prop("comb_frame", comb_frame_prop())
    steve.act("pick", 0.8, face=HIVES[0])
    frame.build(steve.clock - 0.8, HIVES[0][0] + 6, 0, HIVES[0][1] + 1, secs=0.7)
    frame.hand(steve.clock, steve)
    steve.walk((TABLE[0] - 4, TABLE[1] - 15))  # works from the cabin side, facing the camera
    frame.show(steve.clock, TABLE[0] - 11, TY, TABLE[1] + 1)  # west end of the bench, not in front of him
    steve.face(TABLE[0] - 11, TABLE[1])
    steve.act("knead", 1.0)
    alex.until(t2)
    alex.walk((TABLE[0] + 22, TABLE[1] + 3))  # east end of the bench, out of Steve's way
    alex.face(TABLE[0], TABLE[1])
    t3 = steve.clock + 0.2
    s.step(t2, t3, "Step 3", "They fill the honeycomb")
    s.cam.shot(t2, t2 + (t3 - t2) * 0.55, lambda t: steve.position_at(t), dist=66, elev_deg=22, yaw0_deg=40, yaw1_deg=20, subjects=(steve,))
    s.cam.shot(t2 + (t3 - t2) * 0.55, t3, (TABLE[0] - 4, TABLE[1] + 2), dist=58, elev_deg=26, yaw0_deg=-15, yaw1_deg=8, height=11, subjects=(steve, alex))

    # ---- STEP 4: pour it into jars
    jars_e = [s.prop("jar_empty", jar_prop(False)) for _ in range(3)]
    jars_f = [s.prop("jar_full", jar_prop(True)) for _ in range(3)]
    pot = s.prop("honey_pot", honey_pot_prop())
    jx = [TABLE[0] + 3, TABLE[0] + 8, TABLE[0] + 13]
    for j in jars_e:
        j.show(0, 0, -50, 0)  # parked below ground until the step
    for j, x in zip(jars_e, jx):
        j.show(t3 - 0.01, x, TY, TABLE[1] + 1)
    pot.show(0, TABLE[0] - 3, TY, TABLE[1] - 1)
    steve.until(t3)
    steve.walk((TABLE[0] - 3, TABLE[1] - 15))
    steve.face(TABLE[0] - 3, TABLE[1])
    steve.act("pick", 0.3)
    pot.hand(steve.clock, steve)
    for j_e, j_f, x in zip(jars_e, jars_f, jx):
        steve.walk((x, TABLE[1] - 14), avoid=False)
        steve.act("water", 0.9, face=(x, TABLE[1]))
        j_e.hide(steve.clock - 0.5)
        j_f.build(steve.clock - 0.5, x, TY, TABLE[1] + 1, secs=0.5)
    pot.show(steve.clock, TABLE[0] - 3, TY, TABLE[1] - 1)
    t4 = steve.clock + 0.3
    s.step(t3, t4, "Step 4", "Pour it into jars")
    s.cam.shot(t3, t4, (TABLE[0] + 7, TABLE[1] + 4), dist=62, elev_deg=24, yaw0_deg=-12, yaw1_deg=14, height=11, subjects=(steve, alex))

    # ---- TA-DA
    treat = s.prop("jar_full", jar_prop(True))
    alex.until(t4)
    alex.act("pick", 0.4, face=(jx[2], TABLE[1]))
    jars_f[2].hide(alex.clock)
    treat.hand(alex.clock, alex)
    alex.act("eat", 3.0, face=(TABLE[0] + 22, TABLE[1] + 40))
    steve.until(t4)
    # round the west end of the bench (away from Alex) to the front, then cheer facing us
    steve.walk((TABLE[0] - 21, TABLE[1] - 10), (TABLE[0] - 22, TABLE[1] + 9), speed=14)
    steve.act("cheer", 2.4, face=(TABLE[0] - 22, TABLE[1] + 40))
    end = max(alex.clock, steve.clock)
    s.sfx.append([round(t4 + 0.4, 3), "fanfare"])
    s.step(t4, end, "Ta-da!", "Sweet honey!")
    s.cam.shot(t4, end, (TABLE[0], TABLE[1] + 8), dist=84, elev_deg=18, yaw0_deg=-10, yaw1_deg=10, height=12, subjects=(alex, steve))
    s.duration = end

    s.hook_scene = end - 1.6
    s.cam.shot(0, T, (TABLE[0], TABLE[1] + 8), dist=80, elev_deg=18, yaw0_deg=12, yaw1_deg=6, height=12, subjects=(alex, steve))
    s.sfx.append([0.15, "question"])
    s.cam.keys.sort(key=lambda k: (k[0], k[7]))
    s.check()
    s.write()


STORIES = {"pizza": pizza, "ice-cream": ice_cream, "honey": honey}

if __name__ == "__main__":
    for name in sys.argv[1:] or list(STORIES):
        STORIES[name]()
