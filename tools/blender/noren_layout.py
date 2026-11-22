"""The shop noren's shape and texture layout, shared by its texture maker (tools/make_noren_cloth.py)
and its mesh (tools/blender/noren_mesh.py). Metres.

A noren: two panels of heavy cotton hanging from a wooden rod through a sewn sleeve at the top.
Each panel's fabric runs (its path length s from the bottom hem) up the front to the rod, over
it (the sleeve), and a little way down the back, where the sleeve is stitched.
"""

import math

PANEL_W = 0.41      # each panel's width
SLIT = 0.04         # the gap between the two panels
HANG = 0.82         # from the bottom hem up to the rod's centre (front)
ROD_R = 0.012       # the shop's wooden rod (tools/blender/build_shop_tsukumodo.py)
SLEEVE_R = 0.016    # the sleeve round the rod (a little loose)
BACK = 0.022        # the sleeve's back layer, below the rod's centre, to its seam
SEAM = 0.026        # the sleeve's seam on the front, below the rod's centre
HEM = 0.012         # side and bottom hems (folded twice, stitched)
FOLD_DEPTH = 0.018  # the soft vertical folds it hangs in (tools/blender/noren_mesh.py fold)
FOLD_WAVE = 0.14
# The cloth's top band that stays on the rod (the sleeve): its vertices are pinned by the cloth
# simulation (crates/town/src/cloth.rs PIN_BAND must match).
PIN_BAND = 0.042

S_TOTAL = HANG + math.pi * SLEEVE_R + BACK

# texture: one square for both panels, side by side (the print's left half on the first)
TEX = 2048
GUTTER_U = 0.008
GUTTER_V = 0.006


def panel_u(k, t):
    """u of panel k (0: the first, its print's left half; 1: the second) at t across it (0 at
    its left edge as seen from the front, 1 at its right)."""
    u0 = k * 0.5 + GUTTER_U
    u1 = (k + 1) * 0.5 - GUTTER_U
    return u0 + (u1 - u0) * t


def fabric_v(s):
    """v (Blender: 0 at the bottom of the image) of the fabric at path length s from the hem."""
    return GUTTER_V + (1 - 2 * GUTTER_V) * s / S_TOTAL


def px_per_m():
    """Texels per metre across and along the fabric."""
    return (0.5 - 2 * GUTTER_U) * TEX / PANEL_W, (1 - 2 * GUTTER_V) * TEX / S_TOTAL
