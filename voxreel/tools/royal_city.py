"""Generates builds/royal-city/{1,2,3}.json: an original white-stone, blue-roof fantasy capital."""
import json, sys, math
from pathlib import Path

STONE = "#e9e4d8"; STONE2 = "#d2ccbd"; STONE_D = "#a9a397"; COBBLE = "#8c8a86"
ROOF = "#2f63c9"; ROOF_D = "#21479a"; GOLD = "#f2c230"; WATER = "#3aa6e0"; WATER_D = "#2b86c2"
GRASS = "#6cbf4a"; GRASS_D = "#57a83c"; WOOD = "#7a4b2a"; DARK = "#2b2f3a"; WINDOW = "#ffd66b"
RED = "#c8322c"; LEAF = "#3f9a3a"

class B:
    def __init__(s, title, size): s.title, s.size, s.ops = title, size, []
    def box(s, a, b, c, hollow=False): s.ops.append({"op": "box", "from": list(a), "to": list(b), "color": c, **({"hollow": True} if hollow else {})})
    def cyl(s, base, r, h, c, hollow=False): s.ops.append({"op": "cylinder", "base": list(base), "radius": r, "height": h, "color": c, **({"hollow": True} if hollow else {})})
    def cone(s, base, r, h, c): s.ops.append({"op": "cone", "base": list(base), "radius": r, "height": h, "color": c})
    def vox(s, pts, c): s.ops.append({"op": "voxels", "points": [list(p) for p in pts], "color": c})
    def erase(s, a, b): s.ops.append({"op": "erase", "from": list(a), "to": list(b)})
    def dump(s, path):
        Path(path).write_text(json.dumps({"title": s.title, "size": s.size, "ops": s.ops}))

    # --- helpers -----------------------------------------------------------
    def crenel(s, x0, x1, z0, z1, y, c=STONE):
        """Battlements on the rim of rectangle [x0,x1)x[z0,z1) at height y (every other block)."""
        pts = []
        for x in range(x0, x1):
            if (x - x0) % 2 == 0: pts += [(x, y, z0), (x, y, z1 - 1)]
        for z in range(z0, z1):
            if (z - z0) % 2 == 0: pts += [(x0, y, z), (x1 - 1, y, z)]
        s.vox(pts, c)

    def tower(s, cx, cz, r, h, y0=0, roof_h=None, spire=True, ring=True):
        """Round white tower with a blue cone roof and gold tip."""
        s.cyl((cx, y0, cz), r, h, STONE)
        if ring: s.cyl((cx, y0 + h - 2, cz), r + 0.6, 1, STONE2)
        s.cyl((cx, y0, cz), r + 0.4, 2, STONE_D)
        rh = roof_h or int(r * 2.6)
        s.cone((cx, y0 + h, cz), r + 1.2, rh, ROOF)
        if spire: s.vox([(int(cx), y0 + h + rh + k, int(cz)) for k in range(2)], GOLD)
        # windows on 4 sides
        ir = int(r)
        for y in range(y0 + 4, y0 + h - 3, 4):
            s.vox([(int(cx), y, int(cz) - ir), (int(cx), y, int(cz) + ir), (int(cx) - ir, y, int(cz)), (int(cx) + ir, y, int(cz))], WINDOW)

    def hip_roof(s, x0, x1, z0, z1, y, c=ROOF, trim=ROOF_D):
        """Stepped pyramid/hip roof over [x0,x1)x[z0,z1) starting at y."""
        k = 0
        while x0 + k < x1 - k and z0 + k < z1 - k:
            s.box((x0 - 1 + k, y + k, z0 - 1 + k), (x1 + 1 - k, y + k + 1, z1 + 1 - k), trim if k == 0 else c)
            k += 1
        return y + k

    def gable_roof(s, x0, x1, z0, z1, y, along_x=True, c=ROOF):
        """Gabled roof, ridge along x (or z)."""
        span = (z1 - z0) if along_x else (x1 - x0)
        k = 0
        while 2 * k < span + 2:
            if along_x: s.box((x0 - 1, y + k, z0 - 1 + k), (x1 + 1, y + k + 1, z1 + 1 - k), c if k else ROOF_D)
            else: s.box((x0 - 1 + k, y + k, z0 - 1), (x1 + 1 - k, y + k + 1, z1 + 1), c if k else ROOF_D)
            k += 1
        # gable walls
        for kk in range(k - 1):
            if along_x:
                s.box((x0, y + kk, z0 + kk), (x0 + 1, y + kk + 1, z1 - kk), STONE)
                s.box((x1 - 1, y + kk, z0 + kk), (x1, y + kk + 1, z1 - kk), STONE)
            else:
                s.box((x0 + kk, y + kk, z0), (x1 - kk, y + kk + 1, z0 + 1), STONE)
                s.box((x0 + kk, y + kk, z1 - 1), (x1 - kk, y + kk + 1, z1), STONE)

    def house(s, x0, z0, w, d, h, y=1, along_x=True, door_side="z0"):
        s.box((x0, y, z0), (x0 + w, y + h, z0 + d), STONE)
        s.box((x0, y, z0), (x0 + w, y + 1, z0 + d), STONE_D)
        # timber corners
        for (x, z) in [(x0, z0), (x0 + w - 1, z0), (x0, z0 + d - 1), (x0 + w - 1, z0 + d - 1)]:
            s.box((x, y, z), (x + 1, y + h, z + 1), WOOD)
        mx, mz = x0 + w // 2, z0 + d // 2
        if door_side == "z0": s.box((mx, y, z0), (mx + 1, y + 2, z0 + 1), WOOD); s.vox([(x0 + 1, y + 2, z0), (x0 + w - 2, y + 2, z0)], WINDOW)
        if door_side == "z1": s.box((mx, y, z0 + d - 1), (mx + 1, y + 2, z0 + d), WOOD); s.vox([(x0 + 1, y + 2, z0 + d - 1), (x0 + w - 2, y + 2, z0 + d - 1)], WINDOW)
        if door_side == "x0": s.box((x0, y, mz), (x0 + 1, y + 2, mz + 1), WOOD); s.vox([(x0, y + 2, z0 + 1), (x0, y + 2, z0 + d - 2)], WINDOW)
        if door_side == "x1": s.box((x0 + w - 1, y, mz), (x0 + w, y + 2, mz + 1), WOOD); s.vox([(x0 + w - 1, y + 2, z0 + 1), (x0 + w - 1, y + 2, z0 + d - 2)], WINDOW)
        s.gable_roof(x0, x0 + w, z0, z0 + d, y + h, along_x)

    def banner(s, x, y, z, length=4, face="z", c=ROOF):
        """Hanging blue banner with a gold stripe, flat in the x (face z) or z (face x) plane."""
        for k in range(length):
            col = GOLD if k == 1 else c
            if face == "z": s.vox([(x, y - k, z), (x + 1, y - k, z)], col)
            else: s.vox([(x, y - k, z), (x, y - k, z + 1)], col)

    def tree(s, x, z, y=1, h=3):
        s.box((x, y, z), (x + 1, y + h, z + 1), WOOD)
        s.ops.append({"op": "sphere", "center": [x + 0.5, y + h + 0.5, z + 0.5], "radius": 1.8, "color": LEAF})


