"""Static scenery around "how it's made" scenes: village, farm, meadow (blocky, Minecraft-like).

The camera orbits the scene in the middle at roughly 55-80 voxels, so anything inside that ring
must stay low (paths, flowers, fences, fields) or it would hide the scene; tall things (houses,
trees, barns, towers) stand in a far ring and show up behind the scene.
"""
from __future__ import annotations

import math
import random

from vox import Canvas

SIZE = 240
C = SIZE // 2
CLEAR = 20       # empty clearing radius for the scene itself
NEAR_MAX = 92    # low stuff only up to here
FAR_MIN = 98     # tall stuff from here on

GRASS_A, GRASS_B, GRASS_T = "#84ce66", "#78c25c", "#5fae45"
DIRT, DIRT_D = "#b5915f", "#9c7a4c"
LOG, LOG_D, PLANK, PLANK_D = "#6b4a2b", "#523720", "#b8935a", "#9c7a45"
COBBLE, COBBLE_D, GLASS, ROOF, ROOF_D = "#8f8f8f", "#6f6f6f", "#a8dcf2", "#7a3b2a", "#5e2c20"
LEAVES, LEAVES_D, LEAVES_L = "#3f9a3a", "#2f7d2e", "#58b04a"
WATER, WATER_D, SAND = "#3f8fe0", "#3379c4", "#e6d7a0"
WHEAT, WHEAT_G, SOIL = "#e3c25a", "#9ccf4a", "#6e4528"
FLOWERS = ["#ff5a5a", "#ffd23f", "#ffffff", "#b58cff", "#ff8a2a", "#ff7eb6", "#5ab4ff"]


def polar(r, a):
    return C + math.cos(a) * r, C + math.sin(a) * r


def dist(x, z):
    return math.hypot(x - C, z - C)


