"""Textures for the inside of a Japanese house (Tsukumodō's home: tools/blender/build_shop_tsukumodo.py).

    python tools/make_house_textures.py

-> assets/cache/house/tatami_{base,normal,orm}.png   (1024 x 2048: one mat, its long side up the
   image). The face of a tatami (the omote): rush stems (igusa) woven across the mat's width,
   about 3.5 mm apart, held by warp threads running its length about 18 mm apart. Colour: rush a
   few years old, gone from green to a soft straw gold, each stem a little different; ridged
   stems in the normal map, the warp sunk between them. Roughness ~0.7 (rush has a faint sheen).
-> assets/cache/house/<fabric>_{base,normal,orm}.png  (1024 x 1024, tiling, 0.5 m a tile): the
   household's cotton, a plain weave (~2 mm threads, slubs, the dye a little uneven):
   futon_cover   your quilt's cover: indigo with a paler blue tattersall check (3 cm)
   futon_sheet   the mattress's sheet: unbleached white
   pillow        the buckwheat pillow's cover: blue and white ticking stripes
   curtain       your room's curtains: a light sage cotton
"""

import os

import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "cache", "house")
os.makedirs(OUT, exist_ok=True)
rng = np.random.default_rng(5)

W, H = 1024, 2048
STEM = 4.0          # px per stem (1.76 m / 2048 px * 4 = 3.4 mm)
WARP = 21.0         # px between warp threads (0.88 m / 1024 * 21 = 18 mm)

y = np.arange(H)[:, None].astype(np.float32)
x = np.arange(W)[None, :].astype(np.float32)
# each stem: its own shade (some greener, some paler), varying slowly along its length
n_stems = int(H / STEM) + 2
shade = rng.normal(0.0, 1.0, n_stems).astype(np.float32)
along = rng.normal(0.0, 1.0, (n_stems, 16)).astype(np.float32)
idx = (y / STEM).astype(int)
phase = (y / STEM) % 1.0
seg = np.clip((x / W * 15).astype(int), 0, 14)
t = (x / W * 15) % 1.0
var = along[idx, seg] * (1 - t) + along[idx, seg + 1] * t
s = shade[idx] * 0.9 + var * 0.5
# the stem's round section: brightest along its middle, darker in the grooves between stems
ridge = np.sin(np.pi * phase)
# the warp threads: narrow dips every WARP px (a stem passes over and under them)
wphase = (x % WARP) / WARP
warp = np.exp(-((wphase - 0.5) / 0.04) ** 2)

