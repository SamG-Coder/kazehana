"""Lower the baked terrain where it stands above an asphalt road (crates/town/tests/ground_checks.rs
the_ground_never_rises_through_a_road), only directly under the asphalt and 0.3 m beyond its edges
(no wider: a wider cut leaves a trench and other roads' and tracks' ribbons hanging over it). Only
ever lowers.

    python tools/fix_road_ground.py

Rewrites assets/town/height.f32. Mirrors crates/town/src/lib.rs road_lift / road_profile.
"""

import json
import os

import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
T = os.path.join(ROOT, "assets", "town")
plan = json.load(open(os.path.join(T, "town.json"), encoding="utf8"))
ter = plan["terrain"]
nx, ny = ter["nx"], ter["ny"]
x0, y0 = ter["origin"]
path = os.path.join(T, ter["file"])
hgt = np.fromfile(path, dtype="<f4").reshape(ny, nx).astype(np.float64)
land = np.array(Image.open(os.path.join(T, ter["landuse"])))

LIFT = {"pref": 0.050, "school": 0.048, "old": 0.046, "street": 0.044, "lane": 0.042}
EDGE_LIFT = 0.55
MARGIN = 0.75



def segments():
    for road in plan["roads"]:
        if road["kind"] in ("farm", "levee") or not road.get("h"):
            continue
        pts, h, hw = road["pts"], road["h"], road["width"] / 2
        lift = LIFT.get(road["kind"], 0.040)
        for i in range(1, len(pts)):
            yield road, pts[i - 1], pts[i], h[i - 1], h[i], hw, lift


# first: the highest asphalt over every cell (any road's footprint), so no cut goes under a road
# that passes above this one (hairpins, crossings on a slope)
over = np.full(hgt.shape, -np.inf)
for road, (ax, ay), (bx, by), ha, hb, hw, lift in segments():
    dx, dy = bx - ax, by - ay
    L2 = dx * dx + dy * dy
    if L2 < 1e-6:
        continue
    i0 = max(int(np.floor(min(ax, bx) - hw - x0)) - 1, 0)
    i1 = min(int(np.ceil(max(ax, bx) + hw - x0)) + 1, nx - 1)
    j0 = max(int(np.floor(min(ay, by) - hw - y0)) - 1, 0)
    j1 = min(int(np.ceil(max(ay, by) + hw - y0)) + 1, ny - 1)
    if i1 < i0 or j1 < j0:
        continue
    gi, gj = np.meshgrid(np.arange(i0, i1 + 1), np.arange(j0, j1 + 1))
    px, py = x0 + gi, y0 + gj
    u = np.clip(((px - ax) * dx + (py - ay) * dy) / L2, 0.0, 1.0)
    d = np.hypot(px - (ax + u * dx), py - (ay + u * dy))
    top = ha + (hb - ha) * u + lift * EDGE_LIFT
    blk = over[j0:j1 + 1, i0:i1 + 1]
    m = d < hw + 0.5
    blk[m] = np.maximum(blk[m], top[m])

lowered = 0
for road in plan["roads"]:
    if road["kind"] in ("farm", "levee") or not road.get("h"):
        continue
    pts, h, hw = road["pts"], road["h"], road["width"] / 2
    reach = hw + MARGIN
    lift = LIFT.get(road["kind"], 0.040)
    for i in range(1, len(pts)):
        (ax, ay), (bx, by) = pts[i - 1], pts[i]
        dx, dy = bx - ax, by - ay
        L2 = dx * dx + dy * dy
        if L2 < 1e-6:
            continue
        i0 = max(int(np.floor(min(ax, bx) - reach - x0)) - 1, 0)
        i1 = min(int(np.ceil(max(ax, bx) + reach - x0)) + 1, nx - 1)
        j0 = max(int(np.floor(min(ay, by) - reach - y0)) - 1, 0)
        j1 = min(int(np.ceil(max(ay, by) + reach - y0)) + 1, ny - 1)
        if i1 < i0 or j1 < j0:
            continue
        gi, gj = np.meshgrid(np.arange(i0, i1 + 1), np.arange(j0, j1 + 1))
        px, py = x0 + gi, y0 + gj
        u = np.clip(((px - ax) * dx + (py - ay) * dy) / L2, 0.0, 1.0)
        d = np.hypot(px - (ax + u * dx), py - (ay + u * dy))
        near = d < reach
        if not near.any():
            continue
        ch = h[i - 1] + (h[i] - h[i - 1]) * u
        top = ch + lift + (lift * EDGE_LIFT - lift) * np.minimum(d / hw, 1.0)
        limit = top - 0.05
        block = hgt[j0:j1 + 1, i0:i1 + 1]
        # (not under another road that passes above this one there)
        # (only the road's own ground: asphalt, and its gravel shoulder; never a field, a garden
        # or a verge beside it)
        lu_blk = land[ny - 1 - np.arange(j0, j1 + 1)][:, i0:i1 + 1]
        # (under the road anything; in the 0.75 m beyond its edge, never a paddy, a vegetable
        # plot or a garden: no trench along a field)
        cut = near & (block > limit) & ((d <= hw) | ~np.isin(lu_blk, (1, 2, 9)))
        if cut.any():
            block[cut] = limit[cut]
            lowered += int(cut.sum())

hgt.astype("<f4").tofile(path)
print(f"road ground: {lowered} cells lowered")
