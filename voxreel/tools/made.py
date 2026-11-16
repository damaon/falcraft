"""Hand-made "how it's made" episodes: writes made/<slug>/{1..4,product}.json + recipe.json.

    uv run --no-project tools/made.py            # all episodes
    uv run --no-project tools/made.py pizza      # one episode
    cargo run --release -- made made/pizza/recipe.json
"""
from __future__ import annotations

import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from vox import Canvas  # noqa: E402
import scenery  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent / "made"

# shared palette
GRASS, GRASS_D, SOIL, SOIL_D = "#6cbf4a", "#57a83c", "#8a5a36", "#6e4528"
LEAF, LEAF_D, LEAF_L = "#3f9a3a", "#2f7d2e", "#5fbf4f"
WOOD, WOOD_L, WOOD_D = "#9a6338", "#c08552", "#6e4326"
WHITE, CREAM, GREY, GREY_D, DARK = "#f7f4ee", "#f3e3c3", "#b9bcc4", "#80858f", "#3a3033"
RED, RED_D, ORANGE, YELLOW, GOLD = "#e5393a", "#a8262a", "#ff8a2a", "#ffd23f", "#f2b01e"
BLUE, BLUE_L, PINK, PINK_L, BROWN = "#3a8be0", "#8fd0ff", "#ff7eb6", "#ffc2dc", "#6b3e26"


def grass_base(c: Canvas, w: int, d: int):
    c.box((0, 0, 0), (w - 1, 0, d - 1), GRASS)
    c.paint(lambda x, y, z, col: GRASS_D if y == 0 and (x * 7 + z * 13) % 11 == 0 else None)