gold = np.array([0.62, 0.56, 0.33], np.float32)
green = np.array([0.50, 0.52, 0.30], np.float32)
pale = np.array([0.70, 0.64, 0.42], np.float32)
mix = np.where(s[..., None] < 0, gold + (green - gold) * np.clip(-s, 0, 1)[..., None] * 0.6, gold + (pale - gold) * np.clip(s, 0, 1)[..., None] * 0.6)
light = (0.78 + 0.22 * ridge)[..., None] * (1.0 - 0.08 * warp)[..., None]
base = np.clip(mix * light, 0, 1)
# a little wear: faint broad blotches (feet, sun)
blot = rng.normal(0, 1, (H // 128 + 2, W // 128 + 2)).astype(np.float32)
by, bx = (y / 128), (x / 128)
iy, ix = by.astype(int), bx.astype(int)
fy, fx = by - iy, bx - ix
b = blot[iy, ix] * (1 - fy) * (1 - fx) + blot[iy + 1, ix] * fy * (1 - fx) + blot[iy, ix + 1] * (1 - fy) * fx + blot[iy + 1, ix + 1] * fy * fx
base = np.clip(base * (1 + 0.05 * b)[..., None], 0, 1)
Image.fromarray((np.power(base, 1 / 2.2) * 255).astype(np.uint8)).save(os.path.join(OUT, "tatami_base.png"))

# height: the stems' ridges, the warp sunk; normals from its slopes (+Y up the image)
h = ridge * 1.0 - warp * 0.35
dy = np.roll(h, -1, 0) - np.roll(h, 1, 0)
dx = np.roll(h, -1, 1) - np.roll(h, 1, 1)
k = 0.9
n = np.stack([-dx * k, dy * k, np.ones_like(h)], -1)
n /= np.linalg.norm(n, axis=-1, keepdims=True)
Image.fromarray(((n * 0.5 + 0.5) * 255).astype(np.uint8)).save(os.path.join(OUT, "tatami_normal.png"))

ao = 0.75 + 0.25 * ridge * (1 - 0.5 * warp)
rough = 0.62 + 0.12 * (1 - ridge) + 0.05 * warp
orm = np.stack([ao, rough, np.zeros_like(ao)], -1)
Image.fromarray((np.clip(orm, 0, 1) * 255).astype(np.uint8)).save(os.path.join(OUT, "tatami_orm.png"))
print("wrote", OUT)


# ----------------------------------------------------------------------------- cotton
N = 1024
yy, xx = np.mgrid[0:N, 0:N].astype(np.float32)
PITCH = 4.0                     # px per thread (0.5 m / 1024 * 4 = 2 mm)


def smooth_noise(cells, amp, seed):
    """Tiling value noise: `cells` across the tile."""
    g = np.random.default_rng(seed).normal(0, 1, (cells, cells)).astype(np.float32)
    fy, fx = yy / N * cells, xx / N * cells
    iy, ix = fy.astype(int) % cells, fx.astype(int) % cells
    ty, tx = fy - np.floor(fy), fx - np.floor(fx)
    ty, tx = ty * ty * (3 - 2 * ty), tx * tx * (3 - 2 * tx)
    a, b = g[iy, ix], g[iy, (ix + 1) % cells]
    c, d = g[(iy + 1) % cells, ix], g[(iy + 1) % cells, (ix + 1) % cells]
    return amp * ((a * (1 - tx) + b * tx) * (1 - ty) + (c * (1 - tx) + d * tx) * ty)


def cotton(name, colour, seed):
    """A plain weave in `colour` (an N x N x 3 linear array): over-under threads, slubs."""
    r = np.random.default_rng(seed)
    wx = (xx / PITCH) % 1.0
    wy = (yy / PITCH) % 1.0
    cx = (xx / PITCH).astype(int)
    cy = (yy / PITCH).astype(int)
    over = ((cx + cy) % 2 == 0)
    # each thread's own thickness (slubs along it), as a height across its width
    slub_w = r.normal(0, 1, N // int(PITCH) + 1).astype(np.float32)[cx % (N // int(PITCH))]
    slub_h = r.normal(0, 1, N // int(PITCH) + 1).astype(np.float32)[cy % (N // int(PITCH))]
    warp = np.sin(np.pi * wx) * (1 + 0.15 * slub_w)
    weft = np.sin(np.pi * wy) * (1 + 0.15 * slub_h)
    h = np.where(over, warp * 0.8 + weft * 0.4, weft * 0.8 + warp * 0.4)
    h += smooth_noise(16, 0.25, seed + 1)      # soft unevenness (a used fabric)
    dye = 1.0 + smooth_noise(8, 0.05, seed + 2) + 0.06 * (h - 0.6)
    base = np.clip(colour * dye[..., None], 0, 1)
    Image.fromarray((np.power(base, 1 / 2.2) * 255).astype(np.uint8)).save(os.path.join(OUT, name + "_base.png"))
    dy = np.roll(h, -1, 0) - np.roll(h, 1, 0)
    dx = np.roll(h, -1, 1) - np.roll(h, 1, 1)
    n = np.stack([-dx * 0.6, dy * 0.6, np.ones_like(h)], -1)
    n /= np.linalg.norm(n, axis=-1, keepdims=True)
    Image.fromarray(((n * 0.5 + 0.5) * 255).astype(np.uint8)).save(os.path.join(OUT, name + "_normal.png"))
    orm = np.stack([0.8 + 0.2 * np.clip(h, 0, 1), 0.85 + 0.08 * (1 - np.clip(h, 0, 1)), np.zeros_like(h)], -1)
    Image.fromarray((np.clip(orm, 0, 1) * 255).astype(np.uint8)).save(os.path.join(OUT, name + "_orm.png"))


def flat(c):
    return np.broadcast_to(np.array(c, np.float32), (N, N, 3)).copy()


# your quilt: indigo, a tattersall of two paler blues (lines every 3 cm, alternating)
cov = flat((0.025, 0.045, 0.12))
step = 64                       # px: 3.1 cm
for k, col in ((0, (0.10, 0.17, 0.32)), (32, (0.05, 0.09, 0.20))):
    lines = (((xx + k) % step) < 3) | (((yy + k) % step) < 3)
    cov[lines] = col
cotton("futon_cover", cov, 11)
cotton("futon_sheet", flat((0.78, 0.76, 0.70)), 12)
# the pillow's ticking: blue stripes of two widths on white, along the tile's v
tick = flat((0.80, 0.80, 0.78))
band = xx % 128
tick[(band < 14) | ((band > 30) & (band < 36))] = (0.05, 0.10, 0.28)
cotton("pillow", tick, 13)
cotton("curtain", flat((0.42, 0.50, 0.38)), 14)
print("wrote cotton", OUT)
