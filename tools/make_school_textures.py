"""Tiles for the school's toilets, sinks and wet areas (tools/blender/build_school.py):

    python tools/make_school_textures.py

-> assets/cache/school/wall_tile_{base,normal,orm}.png  (1024 x 1024 = 1 m): 100 mm glossy white
   ceramic tiles in a stack bond, pale grey grout lines 2.5 mm wide, each tile a touch off-white
   and its glaze faintly wavy.
-> assets/cache/school/floor_tile_{base,normal,orm}.png (1024 x 1024 = 1 m): 200 mm unglazed grey
   anti-slip floor tiles, darker grout, a fine speckle.
"""

import os

import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "cache", "school")
os.makedirs(OUT, exist_ok=True)
N = 1024
yy, xx = np.mgrid[0:N, 0:N].astype(np.float32)


def tiles(name, pitch_px, grout_px, colour, grout_col, vary, gloss, speckle, seed):
    rng = np.random.default_rng(seed)
    yy, xx = np.mgrid[0:N, 0:N].astype(np.float32)   # (its own grid: the gym's code reuses the names)
    n = int(round(N / pitch_px))
    ix = (xx / pitch_px).astype(int) % n
    iy = (yy / pitch_px).astype(int) % n
    fx = (xx % pitch_px)
    fy = (yy % pitch_px)
    edge = np.minimum(np.minimum(fx, pitch_px - fx), np.minimum(fy, pitch_px - fy))
    grout = edge < grout_px / 2
    shade = rng.normal(0, vary, (n, n)).astype(np.float32)[iy, ix]
    base = np.array(colour, np.float32)[None, None, :] * (1 + shade[..., None])
    if speckle:
        base *= (1 + rng.normal(0, speckle, (N, N, 1)).astype(np.float32))
    base = np.where(grout[..., None], np.array(grout_col, np.float32)[None, None, :], base)
    Image.fromarray((np.power(np.clip(base, 0, 1), 1 / 2.2) * 255).astype(np.uint8)).save(os.path.join(OUT, name + "_base.png"))
    # height: tiles flat with rounded edges, the grout sunk; a faint wave in each glaze
    h = np.clip(edge / (grout_px * 1.5 + 1e-3), 0, 1)
    h = h * h * (3 - 2 * h)
    tx = rng.normal(0, 1, (n, n)).astype(np.float32)[iy, ix]
    h = h + 0.03 * np.sin(fx / pitch_px * 6.28 + tx) * (~grout)
    dy = np.roll(h, -1, 0) - np.roll(h, 1, 0)
    dx = np.roll(h, -1, 1) - np.roll(h, 1, 1)
    nrm = np.stack([-dx * 2.0, dy * 2.0, np.ones_like(h)], -1)
    nrm /= np.linalg.norm(nrm, axis=-1, keepdims=True)
    Image.fromarray(((nrm * 0.5 + 0.5) * 255).astype(np.uint8)).save(os.path.join(OUT, name + "_normal.png"))
    rough = np.where(grout, 0.9, gloss).astype(np.float32)
    ao = np.where(grout, 0.75, 1.0).astype(np.float32)
    orm = np.stack([ao, rough, np.zeros_like(ao)], -1)
    Image.fromarray((orm * 255).astype(np.uint8)).save(os.path.join(OUT, name + "_orm.png"))


tiles("wall_tile", N / 10, 3, (0.86, 0.87, 0.85), (0.55, 0.56, 0.55), 0.015, 0.12, 0.0, 1)
tiles("floor_tile", N / 5, 6, (0.36, 0.37, 0.37), (0.20, 0.20, 0.20), 0.04, 0.75, 0.06, 2)
print("wrote", OUT)


# ---- the gym's floor: maple boards with the courts' lines, for the arena (tools/blender/
# build_school_gym.py): FX x FY metres, 4096 px across (1 px = 0.8 cm)
FX, FY = 32.75, 27.75
GW = 4096
GH = int(GW * FY / FX)
S = GW / FX
gy, gx = np.mgrid[0:GH, 0:GW].astype(np.float32)
rng = np.random.default_rng(9)
# boards 7.5 cm wide along x, 1.8 m long, staggered; each board its own shade, a grain along it
bw = 0.075 * S
row = (gy / bw).astype(int)
off = (row * 0.37 % 1.0) * 1.8 * S
col = ((gx + off) / (1.8 * S)).astype(int)
shade = rng.normal(0, 0.05, (row.max() + 2, col.max() + 2)).astype(np.float32)[row, col]
grain = np.sin(gx / S * 40 + np.sin(gy / S * 3) * 2) * 0.015
base = np.stack([np.full_like(gx, 0.78), np.full_like(gx, 0.58), np.full_like(gx, 0.36)], -1) * (1 + shade + grain)[..., None]
seam = ((gy % bw) < 1.2) | (((gx + off) % (1.8 * S)) < 1.2)
base[seam] *= 0.8
img = Image.fromarray((np.power(np.clip(base, 0, 1), 1 / 2.2) * 255).astype(np.uint8))
from PIL import ImageDraw  # noqa: E402
d = ImageDraw.Draw(img)
M = lambda x, y: ((x + 19.75) * S, (FY - (y + 12.0)) * S)      # arena metres (gym frame) -> px


