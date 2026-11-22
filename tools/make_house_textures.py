"""Textures for the inside of a Japanese house (Tsukumodō's home: tools/blender/build_shop_tsukumodo.py).

    python tools/make_house_textures.py

-> assets/cache/house/tatami_{base,normal,orm}.png   (1024 x 2048: one mat, its long side up the
   image). The face of a tatami (the omote): rush stems (igusa) woven across the mat's width,
   about 3.5 mm apart, held by warp threads running its length about 18 mm apart. Colour: rush a
   few years old, gone from green to a soft straw gold, each stem a little different; ridged
   stems in the normal map, the warp sunk between them. Roughness ~0.7 (rush has a faint sheen).
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