# ---------------------------------------------------------------- 1: city gate
def gate():
    b = B("Kingdom gate", [48, 44, 40])
    W, D = 48, 40
    b.box((0, 0, 0), (W, 1, D), GRASS)
    b.box((0, 0, 0), (W, 1, 9), WATER)                      # moat in front
    b.box((0, 0, 8), (W, 1, 9), STONE_D)
    b.box((20, 1, 0), (28, 2, 9), STONE2)                  # bridge deck
    b.vox([(x, 2, z) for x in (20, 27) for z in range(0, 9, 2)], STONE)  # bridge railing posts
    b.box((20, 0, 2), (28, 1, 3), STONE_D); b.box((20, 0, 6), (28, 1, 7), STONE_D)  # bridge piers
    # curtain wall
    b.box((2, 1, 10), (46, 13, 14), STONE)
    b.box((2, 1, 10), (46, 2, 14), STONE_D)
    b.box((2, 13, 10), (46, 14, 14), STONE2)
    b.crenel(2, 46, 10, 14, 14)
    # gatehouse
    b.box((16, 1, 9), (32, 21, 16), STONE)
    b.box((15, 20, 8), (33, 21, 17), STONE2)
    b.crenel(15, 33, 8, 17, 21)
    b.hip_roof(18, 30, 10, 15, 21)
    # arch opening
    b.erase((21, 1, 8), (27, 11, 17))
    b.erase((22, 11, 8), (26, 12, 17))
    b.erase((23, 12, 8), (25, 13, 17))
    b.box((21, 1, 13), (27, 9, 14), DARK)                   # portcullis shadow
    b.vox([(x, y, 12) for x in range(21, 27) for y in range(9, 11)], DARK)
    b.vox([(x, y, 12) for x in range(21, 27, 2) for y in range(1, 9)], COBBLE)  # portcullis bars
    b.box((21, 1, 0), (27, 2, 9), STONE2)
    # gold crest above the gate
    b.box((22, 14, 8), (26, 18, 9), ROOF)
    b.vox([(23, 15, 7), (24, 15, 7), (23, 16, 7), (24, 16, 7), (22, 17, 7), (25, 17, 7)], GOLD)
    # banners
    for x in (17, 30): b.banner(x, 18, 8, 6)
    # big gate towers
    for cx in (14, 34): b.tower(cx, 12, 4.5, 26, 0, roof_h=12)
    # corner towers
    for cx in (3, 45): b.tower(cx, 12, 3.2, 18, 0, roof_h=8)
    # guardian statues on plinths (original: knights with spears)
    for x in (17, 30):
        b.box((x - 1, 1, 0), (x + 2, 4, 3), STONE_D)
        b.box((x, 4, 1), (x + 1, 7, 2), STONE2)             # body
        b.vox([(x, 7, 1)], STONE)                            # head
        b.vox([(x, 8, 1)], GOLD)                             # helmet plume
        b.vox([(x + (1 if x < 24 else -1), y, 1) for y in range(4, 10)], COBBLE)  # spear
    # city behind the wall: rooftops + keep spire
    b.house(4, 18, 8, 6, 5, along_x=True, door_side="z0")
    b.house(36, 18, 8, 6, 5, along_x=True, door_side="z0")
    b.house(5, 28, 6, 8, 6, along_x=False, door_side="x1")
    b.house(37, 28, 6, 8, 6, along_x=False, door_side="x0")
    b.box((20, 1, 17), (28, 2, 40), STONE2)                 # main street
    b.box((17, 1, 26), (31, 16, 36), STONE)                 # keep
    b.crenel(17, 31, 26, 36, 16)
    b.hip_roof(18, 30, 27, 35, 16)
    b.tower(24, 31, 3.5, 22, 0, roof_h=14)
    for z in (20, 24): b.tree(14, z); b.tree(33, z)
    return b


