"""Tiny voxel canvas for hand-made builds: shapes, painting, rotation; writes voxreel build JSON.

Coordinates: integer voxels, x right, y up, z towards the viewer. Shapes test voxel centres,
so `box` corners are inclusive like the DSL. Output is a list of 1-voxel-thick `box` runs along x
per colour, which the renderer, the website and the Minecraft export all read as-is.
"""
from __future__ import annotations

import json
import math
import random
from pathlib import Path


class Canvas:
    def __init__(self, title: str, seed: int = 1, keep_origin: bool = False):
        """`keep_origin`: coordinates are absolute (a world other things are placed in): nothing
        below 0 on any axis is kept and the output is not shifted."""
        self.title = title
        self.v: dict[tuple[int, int, int], str] = {}
        self.rng = random.Random(seed)
        self.keep_origin = keep_origin

    # -- primitives ----------------------------------------------------------
    def set(self, p, c):
        x, y, z = (int(round(k)) for k in p)
        if y >= 0 and (not self.keep_origin or (x >= 0 and z >= 0)):
            self.v[(x, y, z)] = c

    def get(self, p):
        return self.v.get(tuple(int(round(k)) for k in p))

    def erase(self, a, b):
        lo = [min(a[i], b[i]) for i in range(3)]
        hi = [max(a[i], b[i]) for i in range(3)]
        for k in [k for k in self.v if all(lo[i] <= k[i] <= hi[i] for i in range(3))]:
            del self.v[k]

    def box(self, a, b, c, hollow=False):
        lo = [int(round(min(a[i], b[i]))) for i in range(3)]
        hi = [int(round(max(a[i], b[i]))) for i in range(3)]
        for x in range(lo[0], hi[0] + 1):
            for y in range(lo[1], hi[1] + 1):
                for z in range(lo[2], hi[2] + 1):
                    if hollow and lo[0] < x < hi[0] and lo[1] < y < hi[1] and lo[2] < z < hi[2]:
                        continue
                    self.set((x, y, z), c)

    def ellipsoid(self, center, radii, c, hollow=False, where=None):
        cx, cy, cz = center
        rx, ry, rz = radii
        for x in range(math.floor(cx - rx), math.ceil(cx + rx) + 1):
            for y in range(math.floor(cy - ry), math.ceil(cy + ry) + 1):
                for z in range(math.floor(cz - rz), math.ceil(cz + rz) + 1):
                    d = ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 + ((z - cz) / rz) ** 2
                    if d > 1.0:
                        continue
                    if hollow:
                        inner = ((x - cx) / max(rx - 1, 0.5)) ** 2 + ((y - cy) / max(ry - 1, 0.5)) ** 2 + ((z - cz) / max(rz - 1, 0.5)) ** 2
                        if inner <= 1.0:
                            continue
                    if where and not where(x, y, z):
                        continue
                    self.set((x, y, z), c)

    def sphere(self, center, r, c, hollow=False, where=None):
        self.ellipsoid(center, (r, r, r), c, hollow, where)

    def cyl(self, base, r, h, c, axis="y", hollow=False, r_end=None):
        """Cylinder (or frustum with `r_end`) from `base` along +axis for `h` voxels (h<0 goes back)."""
        ax = "xyz".index(axis)
        u, w = [i for i in range(3) if i != ax]
        step = 1 if h >= 0 else -1
        n = abs(int(h))
        for k in range(n):
            t = k / max(n - 1, 1)
            rr = r if r_end is None else r + (r_end - r) * t
            for a in range(math.floor(base[u] - rr), math.ceil(base[u] + rr) + 1):
                for b in range(math.floor(base[w] - rr), math.ceil(base[w] + rr) + 1):
                    d = math.hypot(a - base[u], b - base[w])
                    if d > rr or (hollow and d <= rr - 1):
                        continue
                    p = [0, 0, 0]
                    p[ax], p[u], p[w] = base[ax] + k * step, a, b
                    self.set(p, c)

    def cone(self, base, r, h, c, axis="y"):
        self.cyl(base, r, h, c, axis, r_end=0.4)

    def line(self, a, b, r, c):
        n = max(1, int(math.dist(a, b) * 2))
        for i in range(n + 1):
            t = i / n
            p = [a[k] + (b[k] - a[k]) * t for k in range(3)]
            if r <= 0.6:
                self.set(p, c)
            else:
                self.sphere(p, r, c)

    def scatter(self, region, n, c, on_top=True):
        """`n` single voxels on random columns of `region` ((x0,z0),(x1,z1)), on top of what's there."""
        (x0, z0), (x1, z1) = region
        for _ in range(n):
            x, z = self.rng.randint(x0, x1), self.rng.randint(z0, z1)
            ys = [k[1] for k in self.v if k[0] == x and k[2] == z]
            if ys or not on_top:
                self.set((x, (max(ys) + 1) if ys else 0, z), c)

    def paint(self, fn):
        """Recolour existing voxels: fn(x, y, z, colour) -> new colour or None to keep."""
        for k, c in list(self.v.items()):
            n = fn(*k, c)
            if n:
                self.v[k] = n

    def place(self, other: "Canvas", offset=(0, 0, 0)):
        for (x, y, z), c in other.v.items():
            self.set((x + offset[0], y + offset[1], z + offset[2]), c)

    # -- output --------------------------------------------------------------
    def spec(self) -> dict:
        if not self.v:
            raise ValueError(f"{self.title}: empty canvas")
        mins = [min(k[i] for k in self.v) for i in range(3)]
        # Shift so the build starts at 0 on every axis (y stays: the ground is y = 0).
        mins[1] = 0
        if self.keep_origin:
            mins = [0, 0, 0]
        runs: dict[str, list] = {}
        rows: dict[tuple, list] = {}
        for (x, y, z), c in self.v.items():
            rows.setdefault((y, z, c), []).append(x)
        for (y, z, c), xs in rows.items():
            xs.sort()
            start = prev = xs[0]
            for x in xs[1:] + [None]:
                if x is not None and x == prev + 1:
                    prev = x
                    continue
                runs.setdefault(c, []).append(([start - mins[0], y, z - mins[2]], [prev - mins[0], y, z - mins[2]]))
                if x is not None:
                    start = prev = x
        size = [max(k[i] for k in self.v) - mins[i] + 1 for i in range(3)]
        ops = [{"op": "box", "from": a, "to": b, "color": c} for c, rs in runs.items() for a, b in rs]
        return {"title": self.title, "size": size, "ops": ops}

    def dump(self, path: Path):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.spec(), separators=(",", ":")))
        print(f"wrote {path} ({len(self.v)} voxels)")
