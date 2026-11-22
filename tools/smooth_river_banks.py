"""Re-cut the river channel's profile in the baked terrain from the exact distance to the river's
centre line, without regenerating the town (tools/blender/town_terrain.py cut it from a distance
measured on the 1 m raster, so the revetments' crest stepped a metre at a time where the river
runs at a slant).

    python tools/smooth_river_banks.py

Rewrites, inside 20.5 m of the river's line only (the channel, the revetments, the levee's
inner slope and its crest path), assets/town/height.f32 and assets/town/landuse.png with the
same profile and codes as town_terrain.py. The levee's outer slope (blended with the land beyond)
and everything else is left as it is.
"""

import json
import os
import sys

import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "tools", "blender"))
import town_plan as tp  # noqa: E402
import town_terrain as tt  # noqa: E402

T = os.path.join(ROOT, "assets", "town")
plan = json.load(open(os.path.join(T, "town.json"), encoding="utf8"))
ter = plan["terrain"]
nx, ny = ter["nx"], ter["ny"]
x0, y0 = ter["origin"]
hgt = np.fromfile(os.path.join(T, ter["file"]), dtype="<f4").reshape(ny, nx).astype(np.float64)
land = np.array(Image.open(os.path.join(T, ter["landuse"])))           # row 0 the north edge
pts = np.array(plan["river"]["pts"], dtype=np.float64)

# the exact distance from every point near the river to its polyline (by bands of rows)
LIM = 20.5
W = tp.RIVER_WATER
changed = 0
xs = x0 + np.arange(nx, dtype=np.float64)
for j0 in range(0, ny, 64):
    ys = y0 + np.arange(j0, min(ny, j0 + 64), dtype=np.float64)
    if ys[-1] < pts[:, 1].min() - 30 or ys[0] > pts[:, 1].max() + 30:
        continue
    gx, gy = np.meshgrid(xs, ys)
    r = np.full(gx.shape, np.inf)
    for a, b in zip(pts[:-1], pts[1:]):
        if max(a[1], b[1]) < ys[0] - LIM or min(a[1], b[1]) > ys[-1] + LIM:
            continue
        d = b - a
        L2 = max(d @ d, 1e-9)
        u = np.clip(((gx - a[0]) * d[0] + (gy - a[1]) * d[1]) / L2, 0.0, 1.0)
        r = np.minimum(r, np.hypot(gx - (a[0] + u * d[0]), gy - (a[1] + u * d[1])))
    # (roads and their bridge approaches, the railway: left as they were graded)
    img_rows0 = ny - 1 - np.arange(j0, j0 + gx.shape[0])
    lu0 = land[img_rows0]
    m = (r < LIM) & (lu0 != tt.LANDUSE["asphalt"]) & (lu0 != tt.LANDUSE["ballast"])
    # (and clear of every road by its half width and 1.5 m: their shoulders and bridge approaches
    # were graded after the channel was cut)
    if m.any():
        near = np.zeros(gx.shape, bool)
        for road in plan["roads"]:
            q = np.array(road["pts"], dtype=np.float64)
            hw = road["width"] / 2 + 1.5
            for a, b in zip(q[:-1], q[1:]):
                if max(a[1], b[1]) < ys[0] - hw or min(a[1], b[1]) > ys[-1] + hw:
                    continue
                d = b - a
                L2 = max(d @ d, 1e-9)
                u = np.clip(((gx - a[0]) * d[0] + (gy - a[1]) * d[1]) / L2, 0.0, 1.0)
                near |= np.hypot(gx - (a[0] + u * d[0]), gy - (a[1] + u * d[1])) < hw
        m &= ~near
    if not m.any():
        continue
    wl = tt.water_level(gx)
    crest = tp.floor_h(gx) + tt.CREST_UP
    prof = np.where(r < W, wl - 0.35 - 1.1 * np.clip(1 - (r / W) ** 2, 0, 1), np.nan)
    prof = np.where((r >= W) & (r < 12), wl - 0.35 + (r - W) / (12 - W) * 1.9, prof)
    prof = np.where((r >= 12) & (r < 16), wl + 1.55 + (r - 12) / 4.0 * (crest - wl - 1.55), prof)
    prof = np.where((r >= 16) & (r < 21), crest, prof)
    rows = slice(j0, j0 + gx.shape[0])
    block = hgt[rows]
    block[m] = prof[m]
    # land use (row 0 north: the image row of height row j is ny - 1 - j)
    code = np.full(gx.shape, 255, np.uint8)
    code[r < W] = tt.LANDUSE["riverbed"]
    code[(r >= W) & (r < 12)] = tt.LANDUSE["concrete"]
    code[(r >= 12) & (r < 16)] = tt.LANDUSE["levee"]
    code[(r >= 16) & (r < 21)] = tt.LANDUSE["gravel"]
    img_rows = ny - 1 - np.arange(j0, j0 + gx.shape[0])
    sub = land[img_rows]
    sub = np.where(m & (code != 255), code, sub)
    land[img_rows] = sub
    changed += int(m.sum())

hgt.astype("<f4").tofile(os.path.join(T, ter["file"]))
Image.fromarray(land).save(os.path.join(T, ter["landuse"]))
print(f"river banks re-cut: {changed} cells")