# ---------------------------------------------------------------- 2: royal castle
def castle():
    b = B("Royal castle", [46, 52, 46])
    C = 23
    b.cyl((C, 0, C), 22, 1, WATER)
    b.cyl((C, 0, C), 19, 3, STONE_D)                         # rock island
    b.cyl((C, 3, C), 18, 1, GRASS)
    b.box((C - 2, 1, 0), (C + 3, 3, 6), STONE2)              # bridge
    # outer ring wall with towers
    b.cyl((C, 4, C), 16, 7, STONE, hollow=True)
    b.cyl((C, 4, C), 15, 7, STONE, hollow=True)
    b.erase((C - 2, 4, 5), (C + 3, 9, 10))                    # front gate
    b.box((C - 2, 4, 7), (C + 3, 8, 8), DARK)
    for k in range(8):
        a = k * math.tau / 8 + math.tau / 16
        b.tower(C + 0.5 + math.cos(a) * 15.5, C + 0.5 + math.sin(a) * 15.5, 2.6, 11, 4, roof_h=6)
    # courtyard
    b.cyl((C, 4, C), 14.5, 1, STONE2)
    for (x, z) in [(12, 22), (33, 22), (16, 32), (30, 32)]: b.tree(x, z, y=5, h=2)
    # main keep, terraced
    b.box((C - 9, 5, C - 7), (C + 10, 18, C + 9), STONE)
    b.box((C - 9, 5, C - 7), (C + 10, 6, C + 9), STONE_D)
    b.crenel(C - 9, C + 10, C - 7, C + 9, 18)
    b.box((C - 6, 18, C - 4), (C + 7, 28, C + 7), STONE)
    b.crenel(C - 6, C + 7, C - 4, C + 7, 28)
    b.hip_roof(C - 4, C + 5, C - 2, C + 5, 28)
    # windows rows
    b.vox([(x, y, C - 8) for x in range(C - 7, C + 9, 3) for y in (9, 14)], WINDOW)
    b.vox([(x, y, C - 5) for x in range(C - 4, C + 6, 3) for y in (21, 25)], WINDOW)
    # door + steps
    b.box((C - 1, 5, C - 8), (C + 2, 9, C - 7), WOOD)
    b.box((C - 2, 4, C - 11), (C + 3, 5, C - 7), STONE2)
    # tall spires
    b.tower(C - 9, C - 7, 3.2, 26, 5, roof_h=10)
    b.tower(C + 10, C - 7, 3.2, 26, 5, roof_h=10)
    b.tower(C - 9, C + 9, 3.2, 22, 5, roof_h=9)
    b.tower(C + 10, C + 9, 3.2, 22, 5, roof_h=9)
    b.tower(C + 0.5, C + 2, 3.8, 33, 5, roof_h=12)            # central spire
    for x in (C - 5, C + 6): b.banner(x, 16, C - 8, 6)
    b.banner(C, 26, C - 5, 4)
    return b


