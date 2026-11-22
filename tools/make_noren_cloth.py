"""Cotton textures for Tsukumodō's noren (tools/blender/noren_mesh.py, built into the shop).

    python tools/make_noren_cloth.py

-> assets/cache/cloth/noren_base.png    (2048 x 2048, sRGB): the print on indigo-dyed cotton
   assets/cache/cloth/noren_normal.png  (2048 x 2048, tangent space, +Y up the image): weave, hems
   assets/cache/cloth/noren_orm.png     (2048 x 2048): R occlusion, G roughness, B metallic (0)

Layout (tools/blender/noren_layout.py): the two panels side by side, each from its bottom hem (the
bottom of the image) up the front, round the rod's sleeve, and down the back to the sleeve's seam
(the top rows). The print is tools/make_shop_signs.py's `noren` (つく | も), drawn at this size and
undistorted; here it becomes cloth:
- plain weave (heavy cotton, ~1.7 mm thread pitch) in the normal map, with slubs (thicker
  stretches of thread) and a little dye unevenness along the warp;
- the dye: indigo-navy, slightly mottled; the lettering resist-dyed cream, the weave showing
  through it and its edges a touch soft;
- side and bottom hems folded twice and stitched, the sleeve's seams stitched across;
- wear: the cloth by the slit (where hands push it aside) a little faded and greyed, and at
  the bottom hem.
Roughness: cotton 0.85-0.92 (matte, slight sheen lost to the weave), the lettering a little
smoother.
"""

import math
import os
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "blender"))
import make_shop_signs as signs  # noqa: E402
import noren_layout as L  # noqa: E402

OUT = os.path.join(os.path.dirname(HERE), "assets", "cache", "cloth")
N = L.TEX
rng = np.random.default_rng(11)