def ground(c: Canvas, rng: random.Random):
    """Grass with soft two-tone patches (blocky value noise keeps runs long and the file small)."""
    cell = 9
    noise = {(i, j): rng.random() for i in range(SIZE // cell + 2) for j in range(SIZE // cell + 2)}
    for x in range(SIZE):
        for z in range(SIZE):
            c.v[(x, 0, z)] = GRASS_A if noise[(x // cell, z // cell)] > 0.45 else GRASS_B


def paths(c: Canvas, rng: random.Random, ring=True):
    w = 2
    for x in range(SIZE):
        for z in range(SIZE):
            r = dist(x, z)
            on = (abs(x - C) <= w or abs(z - C) <= w) and CLEAR - 2 < r < FAR_MIN + 4
            on |= ring and CLEAR + 4 <= r <= CLEAR + 7
            if on:
                c.v[(x, 0, z)] = DIRT_D if rng.random() < 0.15 else DIRT


def on_path(x, z):
    r = dist(x, z)
    return (abs(x - C) <= 4 or abs(z - C) <= 4) or CLEAR + 2 <= r <= CLEAR + 9


def flowers(c: Canvas, rng: random.Random, n, r0=CLEAR + 10, r1=NEAR_MAX, cluster=5):
    for _ in range(n):
        a, r = rng.random() * math.tau, r0 + rng.random() * (r1 - r0)
        cx, cz = polar(r, a)
        col = rng.choice(FLOWERS)
        for _ in range(cluster):
            x, z = int(cx + rng.gauss(0, 2)), int(cz + rng.gauss(0, 2))
            if not on_path(x, z) and (x, 1, z) not in c.v:
                c.v[(x, 1, z)] = col
                if rng.random() < 0.5:
                    c.v[(x + 1, 1, z)] = GRASS_T


def tufts(c: Canvas, rng: random.Random, n):
    for _ in range(n):
        x, z = rng.randrange(SIZE), rng.randrange(SIZE)
        if CLEAR < dist(x, z) and not on_path(x, z) and (x, 1, z) not in c.v:
            c.v[(x, 1, z)] = GRASS_T


def fence_line(c: Canvas, a, b, col=PLANK_D):
    (x0, z0), (x1, z1) = a, b
    n = int(max(abs(x1 - x0), abs(z1 - z0)))
    for i in range(n + 1):
        x, z = round(x0 + (x1 - x0) * i / max(n, 1)), round(z0 + (z1 - z0) * i / max(n, 1))
        c.v[(x, 2, z)] = col
        if i % 3 == 0:
            c.v[(x, 1, z)] = col
            c.v[(x, 3, z)] = col


def field(c: Canvas, x0, z0, w, d, crop="wheat"):
    for x in range(x0, x0 + w):
        for z in range(z0, z0 + d):
            c.v[(x, 0, z)] = SOIL
            if (z - z0) % 2 == 0:
                continue
            if crop == "wheat":
                c.v[(x, 1, z)] = WHEAT_G if (x * 3 + z) % 5 == 0 else WHEAT
                if (x + z) % 3 == 0:
                    c.v[(x, 2, z)] = WHEAT
            elif crop == "carrot":
                c.v[(x, 1, z)] = "#5fbf4f" if x % 2 else "#ff8a2a"
            elif crop == "pumpkin" and (x - x0) % 4 == 1:
                c.box((x, 1, z), (x + 1, 2, z), "#f08a1c")
                c.v[(x, 3, z)] = "#3f9a3a"
    fence_line(c, (x0 - 1, z0 - 1), (x0 + w, z0 - 1))
    fence_line(c, (x0 - 1, z0 + d), (x0 + w, z0 + d))
    fence_line(c, (x0 - 1, z0 - 1), (x0 - 1, z0 + d))
    fence_line(c, (x0 + w, z0 - 1), (x0 + w, z0 + d))


def pond(c: Canvas, rng: random.Random, cx, cz, rx, rz):
    for x in range(int(cx - rx - 2), int(cx + rx + 3)):
        for z in range(int(cz - rz - 2), int(cz + rz + 3)):
            d = ((x - cx) / rx) ** 2 + ((z - cz) / rz) ** 2
            if d <= 1.0:
                c.v[(x, 0, z)] = WATER_D if d < 0.4 else WATER
                if rng.random() < 0.03:
                    c.v[(x, 1, z)] = "#5fbf4f"  # lily pad
            elif d <= 1.35:
                c.v[(x, 0, z)] = SAND


def oak(c: Canvas, rng: random.Random, x, z, h=None):
    h = h or rng.randint(6, 9)
    c.box((x, 1, z), (x, h, z), LOG)
    top = h
    for (dy, r) in [(-2, 2), (-1, 2), (0, 2), (1, 1), (2, 1)]:
        for dx in range(-r, r + 1):
            for dz in range(-r, r + 1):
                if abs(dx) == r and abs(dz) == r and (r == 2 or rng.random() < 0.5):
                    continue
                if (x + dx, top + dy, z + dz) not in c.v or dy > 0 or dx or dz:
                    c.v[(x + dx, top + dy, z + dz)] = LEAVES_D if rng.random() < 0.25 else LEAVES
    c.v[(x, top + 3, z)] = LEAVES_L


def birch(c: Canvas, rng: random.Random, x, z):
    h = rng.randint(7, 10)
    for y in range(1, h + 1):
        c.v[(x, y, z)] = "#2b2b2b" if rng.random() < 0.2 else "#e8e4d8"
    for (dy, r) in [(-1, 2), (0, 2), (1, 1), (2, 1)]:
        for dx in range(-r, r + 1):
            for dz in range(-r, r + 1):
                if abs(dx) == r and abs(dz) == r:
                    continue
                c.v[(x + dx, h + dy, z + dz)] = "#7cc24a" if rng.random() < 0.7 else "#5fa83a"


def house(c: Canvas, rng: random.Random, x0, z0, w=9, d=7, h=5, face="z"):
    """Plank house on a cobblestone footing with log corners, glass windows, a door and a gable roof."""
    c.box((x0, 1, z0), (x0 + w - 1, 1, z0 + d - 1), COBBLE)
    c.box((x0, 2, z0), (x0 + w - 1, h + 1, z0 + d - 1), PLANK)
    for (x, z) in [(x0, z0), (x0 + w - 1, z0), (x0, z0 + d - 1), (x0 + w - 1, z0 + d - 1)]:
        c.box((x, 1, z), (x, h + 1, z), LOG)
    for x in range(x0 + 2, x0 + w - 2, 3):
        c.v[(x, 3, z0)] = GLASS
        c.v[(x, 3, z0 + d - 1)] = GLASS
        c.v[(x, 4, z0)] = GLASS
        c.v[(x, 4, z0 + d - 1)] = GLASS
    mid = x0 + w // 2
    door_z = z0 + d - 1 if face == "z" else z0
    c.box((mid, 2, door_z), (mid, 3, door_z), LOG_D)
    # gable roof along x
    k = 0
    while d - 2 * k > 0:
        c.box((x0 - 1, h + 2 + k, z0 - 1 + k), (x0 + w, h + 2 + k, z0 + d - k), ROOF if k else ROOF_D)
        if d - 2 * k > 2:
            c.box((x0, h + 2 + k, z0 + k), (x0, h + 2 + k, z0 + d - 1 - k), PLANK_D)
            c.box((x0 + w - 1, h + 2 + k, z0 + k), (x0 + w - 1, h + 2 + k, z0 + d - 1 - k), PLANK_D)
        k += 1
    c.box((x0 + w - 3, h + 2, z0 + 1), (x0 + w - 3, h + 2 + k + 1, z0 + 1), COBBLE_D)  # chimney


def tower(c: Canvas, x0, z0):
    """Church / watch tower: a tall stone landmark on the horizon."""
    c.box((x0, 1, z0), (x0 + 6, 18, z0 + 6), COBBLE)
    c.box((x0, 1, z0), (x0 + 6, 1, z0 + 6), COBBLE_D)
    for y in (6, 12, 16):
        for (x, z) in [(x0 + 3, z0), (x0 + 3, z0 + 6), (x0, z0 + 3), (x0 + 6, z0 + 3)]:
            c.v[(x, y, z)] = GLASS
            c.v[(x, y + 1, z)] = GLASS
    for k in range(5):
        c.box((x0 - 1 + k, 19 + k, z0 - 1 + k), (x0 + 7 - k, 19 + k, z0 + 7 - k), ROOF)
    c.v[(x0 + 3, 24, z0 + 3)] = "#f2c230"
    c.v[(x0 + 3, 25, z0 + 3)] = "#f2c230"


def barn(c: Canvas, x0, z0):
    RED, RED_D, WHITE = "#b23a2e", "#8c2b22", "#f2efe6"
    w, d, h = 16, 12, 8
    c.box((x0, 1, z0), (x0 + w - 1, h, z0 + d - 1), RED)
    c.paint(lambda x, y, z, col: RED_D if col == RED and x0 <= x < x0 + w and z0 <= z < z0 + d and (x - x0) % 2 == 0 else None)
    for x in (x0, x0 + w - 1):
        c.box((x, 1, z0), (x, h, z0), WHITE)
        c.box((x, 1, z0 + d - 1), (x, h, z0 + d - 1), WHITE)
    c.box((x0 + 5, 1, z0 + d - 1), (x0 + 10, 6, z0 + d - 1), WHITE)  # big door frame
    c.box((x0 + 6, 1, z0 + d - 1), (x0 + 9, 5, z0 + d - 1), RED_D)
    k = 0
    while d - 2 * k > 0:
        c.box((x0 - 1, h + 1 + k, z0 - 1 + k), (x0 + w, h + 1 + k, z0 + d - k), "#5a5f69")
        k += 1


def silo(c: Canvas, x, z):
    c.cyl((x, 1, z), 3.2, 17, "#c9cdd4")
    for y in (4, 8, 12, 16):
        c.cyl((x, y, z), 3.4, 1, "#9aa0aa")
    c.sphere((x, 18, z), 3.2, "#9aa0aa", where=lambda px, py, pz: py >= 18)


def windmill(c: Canvas, x, z):
    c.cyl((x, 1, z), 3.5, 14, "#f2efe6", r_end=2.5)
    c.cone((x, 15, z), 3.2, 4, ROOF)
    hub = (x, 13, z + 3)
    for k in range(4):
        a = k * math.pi / 2 + 0.4
        end = (hub[0] + math.cos(a) * 10, hub[1] + math.sin(a) * 10, hub[2])
        c.line(hub, end, 0.5, PLANK_D)
        for t in range(3, 10):
            px, py = hub[0] + math.cos(a) * t, hub[1] + math.sin(a) * t
            nx, ny = -math.sin(a), math.cos(a)
            for s in (1, 2):
                c.set((px + nx * s, py + ny * s, hub[2]), "#f2efe6")


def cow(c: Canvas, x, z, facing=1):
    W, B, P = "#f7f4ee", "#3a3033", "#ffc2dc"
    c.box((x - 3, 3, z - 1), (x + 3, 5, z + 1), W)
    c.paint(lambda px, py, pz, col: B if col == W and abs(px - x) <= 3 and 3 <= py <= 5 and abs(pz - z) <= 1 and (px + py) % 3 == 0 else None)
    for dx in (-3, 3):
        for dz in (-1, 1):
            c.box((x + dx, 1, z + dz), (x + dx, 2, z + dz), W)
    hx = x + 4 * facing
    c.box((hx, 4, z - 1), (hx + facing, 6, z + 1), W)
    c.box((hx + facing, 4, z - 1), (hx + facing, 4, z + 1), P)


def hive_box(c: Canvas, x, z):
    c.box((x, 1, z), (x + 3, 1, z + 3), LOG_D)
    c.box((x, 2, z), (x + 3, 5, z + 3), "#f2efe6")
    c.box((x, 3, z), (x + 3, 3, z + 3), "#ffd23f")
    c.box((x - 1, 6, z - 1), (x + 4, 6, z + 4), "#3a8be0")


def far_ring(rng: random.Random, n, r0=FAR_MIN, r1=116, jitter=0.15):
    """Evenly spread spots in the far ring, with some randomness."""
    out = []
    for k in range(n):
        a = k / n * math.tau + rng.uniform(-jitter, jitter)
        out.append((a, polar(r0 + rng.random() * (r1 - r0), a)))
    return out


def corner_forest(c: Canvas, rng: random.Random, every=7):
    """Trees past the far ring (square corners) so the edge of the world isn't bare."""
    for x in range(4, SIZE - 4, every):
        for z in range(4, SIZE - 4, every):
            if dist(x, z) > 122 and rng.random() < 0.7:
                (oak if rng.random() < 0.7 else birch)(c, rng, x + rng.randint(-2, 2), z + rng.randint(-2, 2))


def village(seed=1) -> Canvas:
    c = Canvas("Village", seed=seed)
    rng = random.Random(seed)
    ground(c, rng)
    paths(c, rng)
    pond(c, rng, *polar(62, 2.3), 9, 6)
    for a in (0.6, 3.9):
        x, z = polar(72, a)
        field(c, int(x) - 7, int(z) - 5, 14, 10)
    flowers(c, rng, 70)
    tufts(c, rng, 400)
    for a in (0.3, 1.25, 2.0, 3.3, 4.6, 5.5):  # low fences along the paths
        x, z = polar(50, a)
        fence_line(c, (x - 6, z), (x + 6, z))
    spots = far_ring(rng, 22)
    for i, (a, (x, z)) in enumerate(spots):
        if i == 5:
            tower(c, int(x) - 3, int(z) - 3)
        elif i % 2 == 0:
            house(c, rng, int(x) - 4, int(z) - 3, w=rng.choice([9, 11]), d=7, h=rng.choice([4, 5]))
        else:
            for _ in range(2):
                (oak if rng.random() < 0.75 else birch)(c, rng, int(x) + rng.randint(-4, 4), int(z) + rng.randint(-4, 4))
    corner_forest(c, rng)
    return c


def farm(seed=2) -> Canvas:
    c = Canvas("Farm", seed=seed)
    rng = random.Random(seed)
    ground(c, rng)
    paths(c, rng, ring=False)
    crops = ["wheat", "carrot", "pumpkin", "wheat"]
    for i, a in enumerate((0.45, 1.9, 3.5, 5.2)):
        x, z = polar(66, a)
        field(c, int(x) - 9, int(z) - 6, 18, 12, crops[i])
    pond(c, rng, *polar(58, 2.7), 8, 5)
    flowers(c, rng, 40)
    tufts(c, rng, 400)
    for i, (a, (x, z)) in enumerate(far_ring(rng, 18)):
        if i == 0:
            barn(c, int(x) - 8, int(z) - 6)
        elif i == 3:
            silo(c, int(x), int(z))
        elif i == 7:
            windmill(c, int(x), int(z))
        elif i == 11:
            house(c, rng, int(x) - 5, int(z) - 4, w=11, d=8, h=5)
        elif i in (5, 13):
            for k in range(3):
                cow(c, int(x) + k * 9 - 9, int(z) + rng.randint(-3, 3), 1 if k % 2 else -1)
            fence_line(c, (x - 14, z - 7), (x + 14, z - 7))
            fence_line(c, (x - 14, z + 7), (x + 14, z + 7))
        elif i in (9, 15):
            for k in range(3):
                c.box((int(x) + k * 4, 1, int(z)), (int(x) + k * 4 + 2, 2, int(z) + 2), WHEAT)
        else:
            for _ in range(2):
                oak(c, rng, int(x) + rng.randint(-4, 4), int(z) + rng.randint(-4, 4))
    corner_forest(c, rng)
    return c


def meadow(seed=3) -> Canvas:
    c = Canvas("Meadow", seed=seed)
    rng = random.Random(seed)
    ground(c, rng)
    paths(c, rng, ring=False)
    pond(c, rng, *polar(64, 4.0), 10, 7)
    flowers(c, rng, 260, cluster=7)
    tufts(c, rng, 500)
    for a in (0.8, 2.2):
        for k in range(4):
            x, z = polar(104, a + k * 0.06)
            hive_box(c, int(x), int(z))
    for a, (x, z) in far_ring(rng, 34, jitter=0.08):
        for _ in range(2):
            (oak if rng.random() < 0.6 else birch)(c, rng, int(x) + rng.randint(-4, 4), int(z) + rng.randint(-4, 4))
    corner_forest(c, rng, every=6)
    return c