# ---------------------------------------------------------------- 3: canal town + cathedral
def canal_town():
    b = B("Canal town", [48, 46, 44])
    W, D = 48, 44
    b.box((0, 0, 0), (W, 1, D), STONE2)
    b.box((0, 0, 0), (W, 1, 3), GRASS)
    # canal across the town (x direction) and one along z
    b.box((0, 0, 9), (W, 1, 15), WATER)
    b.box((0, 1, 8), (W, 2, 9), STONE); b.box((0, 1, 15), (W, 2, 16), STONE)
    b.box((0, 1, 9), (W, 1, 15), WATER)
    for bx in (8, 38):                                        # arched bridges
        b.box((bx, 1, 8), (bx + 4, 3, 16), STONE)
        b.erase((bx + 1, 1, 9), (bx + 3, 2, 15))
        b.box((bx, 3, 8), (bx + 1, 4, 16), STONE_D); b.box((bx + 3, 3, 8), (bx + 4, 4, 16), STONE_D)
    # little boat
    b.box((22, 1, 10), (28, 2, 13), WOOD); b.vox([(25, y, 11) for y in range(2, 6)], WOOD)
    b.box((24, 3, 12), (25, 6, 13), "#ffffff")
    # front row of houses (before canal)
    b.house(1, 3, 7, 5, 4, along_x=True, door_side="z0")
    b.house(40, 3, 7, 5, 4, along_x=True, door_side="z0")
    # houses behind canal
    b.house(1, 18, 7, 6, 5, along_x=True, door_side="z0")
    b.house(1, 27, 6, 7, 6, along_x=False, door_side="x1")
    b.house(1, 37, 7, 6, 5, along_x=True, door_side="z0")
    b.house(40, 18, 7, 6, 5, along_x=True, door_side="z0")
    b.house(41, 27, 6, 7, 7, along_x=False, door_side="x0")
    b.house(40, 37, 7, 6, 4, along_x=True, door_side="z0")
    # cathedral in the middle
    x0, x1, z0, z1 = 14, 34, 22, 42
    b.box((x0, 1, z0), (x1, 2, z1), STONE_D)
    b.box((17, 1, z0), (31, 14, z1), STONE)                  # nave
    b.gable_roof(17, 31, z0 + 4, z1, 14, along_x=False)
    b.box((x0, 1, 30), (x1, 11, 36), STONE)                  # transept
    b.gable_roof(x0, x1, 30, 36, 11, along_x=True)
    # front facade with twin towers
    b.box((15, 1, 19), (33, 20, 25), STONE)
    b.crenel(15, 33, 19, 25, 20)
    b.erase((22, 1, 19), (26, 7, 20)); b.box((22, 1, 20), (26, 7, 21), WOOD)
    b.vox([(23, 7, 19), (24, 7, 19)], WOOD)
    # rose window
    b.vox([(23, 12, 18), (24, 12, 18), (22, 13, 18), (25, 13, 18), (22, 14, 18), (25, 14, 18), (23, 15, 18), (24, 15, 18)], GOLD)
    b.vox([(23, 13, 18), (24, 13, 18), (23, 14, 18), (24, 14, 18)], ROOF)
    for x in (17, 30): b.vox([(x, y, 18) for y in range(4, 9)] + [(x + 1, y, 18) for y in range(4, 9)], WINDOW)
    b.tower(16, 21, 3, 30, 0, roof_h=12)
    b.tower(32, 21, 3, 30, 0, roof_h=12)
    # crossing spire
    b.cyl((24, 14, 33), 2.5, 8, STONE)
    b.cone((24, 22, 33), 3.5, 14, ROOF)
    b.vox([(24, 36, 33), (24, 37, 33), (23, 37, 33), (25, 37, 33), (24, 38, 33)], GOLD)
    # side windows
    b.vox([(16, y, z) for z in range(27, 41, 3) for y in (5, 6, 7, 8)], WINDOW)
    b.vox([(31, y, z) for z in range(27, 41, 3) for y in (5, 6, 7, 8)], WINDOW)
    # plaza fountain + trees
    b.cyl((24, 1, 4.5), 3, 1, STONE); b.cyl((24, 1, 4.5), 2, 1, WATER)
    b.vox([(24, 2, 4), (24, 3, 4)], STONE); b.vox([(24, 4, 4)], WATER)
    for x in (12, 35): b.tree(x, 5)
    for x in (10, 37): b.banner(x, 6, 16, 3)
    return b


if __name__ == "__main__":
    out = Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
    for i, f in enumerate([gate, castle, canal_town], 1):
        f().dump(out / f"{i}.json")
        print("wrote", out / f"{i}.json")