def smooth_noise(shape, cell, octaves=3):
    """Value noise in -1..1, features about `cell` px."""
    h, w = shape
    out = np.zeros(shape, np.float32)
    amp, tot = 1.0, 0.0
    for o in range(octaves):
        c = max(2, int(cell / 2 ** o))
        g = rng.standard_normal((h // c + 3, w // c + 3)).astype(np.float32)
        im = Image.fromarray(g, mode="F").resize((w + 3 * c, h + 3 * c), Image.BICUBIC)
        out += amp * np.asarray(im)[c:c + h, c:c + w]
        tot += amp
        amp *= 0.5
    out /= tot
    return np.clip(out / (np.abs(out).max() + 1e-6), -1, 1)


def main():
    os.makedirs(OUT, exist_ok=True)
    pu, pv = L.px_per_m()
    ys, xs = np.mgrid[0:N, 0:N].astype(np.float32)
    v = 1.0 - (ys + 0.5) / N          # Blender v (0 at the bottom of the image)
    u = (xs + 0.5) / N
    # each texel's place on its panel: panel k, metres across (from its left edge) and up (s)
    k = (u >= 0.5).astype(np.int32)
    t = (u - (k * 0.5 + L.GUTTER_U)) / (0.5 - 2 * L.GUTTER_U)
    across = t * L.PANEL_W
    s = (v - L.GUTTER_V) / (1 - 2 * L.GUTTER_V) * L.S_TOTAL
    inside = (t >= 0) & (t <= 1) & (s >= 0) & (s <= L.S_TOTAL)

    # ---------------------------------------------------------------- the weave (height field)
    pitch = 4.0  # px per thread (~1.7 mm)
    # each warp thread (vertical) and weft thread (horizontal) has its own slight thickness,
    # varying slowly along it (slubs)
    col = np.floor(xs / pitch).astype(np.int64)
    row = np.floor(ys / pitch).astype(np.int64)
    warp_thick = 0.85 + 0.3 * rng.random(N // int(pitch) + 2)[col]
    weft_thick = 0.85 + 0.3 * rng.random(N // int(pitch) + 2)[row]
    slub_v = smooth_noise((N, N // int(pitch) + 2), 40, 2)
    slub = np.take_along_axis(slub_v, np.clip(col, 0, slub_v.shape[1] - 1), axis=1)
    warp_thick = warp_thick * (1 + 0.25 * np.maximum(slub, 0) ** 2)
    ph_x = (xs / pitch) * math.pi
    ph_y = (ys / pitch) * math.pi
    over = np.sign(np.sin(ph_x) * np.sin(ph_y))           # which thread is on top here
    warp = np.abs(np.cos(ph_x)) ** 0.7 * warp_thick        # round thread profiles
    weft = np.abs(np.cos(ph_y)) ** 0.7 * weft_thick
    height = np.where(over > 0, warp + 0.35 * weft, weft + 0.35 * warp) * 0.5

    # ---------------------------------------------------------------- hems, seams, stitches
    from_side = np.minimum(across, L.PANEL_W - across)
    hem = (from_side < L.HEM) | (s < L.HEM)
    dist_hem_edge = np.minimum(np.abs(from_side - L.HEM), np.where(s < L.HEM + 0.01, np.abs(s - L.HEM), 1.0))
    # the hems' inner fold edge: a soft ridge; the hems stand a little proud
    height += 0.6 * hem + 0.5 * np.exp(-(dist_hem_edge * pu / 2.0) ** 2)
    seam_front = L.HANG - L.SEAM
    seam_back = L.S_TOTAL - 0.004
    stitch_rows = []
    stitch = np.zeros((N, N), bool)

    def stitch_line(mask_line, along_m):
        # dashed: 3 mm stitches, 1.2 mm gaps, along the line
        ph = np.mod(along_m * 1000.0, 4.2)
        return mask_line & (ph < 3.0)

    on_side = np.abs(from_side - 0.008) * pu < 1.3
    on_bottom = np.abs(s - 0.008) * pv < 1.3
    on_seam = (np.abs(s - seam_front) * pv < 1.3) | (np.abs(s - seam_back) * pv < 1.3)
    stitch |= stitch_line(on_side & (s > 0.004), s)
    stitch |= stitch_line(on_bottom & (from_side > 0.004), across)
    stitch |= stitch_line(on_seam & (from_side > 0.004), across)
    height = height - 0.9 * stitch   # stitches pull the cloth in
    del stitch_rows

    # ---------------------------------------------------------------- the dye and the print
    W = int((0.5 - 2 * L.GUTTER_U) * N) * 2
    H = int((1 - 2 * L.GUTTER_V) * N * L.HANG / L.S_TOTAL)
    pr = np.asarray(signs.noren(W, H, weathered=False).convert("RGB")).astype(np.float32) / 255.0
    # cream ink mask from the print (navy = 0, cream = 1)
    navy = np.array([30, 38, 64], np.float32) / 255.0
    cream = np.array(signs.CREAM, np.float32) / 255.0
    ink_p = np.clip(((pr - navy) * (cream - navy)).sum(-1) / ((cream - navy) ** 2).sum(), 0, 1)
    ink = np.zeros((N, N), np.float32)
    # place each half of the print on its panel's hanging part
    half = W // 2
    for kk in range(2):
        x0 = int(round(L.panel_u(kk, 0.0) * N))
        y_top = int(round((1 - L.fabric_v(L.HANG)) * N))
        part = ink_p[:, kk * half:(kk + 1) * half]
        hh, ww = part.shape
        ink[y_top:y_top + hh, x0:x0 + ww] = part[: N - y_top, : N - x0]
    # resist-dyed edges: soft, and the weave shows (the ink sits less on the valleys)
    ink_im = Image.fromarray((ink * 255).astype(np.uint8)).filter(__import__("PIL.ImageFilter", fromlist=["GaussianBlur"]).GaussianBlur(1.2))
    ink = np.asarray(ink_im).astype(np.float32) / 255.0
    ink = np.clip(ink * (0.82 + 0.25 * (height - height.mean())), 0, 1)

    def srgb_to_lin(c):
        return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)

    def lin_to_srgb(c):
        return np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(np.clip(c, 0, None), 1 / 2.4) - 0.055)

    dye = srgb_to_lin(np.array([30, 38, 64], np.float32) / 255.0)
    ink_col = srgb_to_lin(np.array([214, 204, 180], np.float32) / 255.0)   # cotton cream, not paper white
    mottle = smooth_noise((N, N), 180, 4)
    streak = smooth_noise((N // 8, N), 6, 2)                # along the warp: long, thin
    streak = np.asarray(Image.fromarray(streak, mode="F").resize((N, N), Image.BICUBIC))
    shade = 1.0 + 0.06 * mottle + 0.035 * streak
    base = dye[None, None, :] * shade[..., None]
    base = base * (1 - ink[..., None]) + ink_col[None, None, :] * (1 + 0.03 * mottle[..., None]) * ink[..., None]
    # the weave: valleys a little darker (shadowed fibres), peaks a little lighter
    base *= (0.9 + 0.2 * (height - height.min()) / (np.ptp(height) + 1e-6))[..., None]
    # hems: two more layers of the same cloth: deeper colour
    base *= np.where(hem, 0.9, 1.0)[..., None]
    # stitches: cotton thread a shade lighter than the cloth
    thread = srgb_to_lin(np.array([52, 60, 86], np.float32) / 255.0)
    base = np.where(stitch[..., None] & (ink[..., None] < 0.5), thread[None, None, :], base)
    # wear: by the slit (inner edges, the lower half: hands push it aside) and the bottom hem
    inner = np.where(k == 0, L.PANEL_W - across, across)    # distance from the slit
    rub = np.exp(-(inner / 0.07) ** 2) * np.clip((0.55 - s) / 0.4, 0, 1) * 0.6 + np.exp(-(s / 0.03) ** 2) * 0.4
    rub *= 0.7 + 0.3 * smooth_noise((N, N), 30, 2)
    grey = base.mean(-1, keepdims=True)
    faded = base * (1 - 0.25 * rub[..., None]) + (grey * 1.6 + 0.01) * (0.25 * rub[..., None])
    base = np.where(inside[..., None], faded, dye[None, None, :])
    rgb = (np.clip(lin_to_srgb(base), 0, 1) * 255 + 0.5).astype(np.uint8)
    Image.fromarray(rgb, "RGB").save(os.path.join(OUT, "noren_base.png"))

    # ---------------------------------------------------------------- normal map
    hgt = np.where(inside, height, height.mean())
    # px -> metres: the weave's relief is a fraction of a millimetre
    relief = 0.00025
    dx = (np.roll(hgt, -1, 1) - np.roll(hgt, 1, 1)) * 0.5 * relief * pu
    dy = (np.roll(hgt, -1, 0) - np.roll(hgt, 1, 0)) * 0.5 * relief * pv
    # tangent space: +X along u, +Y along v (up the image = -row)
    nx, ny, nz = -dx, dy, np.ones_like(dx)
    ln = np.sqrt(nx * nx + ny * ny + nz * nz)
    nrm = np.stack([nx / ln, ny / ln, nz / ln], -1)
    Image.fromarray(((nrm * 0.5 + 0.5) * 255 + 0.5).astype(np.uint8), "RGB").save(os.path.join(OUT, "noren_normal.png"))

    # ---------------------------------------------------------------- ORM
    ao = 1.0 - 0.25 * (1 - (height - height.min()) / (np.ptp(height) + 1e-6)) - 0.08 * np.exp(-(dist_hem_edge * pu / 3.0) ** 2) - 0.1 * stitch
    rough = 0.89 + 0.03 * smooth_noise((N, N), 60, 2) - 0.07 * ink + 0.02 * (1 - ao)
    orm = np.stack([np.clip(ao, 0, 1), np.clip(rough, 0, 1), np.zeros_like(ao)], -1)
    Image.fromarray((orm * 255 + 0.5).astype(np.uint8), "RGB").save(os.path.join(OUT, "noren_orm.png"))
    print("wrote", OUT, "(%.0f x %.0f px/m)" % (pu, pv))


if __name__ == "__main__":
    main()