def tile_floor(c: Canvas, w: int, d: int, a=WHITE, b="#e9d9c4"):
    for x in range(w):
        for z in range(d):
            c.set((x, 0, z), a if ((x // 3) + (z // 3)) % 2 == 0 else b)


def table(c: Canvas, x0, z0, x1, z1, top_y=8, col=WOOD_L, leg=WOOD):
    c.box((x0, top_y, z0), (x1, top_y + 1, z1), col)
    c.box((x0, top_y, z0), (x1, top_y, z1), WOOD)  # darker underside edge
    for x, z in [(x0 + 1, z0 + 1), (x1 - 2, z0 + 1), (x0 + 1, z1 - 2), (x1 - 2, z1 - 2)]:
        c.box((x, 0, z), (x + 1, top_y - 1, z + 1), leg)


def fence(c: Canvas, w: int, d: int, col=WOOD_L):
    for x in range(0, w, 4):
        for z in (0, d - 1):
            c.box((x, 1, z), (x, 3, z), col)
    for z in range(0, d, 4):
        for x in (0, w - 1):
            c.box((x, 1, z), (x, 3, z), col)
    for y in (2,):
        c.box((0, y, 0), (w - 1, y, 0), col)
        c.box((0, y, d - 1), (w - 1, y, d - 1), col)
        c.box((0, y, 0), (0, y, d - 1), col)
        c.box((w - 1, y, 0), (w - 1, y, d - 1), col)


# ================================================================ PIZZA
def tomato_plant(c: Canvas, x, z, y=2):
    c.line((x + 2, y, z), (x + 2, y + 12, z), 0.5, WOOD_L)  # stake
    c.sphere((x, y + 4, z), 3.2, LEAF)
    c.sphere((x + 0.5, y + 8, z - 0.5), 2.7, LEAF_D)
    c.sphere((x - 0.5, y + 11, z), 1.8, LEAF)
    for dx, dy, dz in [(2.6, 4, 1.2), (-2.4, 6, 1.6), (0.6, 8.5, 2.6), (-1.2, 3, -2.8), (2.2, 9.5, -1.4), (-2.6, 9, -0.6), (1.4, 5.5, -2.6)]:
        c.sphere((x + dx, y + dy, z + dz), 1.2, RED)
        c.set((x + dx, y + dy + 1.3, z + dz), LEAF_L)


def pizza_garden():
    c = Canvas("Tomato garden")
    W = D = 30
    grass_base(c, W, D)
    for z0 in (6, 19):
        c.box((3, 1, z0), (26, 1, z0 + 4), SOIL)
        c.paint(lambda x, y, z, col: SOIL_D if y == 1 and col == SOIL and (x + z) % 3 == 0 else None)
        for x in (7, 15, 23):
            tomato_plant(c, x, z0 + 2)
    fence(c, W, D)
    # watering can
    c.box((24, 1, 26), (27, 4, 28), BLUE)
    c.line((24, 3, 27), (21, 5, 27), 0.5, BLUE)
    c.box((25, 5, 27), (26, 5, 27), BLUE)
    # basket of picked tomatoes
    c.box((3, 1, 13), (7, 3, 16), WOOD_L, hollow=True)
    c.box((4, 3, 14), (6, 3, 15), RED)
    return c


def dough_table():
    c = Canvas("Making the dough", seed=2)
    W = D = 30
    tile_floor(c, W, D)
    table(c, 2, 5, 27, 24, top_y=8)
    top = 10
    # bowl with dough
    c.cyl((10, top, 18), 3.5, 5, "#5aa9dc", hollow=True, r_end=5.5)
    c.cyl((10, top, 18), 3.5, 1, "#5aa9dc")
    c.ellipsoid((10, top + 4, 18), (4.2, 2.2, 4.2), CREAM)
    c.sphere((10, top + 6, 18), 1.6, CREAM)
    # flour sack + spilled flour
    c.ellipsoid((21, top + 4, 10), (3.5, 4.5, 3), "#e3b36f")
    c.box((20, top + 8, 9), (22, top + 9, 11), "#c9b48a")
    c.box((19, top + 3, 13), (23, top + 5, 13), BLUE_L)  # label stripe
    for x, z in [(17, 12), (18, 14), (16, 13), (19, 15), (15, 16), (17, 16)]:
        c.set((x, top, z), WHITE)
    # rolling pin
    c.cyl((14, top + 1, 7), 1.2, 9, WOOD_L, axis="x")
    c.cyl((12, top + 1, 7), 0.6, 2, WOOD, axis="x")
    c.cyl((23, top + 1, 7), 0.6, 2, WOOD, axis="x")
    # eggs
    for x, z in [(5, 8), (7, 9), (5, 10)]:
        c.ellipsoid((x, top + 1, z), (0.9, 1.4, 0.9), WHITE)
    return c


def pizza_disc(c: Canvas, cx, cy, cz, r, toppings=True, seed=3):
    """Raw pizza: crust ring, sauce, cheese, pepperoni and basil, starting at height cy."""
    c.cyl((cx, cy, cz), r, 1, CREAM)
    c.cyl((cx, cy + 1, cz), r, 1, "#e9c78f", hollow=True)
    c.cyl((cx, cy + 1, cz), r - 1, 1, RED)
    if not toppings:
        return
    rng = c.rng
    for x in range(int(cx - r), int(cx + r) + 1):
        for z in range(int(cz - r), int(cz + r) + 1):
            if math.hypot(x - cx, z - cz) <= r - 1.6 and rng.random() < 0.55:
                c.set((x, cy + 1, z), YELLOW)
    for a in range(0, 360, 60):
        for rr in (r * 0.35, r * 0.68):
            px, pz = cx + math.cos(math.radians(a + rr * 9)) * rr, cz + math.sin(math.radians(a + rr * 9)) * rr
            c.cyl((px, cy + 2, pz), 1.5, 1, RED_D)
    for a in range(20, 360, 90):
        c.set((cx + math.cos(math.radians(a)) * r * 0.5, cy + 2, cz + math.sin(math.radians(a)) * r * 0.5), LEAF_L)


def topping_table():
    c = Canvas("Sauce and cheese", seed=3)
    W = D = 30
    tile_floor(c, W, D)
    table(c, 2, 5, 27, 24, top_y=8)
    top = 10
    c.cyl((13, top, 15), 9.5, 1, WOOD)  # board
    pizza_disc(c, 13, top + 1, 15, 8.5)
    # sauce jar
    c.cyl((24, top, 8), 2.4, 5, RED)
    c.cyl((24, top + 5, 8), 2.6, 1, GREY)
    c.box((22, top + 2, 10), (26, top + 3, 10), WHITE)
    # cheese block with holes
    c.box((21, top, 18), (25, top + 3, 22), YELLOW)
    for p in [(21, top + 2, 19), (23, top + 3, 20), (25, top + 1, 21), (22, top + 1, 22)]:
        c.set(p, GOLD)
    # spoon
    c.line((4, top, 9), (8, top + 1, 11), 0.5, GREY)
    c.sphere((3.5, top, 8.5), 1.0, GREY)
    return c


def oven():
    c = Canvas("Pizza oven", seed=4)
    W = D = 32
    c.box((0, 0, 0), (W - 1, 0, D - 1), "#c9c3b5")
    BRICK, BRICK_D, STONE = "#c4553a", "#9a3f2b", "#e6dccb"
    c.sphere((15.5, 8, 14.5), 10, BRICK)
    c.erase((0, 0, 0), (31, 7, 31))
    c.box((4, 1, 3), (27, 7, 26), BRICK_D)
    c.paint(lambda x, y, z, col: "#b44a33" if col == BRICK and (y % 3 == 0 or (x + y // 3 * 2) % 5 == 0) else None)
    # opening with stone arch, dark inside and a fire
    c.box((11, 8, 22), (20, 14, 26), STONE)
    c.erase((12, 8, 15), (19, 12, 31))
    c.erase((13, 13, 15), (18, 13, 31))
    c.box((12, 8, 15), (19, 13, 15), DARK)
    c.box((12, 7, 15), (19, 7, 25), "#5a4a44")
    for x, z in [(13, 17), (15, 16), (17, 17), (18, 16)]:
        c.box((x, 8, z), (x + 1, 8, z), WOOD_D)
    c.cone((15.5, 9, 17), 3.2, 4, ORANGE)
    c.cone((15.5, 9, 17), 1.8, 3, YELLOW)
    # pizza on a peel going in
    c.box((12, 9, 20), (19, 9, 25), WOOD_L)
    c.line((15.5, 9, 26), (15.5, 11, 31), 0.5, WOOD)
    pizza_disc(c, 15.5, 10, 22.5, 3.2, seed=5)
    # chimney + smoke
    c.box((14, 15, 8), (17, 23, 11), BRICK_D)
    for (x, y, z, r) in [(16, 26, 9.5, 1.6), (17, 29, 10, 2.1), (15, 32, 9, 2.5)]:
        c.sphere((x, y, z), r, "#eef2f7")
    # log pile
    for i, (x, y) in enumerate([(27, 2), (29, 2), (28, 4)]):
        c.cyl((x, y, 26), 1.2, 5, WOOD_D, axis="z")
        c.set((x, y, 31), WOOD_L)
    return c


def pizza_product():
    c = Canvas("Pizza", seed=6)
    cx = cz = 20
    c.cyl((cx, 0, cz), 18, 1, WOOD)
    c.cyl((cx, 0, cz), 18, 1, WOOD_D, hollow=True)
    pizza = Canvas("tmp", seed=7)
    pizza_disc(pizza, cx, 1, cz, 16)
    pizza.paint(lambda x, y, z, col: "#d9a95f" if col == "#e9c78f" else ("#f6dfa8" if col == CREAM else None))  # baked crust
    # cut out one slice (wedge facing +z) and lift it up, tilted, with stretchy cheese
    half = math.radians(24)
    slice_vox = {}
    for (x, y, z), col in list(pizza.v.items()):
        dx, dz = x - cx, z - cz
        ang = math.atan2(dx, dz)
        if abs(ang) <= half and math.hypot(dx, dz) <= 16.5:
            slice_vox[(x, y, z)] = col
            del pizza.v[(x, y, z)]
    c.place(pizza)
    # lifted straight up and pulled out a bit, crust end slightly higher
    tilt = math.radians(14)
    lift, pull = 7, 4
    for (x, y, z), col in slice_vox.items():
        for sz in (-0.3, 0.3):
            dx, dy, dz = x - cx, y - 1, z + sz - cz
            ny = dy * math.cos(tilt) + dz * math.sin(tilt)
            nz = -dy * math.sin(tilt) + dz * math.cos(tilt)
            c.set((cx + dx, 1 + lift + ny, cz + pull + nz), col)
    # stretchy cheese between the pizza and the slice
    for dx, dz in [(-2, 4), (0, 6), (2, 4), (-3, 9), (3, 9), (0, 11)]:
        c.line((cx + dx, 3, cz + dz), (cx + dx, lift + 1 + dz * math.sin(tilt), cz + pull + dz - 1), 0.5, YELLOW)
    # steam puffs
    for x, y, z in [(14, 6, 14), (25, 7, 16), (19, 9, 11)]:
        c.sphere((x, y, z), 1.1, "#ffffff")
    return c


# ================================================================ ICE CREAM
WAFFLE, WAFFLE_D, VANILLA, CHOCO, MINT = "#e0a75e", "#b97a3a", "#fff3d6", "#7a4a2e", "#9be3c4"
SPRINKLES = [RED, YELLOW, BLUE, "#7ad957", PINK, "#ffffff"]


def waffle(c: Canvas):
    c.paint(lambda x, y, z, col: WAFFLE_D if col == WAFFLE and ((x + y) % 3 == 0 or (z - y) % 3 == 0) else None)


def cone(c: Canvas, x, y, z, h, r):
    """Waffle cone, tip down at (x, y, z)."""
    c.cyl((x, y, z), 0.5, h, WAFFLE, r_end=r)


def cow_field():
    c = Canvas("Cow", seed=11)
    W = D = 30
    grass_base(c, W, D)
    fence(c, W, D)
    # body with spots
    c.box((6, 6, 10), (19, 13, 18), WHITE)
    c.paint(lambda x, y, z, col: DARK if col == WHITE and 6 <= x <= 19 and y >= 6 and ((x // 3 * 5 + y // 3 * 3 + z // 3 * 7) % 4 == 0) else None)
    for x, z in [(7, 11), (17, 11), (7, 16), (17, 16)]:
        c.box((x, 1, z), (x + 1, 5, z + 1), WHITE)
        c.box((x, 1, z), (x + 1, 1, z + 1), DARK)  # hooves
    c.box((11, 5, 13), (14, 5, 15), PINK_L)  # udder
    # head
    c.box((20, 9, 11), (25, 15, 17), WHITE)
    c.box((26, 9, 11), (27, 12, 17), PINK_L)
    c.set((27, 11, 12), DARK), c.set((27, 11, 16), DARK)
    for z in (12, 16):
        c.set((26, 14, z), DARK)
        c.set((26, 15, z), WHITE)
    c.box((22, 16, 11), (22, 17, 11), CREAM), c.box((22, 16, 17), (22, 17, 17), CREAM)  # horns
    c.box((21, 14, 10), (22, 14, 10), PINK_L), c.box((21, 14, 18), (22, 14, 18), PINK_L)  # ears
    c.line((6, 12, 14), (4, 7, 14), 0.5, WHITE)
    c.set((4, 6, 14), DARK)
    # milk bucket + stool
    c.cyl((12, 1, 23), 2.6, 4, GREY, hollow=True)
    c.cyl((12, 1, 23), 1.8, 3, WHITE)
    c.box((5, 1, 22), (7, 3, 24), WOOD_L)
    # hay bale
    c.box((22, 1, 22), (27, 4, 26), YELLOW)
    c.paint(lambda x, y, z, col: GOLD if col == YELLOW and (x + y) % 2 == 0 else None)
    return c


def strawberry_patch():
    c = Canvas("Strawberries", seed=12)
    W = D = 30
    grass_base(c, W, D)
    for z0 in (4, 12):
        c.box((3, 1, z0), (26, 1, z0 + 4), SOIL)
        for x in range(6, 26, 6):
            c.ellipsoid((x, 2, z0 + 2), (2.6, 1.6, 2.6), LEAF)
            for dx, dz in [(-2, 2), (2, 1), (0, -2), (1, 3)]:
                c.box((x + dx, 2, z0 + 2 + dz), (x + dx, 3, z0 + 2 + dz), RED)
                c.set((x + dx, 4, z0 + 2 + dz), LEAF_L)
    # basket full of berries
    c.box((10, 1, 21), (19, 5, 27), WOOD_L, hollow=True)
    c.paint(lambda x, y, z, col: WOOD if col == WOOD_L and y % 2 == 0 and 21 <= z <= 27 else None)
    c.box((11, 5, 22), (18, 5, 26), RED)
    c.scatter(((11, 22), (18, 26)), 18, RED)
    c.scatter(((11, 22), (18, 26)), 8, LEAF_L)
    for i in range(9):  # handle arch
        a = math.pi * i / 8
        c.set((14.5 + math.cos(a) * 4.5, 6 + math.sin(a) * 5, 24), WOOD)
    return c


def ice_machine():
    c = Canvas("Ice cream machine", seed=13)
    W = D = 30
    tile_floor(c, W, D, PINK_L, WHITE)
    c.box((8, 1, 8), (21, 10, 20), GREY)
    c.box((8, 1, 20), (21, 10, 20), "#d7dbe2")
    for x, col in [(11, RED), (14, "#7ad957"), (17, BLUE)]:
        c.box((x, 7, 21), (x + 1, 8, 21), col)  # buttons
    c.box((10, 3, 21), (19, 4, 21), DARK)  # vent
    c.cyl((14.5, 11, 14), 5.5, 5, "#e3e7ee", hollow=True)
    c.cyl((14.5, 11, 14), 4.6, 3, PINK)
    c.cyl((14.5, 14, 14), 3.4, 1, PINK_L)
    c.cyl((14.5, 15, 14), 2.0, 1, PINK)
    c.box((14, 11, 6), (15, 21, 7), GREY_D)  # mixer post + arm + paddle
    c.box((14, 20, 7), (15, 21, 14), GREY_D)
    c.line((14.5, 20, 14), (14.5, 13, 14), 0.5, GREY_D)
    # ice cubes + milk + berries around
    for (x, y, z) in [(3, 1, 4), (25, 1, 5), (4, 1, 24), (24, 1, 25), (3, 3, 5)]:
        c.box((x, y, z), (x + 1, y + 1, z + 1), BLUE_L)
    for (x, y, z) in [(6, 18, 10), (23, 16, 18), (21, 22, 9), (8, 14, 21)]:
        c.set((x, y, z), BLUE_L), c.set((x + 1, y, z), "#ffffff"), c.set((x, y + 1, z), "#ffffff")
    c.cyl((25, 1, 12), 1.6, 6, WHITE)
    c.cyl((25, 7, 12), 1.0, 1, BLUE)
    for x, z in [(4, 13), (5, 15), (3, 16)]:
        c.box((x, 1, z), (x, 2, z), RED)
        c.set((x, 3, z), LEAF_L)
    return c


def scoop_counter():
    c = Canvas("Scooping", seed=14)
    W = D = 30
    tile_floor(c, W, D, PINK_L, WHITE)
    table(c, 2, 6, 27, 24, top_y=8, col=WHITE, leg=PINK)
    top = 10
    # tub of strawberry ice cream
    c.cyl((9, top, 15), 5, 5, WHITE, hollow=True)
    c.cyl((9, top, 15), 4.2, 4, PINK)
    c.ellipsoid((9, top + 4, 15), (3.6, 1.2, 3.6), PINK_L)
    # scoop lifting a ball
    c.line((14, top + 7, 15), (20, top + 10, 15), 0.5, GREY)
    c.sphere((12, top + 7, 15), 2.4, PINK)
    # cone stand with cones
    c.box((17, top, 9), (25, top + 3, 21), WOOD_L)
    for z in (11, 15, 19):
        c.erase((20, top + 1, z - 1), (22, top + 3, z + 1))
        cone(c, 21, top + 1, z, 7, 2.4)
        waffle(c)
    c.sphere((21, top + 9, 15), 2.6, VANILLA)
    c.sphere((21, top + 9, 19), 2.6, CHOCO)
    return c


def ice_cream_product():
    c = Canvas("Ice cream", seed=15)
    c.cyl((15, 0, 15), 7, 1, PINK_L)
    c.cyl((15, 1, 15), 3, 3, PINK, hollow=True)
    cone(c, 15, 1, 15, 19, 7.5)
    waffle(c)
    scoops = [(PINK, 7.6, 22), (VANILLA, 6.8, 31), (CHOCO, 5.8, 38.5)]
    for col, r, y in scoops:
        c.sphere((15, y, 15), r, col)
    # drips running down each scoop
    for col, r, y in scoops:
        for a in range(0, 360, 72):
            x, z = 15 + math.cos(math.radians(a)) * (r - 0.5), 15 + math.sin(math.radians(a)) * (r - 0.5)
            c.line((x, y - 1, z), (x, y - 1 - (2 + (a // 72) % 3), z), 0.5, col)
    # sprinkles on the surface
    rng = c.rng
    for k, col in list(c.v.items()):
        if col in (PINK, VANILLA, CHOCO) and k[1] > 14 and rng.random() < 0.08:
            c.v[k] = rng.choice(SPRINKLES)
    c.sphere((15, 45, 15), 1.7, RED)
    c.line((15, 46, 15), (16.5, 48.5, 15), 0.5, LEAF)
    return c


# ================================================================ HONEY
HONEY, HONEY_D, WAX, POT, POT_D = "#ffb81c", "#e08a00", "#ffe17a", "#c9733a", "#9c5428"
PETALS = [PINK, "#ffffff", "#b58cff", ORANGE, RED]


def flower(c: Canvas, x, z, h, col):
    c.line((x, 1, z), (x, h, z), 0.5, LEAF)
    c.set((x, h + 1, z), YELLOW)
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        c.set((x + dx, h + 1, z + dz), col)
    c.set((x + 1, h, z), LEAF_L)


def sunflower(c: Canvas, x, z, h):
    c.line((x, 1, z), (x, h, z), 0.6, LEAF)
    c.cyl((x, h, z), 3.2, 1, YELLOW, axis="z")
    c.cyl((x, h, z + 1), 1.6, 1, BROWN, axis="z")
    c.box((x - 2, h - 5, z), (x - 1, h - 5, z), LEAF_L)


def flower_meadow():
    c = Canvas("Flowers", seed=21)
    W = D = 30
    grass_base(c, W, D)
    rng = c.rng
    taken = set()
    for _ in range(40):
        x, z = rng.randint(2, 27), rng.randint(2, 27)
        if any(abs(x - a) < 3 and abs(z - b) < 3 for a, b in taken):
            continue
        taken.add((x, z))
        flower(c, x, z, rng.randint(2, 5), rng.choice(PETALS))
    for x, z, h in [(8, 9, 14), (20, 14, 17), (13, 22, 12)]:
        sunflower(c, x, z, h)
    return c


def bee(c: Canvas, x, y, z, facing=1):
    c.ellipsoid((x, y, z), (2.4, 1.6, 1.6), YELLOW)
    c.paint(lambda px, py, pz, col: DARK if col == YELLOW and abs(px - x) <= 2.5 and abs(py - y) <= 2 and abs(pz - z) <= 2 and (px - x) % 2 == 0 else None)
    c.sphere((x + 2.6 * facing, y + 0.4, z), 1.2, DARK)
    c.set((x - 3 * facing, y, z), DARK)
    c.box((x - 1, y + 2, z - 2), (x, y + 2, z - 1), "#ecf6ff")
    c.box((x - 1, y + 2, z + 1), (x, y + 2, z + 2), "#ecf6ff")


def bee_hive():
    c = Canvas("Bees", seed=22)
    W = D = 30
    grass_base(c, W, D)
    # stacked hive boxes on a stand
    c.box((4, 1, 9), (13, 3, 18), WOOD_D)
    for i, col in enumerate([WHITE, WAX, WHITE]):
        c.box((4, 4 + i * 4, 9), (13, 7 + i * 4, 18), col)
    c.box((3, 16, 8), (14, 17, 19), BLUE)
    c.box((7, 4, 19), (10, 4, 20), WOOD_L)  # landing board
    c.box((7, 5, 18), (10, 5, 18), DARK)  # entrance
    for x, z in [(18, 8), (23, 12), (19, 18), (25, 22), (16, 24)]:
        flower(c, x, z, 3 + (x % 3), PETALS[x % len(PETALS)])
    bee(c, 18, 12, 12, 1)
    bee(c, 23, 9, 20, -1)
    bee(c, 15, 18, 22, 1)
    for k in range(6):  # dotted flight path
        c.set((11 + k * 1.2, 7 + k * 0.9, 20 - k * 0.5), "#ffffff")
    return c


def honeycomb():
    c = Canvas("Honeycomb", seed=23)
    W, D = 30, 22
    grass_base(c, W, D)
    x0, x1, y0, y1, z = 3, 26, 3, 26, 10
    c.box((x0 - 1, 1, z - 1), (x1 + 1, y1 + 1, z + 2), WOOD)  # frame
    c.erase((x0, y0, z - 1), (x1, y1, z + 2))
    for leg in (x0 - 1, x1):
        c.box((leg, 1, z - 3), (leg + 1, 2, z + 4), WOOD_D)
    centers = [(i * 5 + (j % 2) * 2.5 + x0, j * 4.4 + y0, j * 31 + i) for j in range(7) for i in range(6)]
    rng = c.rng
    fill = {cid: rng.choice([HONEY, HONEY, HONEY_D, WAX, "#6e4a12"]) for *_, cid in centers}
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            cx, cy, cid = min(centers, key=lambda p: (p[0] - x) ** 2 + (p[1] - y) ** 2)
            dx, dy = abs(x - cx), abs(y - cy)
            hexd = max(dx * 0.866 + dy * 0.5, dy)
            for zz in range(z, z + 2):
                c.set((x, y, zz), GOLD if hexd > 2.0 else fill[cid])
    bee(c, 9, 20, 13, 1)
    bee(c, 20, 9, 13, -1)
    for x, zz in [(6, 17), (24, 18), (14, 4)]:
        flower(c, x, zz, 3, PETALS[x % len(PETALS)])
    return c


def honey_jars():
    c = Canvas("Honey jars", seed=24)
    W = D = 30
    grass_base(c, W, D)
    table(c, 2, 6, 27, 23, top_y=8)
    top = 10
    # honey barrel with a spout pouring into the middle jar
    c.cyl((6, top, 14), 3.6, 9, WOOD_L)
    for y in (top + 1, top + 4, top + 7):
        c.cyl((6, y, 14), 3.8, 1, GREY_D)
    c.box((9, top + 7, 14), (12, top + 7, 14), GREY_D)
    c.line((12, top + 7, 14), (14, top + 4, 14), 0.5, HONEY)
    for x, h in [(14, 6), (19, 5), (24, 6)]:
        fill = h - 1 if x != 14 else 3
        c.cyl((x, top, 14), 2.4, h, "#e6f6ff", hollow=True)
        c.cyl((x, top, 14), 2.4, fill, HONEY)
        c.paint(lambda px, py, pz, col, x=x: "#fff6d8" if col == HONEY and px == x - 2 and py % 2 == 1 else None)  # glass shine
        if x != 14:
            c.cyl((x, top + h, 14), 2.7, 1, RED)
            c.paint(lambda px, py, pz, col, x=x, y=top + h: WHITE if col == RED and py == y and (px + pz) % 2 == 0 else None)
    c.line((22, top, 19), (26, top + 2, 21), 0.5, WOOD_L)  # honey dipper
    c.cyl((21, top, 19), 1.2, 3, HONEY, axis="x")
    return c


def honey_product():
    c = Canvas("Honey", seed=25)
    c.cyl((16, 0, 16), 15, 1, GRASS)
    c.ellipsoid((16, 12, 16), (11, 11, 11), POT)
    c.cyl((16, 1, 16), 7, 1, POT_D)
    c.cyl((16, 21, 16), 7, 3, POT)
    c.cyl((16, 24, 16), 8, 2, POT_D)
    c.cyl((16, 20, 16), 7.6, 1, "#e7d7b0")  # rope
    c.box((14, 8, 26), (18, 13, 27), "#fff3d6")  # label
    c.box((15, 10, 28), (17, 11, 28), HONEY_D)
    c.ellipsoid((16, 26, 16), (7, 2.6, 7), HONEY)
    for a in range(15, 360, 45):
        x, z = 16 + math.cos(math.radians(a)) * 7.6, 16 + math.sin(math.radians(a)) * 7.6
        c.line((x, 25, z), (x, 25 - 3 - (a // 45) % 4 * 2, z), 0.7, HONEY)
    c.line((20, 27, 17), (26, 36, 20), 0.6, WOOD_L)  # dipper
    c.cyl((19, 27, 17), 1.6, 3, HONEY_D, axis="x")
    bee(c, 9, 30, 14, 1)
    for x, z in [(4, 8), (27, 9), (5, 25), (28, 24)]:
        flower(c, x, z, 3, PETALS[(x + z) % len(PETALS)])
    return c


def actor(skin, path, action, face=None, hat=None, outfit=None, item=None, start=0.0, y=1):
    """A character in a recipe step (see src/actor.rs): walks `path` ([x, z] in build voxels), then acts."""
    a = {"skin": skin, "path": path, "action": action, "start": start, "y": y}
    for k, v in (("face", face), ("hat", hat), ("outfit", outfit), ("item", item)):
        if v is not None:
            a[k] = v
    return a


EPISODES = {
    "pizza": {
        "recipe": {
            "title": "Pizza",
            "hook": "How is pizza made?",
            "theme": "day",
            "backdrop": "backdrop.json",
            "ambience": ["birds", "wind"],
            "step_secs": 2.2,
            "step_hold": 2.0,
            "product_secs": 3.0,
            "product_hold": 2.5,
            "steps": [
                {"build": "1.json", "caption": "Grow the tomatoes", "actors": [
                    actor("alex", [[3, 14], [11, 14]], "water", face=[7, 19], hat="straw", outfit="farmer", item="watering_can"),
                    actor("steve", [[27, 15], [19, 15]], "pick", face=[23, 9], hat="straw", outfit="farmer", item="tomato", start=0.5),
                ]},
                {"build": "2.json", "caption": "Mix the dough", "actors": [
                    actor("steve", [[28, 29], [10, 27]], "stir", face=[10, 18], hat="chef", outfit="chef", item="spoon"),
                ]},
                {"build": "3.json", "caption": "Add sauce and cheese", "actors": [
                    actor("steve", [[1, 29], [12, 27]], "knead", face=[13, 15], hat="chef", outfit="chef"),
                    actor("alex", [[29, 29], [23, 27]], "chop", face=[23, 20], hat="chef", outfit="chef", start=0.4),
                ]},
                {"build": "4.json", "caption": "Bake it in the oven", "actors": [
                    actor("steve", [[1, 31], [7, 28]], "wave", face=[15, 22], hat="chef", outfit="chef"),
                    actor("alex", [[31, 18], [23, 29]], "carry", face=[16, 24], hat="chef", outfit="chef", item="pizza", start=0.4),
                ]},
            ],
            "product": {"build": "product.json", "caption": "Pizza is ready!", "actors": [
                actor("steve", [[0, 30], [5, 33]], "cheer", face=[20, 20], hat="chef", outfit="chef", y=0),
                actor("alex", [[40, 30], [35, 33]], "cheer", face=[20, 20], hat="chef", outfit="chef", y=0, start=0.3),
            ]},
        },
        "builds": {"1": pizza_garden, "2": dough_table, "3": topping_table, "4": oven, "product": pizza_product, "backdrop": scenery.village},
    },
    "ice-cream": {
        "recipe": {
            "title": "Ice cream",
            "hook": "How is ice cream made?",
            "theme": "day",
            "backdrop": "backdrop.json",
            "ambience": ["birds", "wind"],
            "step_secs": 2.2,
            "step_hold": 2.0,
            "product_secs": 3.0,
            "product_hold": 2.5,
            "steps": [
                {"build": "1.json", "caption": "Milk the cow", "actors": [
                    actor("steve", [[2, 27], [12, 26]], "pick", face=[12, 15], hat="straw", outfit="farmer"),
                    actor("alex", [[28, 28], [19, 28]], "carry", face=[12, 15], hat="straw", outfit="farmer", item="bucket", start=0.4),
                ]},
                {"build": "2.json", "caption": "Pick the strawberries", "actors": [
                    actor("alex", [[1, 19], [9, 18]], "pick", face=[9, 14], hat="straw", outfit="farmer", item="strawberry"),
                    actor("steve", [[29, 19], [21, 18]], "pick", face=[21, 14], hat="straw", outfit="farmer", item="strawberry", start=0.4),
                ]},
                {"build": "3.json", "caption": "Mix it and freeze it", "actors": [
                    actor("steve", [[29, 29], [23, 25]], "knead", face=[17, 18], hat="chef", outfit="chef"),
                    actor("alex", [[0, 29], [6, 24]], "carry", face=[12, 15], hat="chef", outfit="chef", item="strawberry", start=0.3),
                ]},
                {"build": "4.json", "caption": "Scoop it up", "actors": [
                    actor("steve", [[1, 29], [9, 27]], "stir", face=[9, 15], hat="chef", outfit="chef", item="scoop"),
                    actor("alex", [[29, 29], [21, 27]], "wave", face=[21, 15], start=0.5),
                ]},
            ],
            "product": {"build": "product.json", "caption": "Ice cream time!", "actors": [
                actor("alex", [[0, 26], [5, 22]], "eat", face=[15, 15], item="ice_cream", y=0),
                actor("steve", [[30, 26], [25, 22]], "cheer", face=[15, 15], hat="chef", outfit="chef", y=0, start=0.3),
            ]},
        },
        "builds": {"1": cow_field, "2": strawberry_patch, "3": ice_machine, "4": scoop_counter, "product": ice_cream_product, "backdrop": scenery.farm},
    },
    "honey": {
        "recipe": {
            "title": "Honey",
            "hook": "How is honey made?",
            "theme": "day",
            "backdrop": "backdrop.json",
            "ambience": ["birds", "bees", "wind"],
            "step_secs": 2.2,
            "step_hold": 2.0,
            "product_secs": 3.0,
            "product_hold": 2.5,
            "steps": [
                {"build": "1.json", "caption": "Flowers bloom", "actors": [
                    actor("steve", [[1, 17], [6, 16]], "water", face=[8, 9], hat="bee", outfit="beekeeper", item="watering_can"),
                    actor("alex", [[29, 28], [22, 24]], "pick", face=[20, 18], item="flower", start=0.4),
                ]},
                {"build": "2.json", "caption": "Bees bring the nectar home", "actors": [
                    actor("steve", [[2, 29], [9, 24]], "pick", face=[9, 14], hat="bee", outfit="beekeeper"),
                ]},
                {"build": "3.json", "caption": "They fill the honeycomb", "actors": [
                    actor("steve", [[1, 21], [8, 17]], "knead", face=[8, 10], hat="bee", outfit="beekeeper"),
                    actor("alex", [[29, 21], [21, 17]], "pick", face=[21, 10], hat="bee", outfit="beekeeper", start=0.4),
                ]},
                {"build": "4.json", "caption": "Pour it into jars", "actors": [
                    actor("steve", [[1, 29], [12, 26]], "water", face=[14, 14], hat="bee", outfit="beekeeper", item="honey_jar"),
                    actor("alex", [[29, 29], [23, 26]], "carry", face=[24, 14], item="honey_jar", start=0.4),
                ]},
            ],
            "product": {"build": "product.json", "caption": "Sweet honey!", "actors": [
                actor("alex", [[2, 28], [7, 25]], "eat", face=[16, 16], item="honey_jar"),
                actor("steve", [[30, 28], [25, 25]], "cheer", face=[16, 16], hat="bee", outfit="beekeeper", start=0.3),
            ]},
        },
        "builds": {"1": flower_meadow, "2": bee_hive, "3": honeycomb, "4": honey_jars, "product": honey_product, "backdrop": scenery.meadow},
    },
}


def main():
    names = sys.argv[1:] or list(EPISODES)
    for name in names:
        ep = EPISODES[name]
        out = ROOT / name
        for file, fn in ep["builds"].items():
            fn().dump(out / f"{file}.json")
        (out / "recipe.json").write_text(json.dumps(ep["recipe"], indent=2) + "\n")
        print(f"wrote {out / 'recipe.json'}")


if __name__ == "__main__":
    main()