def line(pts, col, w=0.05):
    d.line([M(*p) for p in pts], fill=col, width=max(2, int(w * S)))


def circle(cx, cy, r, col, a0=0, a1=360, w=0.05):
    pts = [(cx + r * np.cos(np.radians(a)), cy + r * np.sin(np.radians(a))) for a in np.linspace(a0, a1, 64)]
    line(pts, col, w)


def rect(x0, y0, x1, y1, col, w=0.05):
    line([(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)], col, w)


WHITE, YELLOW, GREEN, RED = (245, 245, 240), (235, 200, 40), (40, 150, 70), (200, 50, 45)
# basketball (white): 28 x 15, along x, centred at (-3.4, 1.9)
bx, by = -3.4, 1.9
rect(bx - 14, by - 7.5, bx + 14, by + 7.5, WHITE)
line([(bx, by - 7.5), (bx, by + 7.5)], WHITE)
circle(bx, by, 1.8, WHITE)
for s in (-1, 1):
    ex = bx + s * 14
    hx = ex - s * 1.575
    rect(min(ex, ex - s * 5.8), by - 2.45, max(ex, ex - s * 5.8), by + 2.45, WHITE)
    circle(ex - s * 5.8, by, 1.8, WHITE)
    a = np.degrees(np.arcsin(6.6 / 6.75))
    if s > 0:
        circle(hx, by, 6.75, WHITE, 180 - a, 180 + a)
    else:
        circle(hx, by, 6.75, WHITE, -a, a)
    for yy in (by - 6.6, by + 6.6):
        line([(ex, yy), (hx - s * 6.75 * np.cos(np.radians(a)), yy)], WHITE)
# volleyball (yellow): two courts of 18 x 9 across the hall
for vx in (-11.0, 4.2):
    rect(vx - 4.5, by - 9, vx + 4.5, by + 9, YELLOW)
    line([(vx - 4.5, by), (vx + 4.5, by)], YELLOW)
    for yy in (by - 3, by + 3):
        line([(vx - 4.5, yy), (vx + 4.5, yy)], YELLOW)
# badminton (green): four courts of 13.4 x 6.1 across the hall, their nets' lines and short
# service lines
for cx in (-15.0, -7.5, 0.5, 8.0):
    rect(cx - 3.05, by - 6.7, cx + 3.05, by + 6.7, GREEN, w=0.04)
    line([(cx - 3.05, by), (cx + 3.05, by)], GREEN, w=0.04)
    for yy in (by - 1.98, by + 1.98):
        line([(cx - 3.05, yy), (cx + 3.05, yy)], GREEN, w=0.04)
img.save(os.path.join(OUT, "gym_floor_base.png"))
# its normal (the board seams) and roughness (a satin lacquer)
h = np.where(seam, 0.0, 1.0).astype(np.float32)
dy = np.roll(h, -1, 0) - np.roll(h, 1, 0)
dx = np.roll(h, -1, 1) - np.roll(h, 1, 1)
nrm = np.stack([-dx * 0.5, dy * 0.5, np.ones_like(h)], -1)
nrm /= np.linalg.norm(nrm, axis=-1, keepdims=True)
Image.fromarray(((nrm * 0.5 + 0.5) * 255).astype(np.uint8)).save(os.path.join(OUT, "gym_floor_normal.png"))
orm = np.stack([np.where(seam, 0.8, 1.0), np.where(seam, 0.6, 0.12), np.zeros_like(h)], -1)   # (a high-gloss lacquer)
Image.fromarray((orm * 255).astype(np.uint8)).save(os.path.join(OUT, "gym_floor_orm.png"))
print("wrote the gym floor")


# ---- the pool's tiles: 100 mm pale blue glazed tiles (the tank), cream deck tiles
tiles("pool_tile", N / 10, 3, (0.55, 0.78, 0.86), (0.75, 0.82, 0.84), 0.02, 0.1, 0.0, 3)
tiles("deck_tile", N / 4, 5, (0.72, 0.70, 0.64), (0.45, 0.44, 0.42), 0.03, 0.7, 0.05, 4)
print("wrote the pool tiles")


