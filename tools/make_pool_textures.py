"""The pool's painted surfaces, made large so they never show a repeat: one tile is 8 x 8 m.

    python tools/make_pool_textures.py

-> assets/cache/school/pool_paint_{base,normal,orm}.png   (the tank: smooth sky-blue pool paint,
   faint roller marks and the slow mottling of repainting, gloss a little uneven)
-> assets/cache/school/deck_paint_{base,normal,orm}.png   (the deck: green non-slip paint, its
   grit, sun-faded and foot-worn patches)

The noise is made in the frequency domain, so every map tiles seamlessly.
"""

import os

import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "cache", "school")
os.makedirs(OUT, exist_ok=True)
N = 2048            # px a tile
METRES = 8.0        # m a tile (4 mm a pixel)


def field(seed, lo_m, hi_m, aniso=1.0):
    """Tileable noise holding features between hi_m and lo_m metres across (unit variance).
    `aniso` > 1 stretches features along x (roller marks)."""
    rng = np.random.default_rng(seed)
    w = rng.standard_normal((N, N)) + 1j * rng.standard_normal((N, N))
    fy = np.fft.fftfreq(N)[:, None] * N / METRES      # cycles a metre
    fx = np.fft.fftfreq(N)[None, :] * N / METRES
    f = np.sqrt((fx * aniso) ** 2 + fy ** 2)
    band = np.exp(-(f * hi_m) ** 2) * (1.0 - np.exp(-(f * lo_m) ** 2))
    a = np.real(np.fft.ifft2(w * band))
    return (a - a.mean()) / (a.std() + 1e-9)


def normal_from(height, strength):
    gy, gx = np.gradient(height)
    n = np.stack([-gx * strength, -gy * strength, np.ones_like(height)], -1)
    return n / np.linalg.norm(n, axis=-1, keepdims=True)


def save(stem, base_lin, nrm, rough, ao=None):
    srgb = np.power(np.clip(base_lin, 0, 1), 1 / 2.2)
    Image.fromarray((srgb * 255 + 0.5).astype(np.uint8)).save(os.path.join(OUT, stem + "_base.png"))
    Image.fromarray(((nrm * 0.5 + 0.5) * 255 + 0.5).astype(np.uint8)).save(os.path.join(OUT, stem + "_normal.png"))
    orm = np.stack([np.ones_like(rough) if ao is None else ao, rough, np.zeros_like(rough)], -1)
    Image.fromarray((np.clip(orm, 0, 1) * 255 + 0.5).astype(np.uint8)).save(os.path.join(OUT, stem + "_orm.png"))
    print("wrote", stem)


def pool_paint():
    colour = np.array([0.45, 0.72, 0.94]) ** 2.2          # (the tank's sky blue, linear: faded by sun and chlorine)
    mottle = field(1, 6.0, 0.8)                             # repainting: patches a few metres across
    patch = field(2, 1.2, 0.25)                             # smaller unevenness
    roller = field(3, 0.6, 0.06, aniso=6.0)                 # roller marks, along x
    shade = 1.0 + 0.045 * mottle + 0.02 * patch + 0.012 * roller
    base = colour[None, None, :] * shade[..., None]
    # a touch greener where the paint has aged
    base[..., 1] *= 1.0 + 0.02 * np.clip(mottle, 0, None)
    height = 0.25 * roller + 0.1 * field(4, 0.05, 0.012)    # roller marks and orange peel: barely there
    nrm = normal_from(height, 0.08)
    rough = np.clip(0.42 + 0.04 * patch + 0.03 * mottle, 0.32, 0.55)
    save("pool_paint", base, nrm, rough)


def deck_paint():
    colour = np.array([0.33, 0.47, 0.38]) ** 2.2           # (the deck's green, linear: sun-faded)
    fade = field(11, 5.0, 0.7)                              # sun-faded and foot-worn patches
    wear = np.clip(field(12, 1.5, 0.2) * 0.6 + fade * 0.4, 0, None)
    grit = field(13, 0.02, 0.006)                           # the non-slip grit
    shade = 1.0 + 0.06 * fade + 0.03 * grit
    base = colour[None, None, :] * shade[..., None]
    base = base * (1.0 - 0.18 * wear[..., None]) + np.array([0.20, 0.22, 0.20]) ** 2.2 * 0.18 * wear[..., None]
    nrm = normal_from(grit * 1.0 + 0.3 * field(14, 0.15, 0.03), 0.9)
    rough = np.clip(0.82 + 0.06 * grit - 0.06 * wear, 0.6, 0.95)
    save("deck_paint", base, nrm, rough)


pool_paint()
deck_paint()
