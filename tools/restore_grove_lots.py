"""Give the shrines' and the temple's lots back to the hill: their graded platforms (cut into the
slope with bare earth faces by tools/blender/town_terrain.py) are returned to the natural land,
forest floor under their trees, until the shrines are built properly.

    python tools/restore_grove_lots.py

Rewrites, inside each lot and a 6 m band round it (blended), assets/town/height.f32 and
assets/town/landuse.png. Roads are left as they are.
"""

import json
import os
import sys

import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "tools", "blender"))
import town_plan as tp  # noqa: E402

T = os.path.join(ROOT, "assets", "town")
plan = json.load(open(os.path.join(T, "town.json"), encoding="utf8"))
ter = plan["terrain"]
nx, ny = ter["nx"], ter["ny"]
x0, y0 = ter["origin"]
hpath = os.path.join(T, ter["file"])
hgt = np.fromfile(hpath, dtype="<f4").reshape(ny, nx).astype(np.float64)
land = np.array(Image.open(os.path.join(T, ter["landuse"])))
BAND = 6.0
FOREST, MOSS, ROAD = 6, 12, (3, 4, 5, 13)

done = 0
for s in plan["sites"]:
    if s["kind"] not in ("shrine", "inari", "temple"):
        continue
    poly = np.array(s["poly"], dtype=np.float64)
    i0 = max(int(poly[:, 0].min() - BAND - x0) - 1, 0)
    i1 = min(int(poly[:, 0].max() + BAND - x0) + 2, nx - 1)
    j0 = max(int(poly[:, 1].min() - BAND - y0) - 1, 0)
    j1 = min(int(poly[:, 1].max() + BAND - y0) + 2, ny - 1)
    gi, gj = np.meshgrid(np.arange(i0, i1 + 1), np.arange(j0, j1 + 1))
    px, py = x0 + gi, y0 + gj
    # inside the polygon, and the distance to its edge
    inside = np.zeros(px.shape, bool)
    dist = np.full(px.shape, np.inf)
    n = len(poly)
    for k in range(n):
        (ax, ay), (bx, by) = poly[k], poly[(k + 1) % n]
        cond = (ay > py) != (by > py)
        xint = (bx - ax) * (py - ay) / np.where(by - ay == 0, 1e-9, by - ay) + ax
        inside ^= cond & (px < xint)
        dx, dy = bx - ax, by - ay
        u = np.clip(((px - ax) * dx + (py - ay) * dy) / max(dx * dx + dy * dy, 1e-9), 0, 1)
        dist = np.minimum(dist, np.hypot(px - ax - u * dx, py - ay - u * dy))
    w = np.where(inside, 1.0, np.clip(1.0 - dist / BAND, 0.0, 1.0))
    w = w * w * (3 - 2 * w)
    rows = ny - 1 - (gj)
    lu = land[rows, gi]
    road = np.isin(lu, ROAD)
    w = np.where(road, 0.0, w)
    nat = tp.height(px, py)
    blk = hgt[j0:j1 + 1, i0:i1 + 1]
    hgt[j0:j1 + 1, i0:i1 + 1] = blk * (1 - w) + nat * w
    # forest floor inside, moss where it was the shrine's
    newlu = np.where(inside & ~road, FOREST, lu)
    land[rows, gi] = newlu
    done += 1

hgt.astype("<f4").tofile(hpath)
Image.fromarray(land).save(os.path.join(T, ter["landuse"]))
print(f"{done} lots given back to the hill")
