"""Fab (Quixel Megascans) surfaces made ready for the school's builders: square, tiling maps with
the finish each place needs baked in (glTF has no maths on textures).

    python tools/prep_fab_textures.py

Reads D:/EpicVault/VaultCache/FabLibrary (glTF Medium downloads), writes
assets/cache/school/<stem>_{base,normal,orm}.png:
- fab_plywood: Plywood Panels (2 x 1 m scan) stacked to 2 x 2 m; its roughness lowered for a
  clear lacquer (the gym's walls).
- fab_gym_floor: Wooden Floor (2 x 2 m), its roughness lowered for the gym's high-gloss lacquer,
  warmed a little towards maple.
- fab_corrugated: Corrugated Metal Sheet (2 x 1 m) stacked to 2 x 2 m, tinted the gym's grey-green
  (the builder turns its UVs so the ribs stand upright).
- fab_white_tile: Clean White Tiles as they are (the school's toilets and sinks).
- fab_blue_tile: the same tiles tinted pale blue (the pool's tank).
- fab_pool_deck: Concrete Floor (2 x 2 m, smooth, stained, worn), its colour taken to the faded
  green of a school pool deck's non-slip coat (its stains and wear kept, its contrast eased).
- fab_pool_coping: the same concrete, a light weathered grey (the pool's edge).
"""

import glob
import os

import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
FAB = "D:/EpicVault/VaultCache/FabLibrary"
OUT = os.path.join(ROOT, "assets", "cache", "school")
os.makedirs(OUT, exist_ok=True)


def maps(prefix):
    d = sorted(glob.glob(os.path.join(FAB, prefix + "*")))[0]
    one = lambda pat: glob.glob(os.path.join(d, "**", pat), recursive=True)[0]
    return one("*_B.png"), one("*_N.png"), one("*_ORM.png")


def load(p):
    return np.asarray(Image.open(p).convert("RGB")).astype(np.float32) / 255.0


def square(a):
    """A 2:1 scan (wider than tall) stacked into a square."""
    h, w, _ = a.shape
    if w == 2 * h:
        return np.concatenate([a, a], axis=0)
    return a


def save(a, path, gamma=False):
    Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8)).save(path)


def shrink(a, size=2048):
    if a.shape[0] <= size:
        return a
    return np.asarray(Image.fromarray((a * 255).astype(np.uint8)).resize((size, size), Image.LANCZOS)).astype(np.float32) / 255.0


def make(stem, prefix, tint=(1.0, 1.0, 1.0), rough=1.0, desaturate=0.0, mean=None, contrast=1.0, rough_add=0.0):
    """`mean`: the linear colour the base should average (its variation kept, scaled by
    `contrast`); otherwise `tint` multiplies it."""
    b, n, o = (shrink(square(load(p))) for p in maps(prefix))
    # (the base is sRGB: tint in linear light, after taking out `desaturate` of its colour)
    lin = np.power(b, 2.2)
    if desaturate:
        grey = lin @ np.array([0.2126, 0.7152, 0.0722], np.float32)
        lin = lin + (grey[..., None] - lin) * desaturate
    if mean is not None:
        m = lin.reshape(-1, 3).mean(0)
        lin = np.array(mean, np.float32) * (1.0 + (lin / np.maximum(m, 1e-4) - 1.0) * contrast)
    else:
        lin = lin * np.array(tint, np.float32)
    save(np.power(np.clip(lin, 0, 1), 1 / 2.2), os.path.join(OUT, stem + "_base.png"))
    save(n, os.path.join(OUT, stem + "_normal.png"))
    o = o.copy()
    o[..., 1] = np.clip(o[..., 1] * rough + rough_add, 0.04, 1.0)
    save(o, os.path.join(OUT, stem + "_orm.png"))
    print("wrote", stem, b.shape[1], "x", b.shape[0])


make("fab_plywood", "Plywood_Panels", tint=(2.05, 1.85, 1.5), rough=0.55, desaturate=0.5)   # (paler: birch)
make("fab_gym_floor", "Wooden_Floor", tint=(1.08, 1.0, 0.86), rough=0.3)
make("fab_corrugated", "Corrugated_Metal_Sheet", tint=(1.1, 1.28, 1.2))
make("fab_white_tile", "Clean_White_Tiles")
make("fab_blue_tile", "Clean_White_Tiles", tint=(0.62, 0.86, 0.95), rough=0.8)
make("fab_pool_deck", "Concrete_Floor", desaturate=1.0, mean=(0.085, 0.18, 0.115), contrast=0.7, rough_add=0.15)
make("fab_pool_coping", "Concrete_Floor", desaturate=1.0, mean=(0.47, 0.47, 0.45), contrast=0.6, rough_add=0.1)