# ---- the gym's birch plywood wall panels (2048 px = 3.6 m: sheets 0.9 x 1.8 m standing up,
# each sheet its own shade and grain, 3 mm joints), and its ceiling boards (2048 px = 2 m: cedar
# boards 15 cm wide along u, joints, knots)
def plywood():
    n = 2048
    s = n / 3.6
    y, x = np.mgrid[0:n, 0:n].astype(np.float32)
    r = np.random.default_rng(21)
    col = (x / (0.9 * s)).astype(int)
    row = (y / (1.8 * s)).astype(int)
    shade = r.normal(0, 0.012, (row.max() + 2, col.max() + 2)).astype(np.float32)[row, col]   # (sheets from one batch: nearly the same tone)
    phase = r.uniform(0, 6.28, (row.max() + 2, col.max() + 2)).astype(np.float32)[row, col]
    grain = 0.025 * np.sin(x / s * 140 + np.sin(y / s * 1.5 + phase) * 6 + phase) + 0.012 * np.sin(x / s * 37 + phase)
    base = np.array([0.80, 0.64, 0.44], np.float32)[None, None, :] * (1 + shade + grain)[..., None]
    joint = ((x % (0.9 * s)) < 1.5) | ((y % (1.8 * s)) < 1.5)
    base[joint] *= 0.75
    Image.fromarray((np.power(np.clip(base, 0, 1), 1 / 2.2) * 255).astype(np.uint8)).save(os.path.join(OUT, "gym_plywood_base.png"))
    h = np.where(joint, 0.0, 1.0).astype(np.float32) + grain * 0.5
    dy = np.roll(h, -1, 0) - np.roll(h, 1, 0)
    dx = np.roll(h, -1, 1) - np.roll(h, 1, 1)
    nrm = np.stack([-dx * 1.2, dy * 1.2, np.ones_like(h)], -1)
    nrm /= np.linalg.norm(nrm, axis=-1, keepdims=True)
    Image.fromarray(((nrm * 0.5 + 0.5) * 255).astype(np.uint8)).save(os.path.join(OUT, "gym_plywood_normal.png"))
    # (a clear lacquer over the birch: a soft sheen, glossier along the grain's lines; the joints
    # matte)
    orm = np.stack([np.where(joint, 0.7, 1.0), np.where(joint, 0.8, 0.26 + 0.6 * np.abs(grain)), np.zeros_like(h)], -1)
    Image.fromarray((np.clip(orm, 0, 1) * 255).astype(np.uint8)).save(os.path.join(OUT, "gym_plywood_orm.png"))


def ceiling_boards():
    n = 2048
    s = n / 2.0
    y, x = np.mgrid[0:n, 0:n].astype(np.float32)
    r = np.random.default_rng(22)
    row = (y / (0.15 * s)).astype(int)
    off = (row * 0.61 % 1.0) * 2.0 * s
    col = ((x + off) / (2.0 * s)).astype(int)
    shade = r.normal(0, 0.025, (row.max() + 2, col.max() + 2)).astype(np.float32)[row, col]
    grain = 0.03 * np.sin(y / s * 150 + np.sin(x / s * 3 + row) * 3)
    base = np.array([0.74, 0.50, 0.30], np.float32)[None, None, :] * (1 + shade + grain)[..., None]
    joint = ((y % (0.15 * s)) < 3) | (((x + off) % (2.0 * s)) < 3)
    base[joint] *= 0.5
    # a few knots
    for _ in range(60):
        cx, cy, rr = r.uniform(0, n), r.uniform(0, n), r.uniform(4, 10)
        d = np.hypot(x - cx, (y - cy) * 2)
        base *= (1 - 0.35 * np.exp(-(d / rr) ** 2))[..., None]
    Image.fromarray((np.power(np.clip(base, 0, 1), 1 / 2.2) * 255).astype(np.uint8)).save(os.path.join(OUT, "gym_ceiling_base.png"))
    h = np.where(joint, 0.0, 1.0).astype(np.float32)
    dy = np.roll(h, -1, 0) - np.roll(h, 1, 0)
    dx = np.roll(h, -1, 1) - np.roll(h, 1, 1)
    nrm = np.stack([-dx * 1.5, dy * 1.5, np.ones_like(h)], -1)
    nrm /= np.linalg.norm(nrm, axis=-1, keepdims=True)
    Image.fromarray(((nrm * 0.5 + 0.5) * 255).astype(np.uint8)).save(os.path.join(OUT, "gym_ceiling_normal.png"))
    orm = np.stack([np.where(joint, 0.6, 1.0), np.full_like(h, 0.6), np.zeros_like(h)], -1)
    Image.fromarray((orm * 255).astype(np.uint8)).save(os.path.join(OUT, "gym_ceiling_orm.png"))


plywood()
ceiling_boards()
print("wrote the gym's plywood and ceiling boards")
