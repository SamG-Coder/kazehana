"""Each town site's own frame (assets/town/town.json's sites), for its building to be laid out in
(tools/blender/build_site.py) and placed by (crates/town/src/plots.rs):

    python tools/site_frames.py      -> assets/town/site_frames.json

A frame: the site's middle (plan x, y), its ground height, its size along its first edge (lx) and
across (ly), and which way its front faces: +ly towards the nearest road, so a building's front
(Blender +Y) faces the street. `theta`: the turn about the world's up axis the game places the
model with (as the houses: the model's back, Blender -Y, to the plan's `back`).
"""

import json
import math
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
t = json.load(open(os.path.join(ROOT, "assets", "town", "town.json"), encoding="utf8"))
KEY = {"Kazehana Elementary School": "elementary", "Kazehana Station": "station", "station plaza": "plaza",
       "Hachiman shrine": "shrine", "Shōrin-ji temple and cemetery": "temple", "post office": "post_office",
       "town office branch": "town_office", "Inari shrine": "inari", "konbini": "konbini",
       "JA farm co-op": "coop", "petrol station": "petrol", "drugstore": "drugstore", "car repair": "car_repair"}


def nearest_road(p, skip_rail=True):
    best = (1e18, None)
    for r in t["roads"]:
        pts = r["pts"]
        for a, b in zip(pts, pts[1:]):
            ax, ay, bx, by = a[0], a[1], b[0], b[1]
            dx, dy = bx - ax, by - ay
            L2 = dx * dx + dy * dy or 1e-9
            u = max(0.0, min(1.0, ((p[0] - ax) * dx + (p[1] - ay) * dy) / L2))
            q = (ax + u * dx, ay + u * dy)
            d = math.dist(p, q)
            if d < best[0]:
                best = (d, q)
    return best[1]


out = {}
for s in t["sites"]:
    key = KEY.get(s["name"])
    if not key:
        continue
    p = s["poly"]
    cx, cy = sum(q[0] for q in p) / len(p), sum(q[1] for q in p) / len(p)
    ex, ey = p[1][0] - p[0][0], p[1][1] - p[0][1]
    lx = math.hypot(ex, ey)
    ex, ey = ex / lx, ey / lx
    ny, nx = ex, -ey                      # (a perpendicular)
    if key == "station":
        # the platform's front is the tracks' side away from the plaza
        pl = next(q for q in t["sites"] if q["name"] == "station plaza")["poly"]
        pc = (sum(q[0] for q in pl) / 4, sum(q[1] for q in pl) / 4)
        toward = (pc[0] - cx, pc[1] - cy)
    else:
        r = nearest_road((cx, cy))
        toward = (r[0] - cx, r[1] - cy)
    if nx * toward[0] + ny * toward[1] < 0:
        nx, ny = -nx, -ny
    # the model's +X must be the frame's x with +Y its front: x = front turned clockwise
    ex, ey = ny, -nx
    back = (-nx, -ny)
    # its size along the model's x and across (its front), from the corners
    xs = [(q[0] - cx) * ex + (q[1] - cy) * ey for q in p]
    ys = [(q[0] - cx) * nx + (q[1] - cy) * ny for q in p]
    lx, ly = max(xs) - min(xs), max(ys) - min(ys)
    # where a road runs along the front inside the site's outline (its carriageway and 0.5 m of
    # shoulder), the lot stops short of it: the frame moves back by that much
    clear = 0.0
    if key not in ("station", "plaza"):
        for j in range(1, 30):
            d = ly / 2 - j * 0.5
            hit = False
            for i in range(-8, 9):
                q = (cx + ex * (lx * 0.5 * i / 8) + nx * d, cy + ey * (lx * 0.5 * i / 8) + ny * d)
                for r in t["roads"]:
                    hw = r["width"] / 2 + 0.5
                    for a_, b_ in zip(r["pts"], r["pts"][1:]):
                        dx, dy = b_[0] - a_[0], b_[1] - a_[1]
                        L2 = dx * dx + dy * dy or 1e-9
                        u = max(0.0, min(1.0, ((q[0] - a_[0]) * dx + (q[1] - a_[1]) * dy) / L2))
                        if math.dist(q, (a_[0] + u * dx, a_[1] + u * dy)) < hw:
                            hit = True
            if hit:
                clear = j * 0.5 + 0.5
    cx, cy, ly = cx - nx * clear / 2, cy - ny * clear / 2, ly - clear
    out[key] = {"name": s["name"], "centre": [round(cx, 3), round(cy, 3)], "ground": s.get("ground"),
                "lx": round(lx, 3), "ly": round(ly, 3), "front_clear": clear,
                "front": [round(nx, 5), round(ny, 5)], "theta": round(math.atan2(back[0], -back[1]), 6)}
json.dump(out, open(os.path.join(ROOT, "assets", "town", "site_frames.json"), "w", encoding="utf8"), ensure_ascii=False, indent=1)
for k, v in out.items():
    print(f"{k:12} {v['lx']:6.1f} x {v['ly']:5.1f}  front {v['front']}")
