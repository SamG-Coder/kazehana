"""The town plan baked into ground the engine renders (used by town_plan.py).

assets/town/height.f32: (NX x NY) little-endian f32 heights on a 1 m grid; row j at y = Y0 + j,
    column i at x = X0 + i. The river channel, revetments and levees, the rail embankment, graded
    roads, level paddies (sunk below their levees), vegetable plots, house platforms and sites
    are cut into the natural land.
assets/town/landuse.png: 8-bit land-use codes (LANDUSE) on the same 1 m grid, row 0 at the
    north edge, for the ground shader.
"""

import math
import os

import numpy as np

import town_plan as tp

LANDUSE = {  # what the ground is (crates/town ground layers)
    "grass": 0, "paddy": 1, "hatake": 2, "gravel": 3, "concrete": 4, "asphalt": 5, "forest": 6,
    "riverbed": 7, "levee": 8, "garden": 9, "dirt": 10, "shrine": 11, "moss": 12, "ballast": 13,
}
WATER_DROP, CREST_UP = 1.9, 1.3  # the river's water level below the floor; the levee crest above it
TERRACE = 0.5  # a plot this much above or below its road is terraced (crates/town/src/plots.rs)



def water_level(x):
    return tp.floor_h(x) - WATER_DROP


# a level crossing's road surface is the rail heads' level: rails 0.24 m above the ballast top,
# less the road ribbon's lift (crates/town/src/lib.rs)
CROSSING_UP = 0.195

def export_terrain(plan):
    from PIL import Image, ImageDraw
    from scipy import ndimage
    X0, Y0, X1, Y1 = tp.X0, tp.Y0, tp.X1, tp.Y1
    NX, NY = int(X1 - X0) + 1, int(Y1 - Y0) + 1
    xs = X0 + np.arange(NX, dtype=np.float64)
    hgt = np.empty((NY, NX), np.float64)
    for j0 in range(0, NY, 200):  # the natural land, in strips
        ys = Y0 + np.arange(j0, min(NY, j0 + 200), dtype=np.float64)
        gx, gy = np.meshgrid(xs, ys)
        hgt[j0:j0 + len(ys)] = tp.height(gx, gy)
    natural = hgt.copy()
    gx, gy = np.meshgrid(xs, Y0 + np.arange(NY, dtype=np.float64))
    land = np.where(tp.rel_height(gx, gy) > 2.0, LANDUSE["forest"], LANDUSE["grass"]).astype(np.uint8)
    # bamboo groves: a floor of dry bamboo leaves, no grass
    bl, db = Image.new("L", (NX, NY), 0), None
    db = ImageDraw.Draw(bl)
    for t in plan.trees:
        if t[2] == "bamboo":
            x, y = t[0] - X0, t[1] - Y0
            db.ellipse([x - 3.5, y - 3.5, x + 3.5, y + 3.5], fill=255)
    land[np.array(bl) > 0] = LANDUSE["forest"]

    def img(mode, fill=0):
        im = Image.new(mode, (NX, NY), fill)
        return im, ImageDraw.Draw(im)

    P = lambda p: (p[0] - X0, p[1] - Y0)  # image coords, row = y index (flipped when saved)

    def sample(a, p, order=1):
        return float(ndimage.map_coordinates(a, [[p[1] - Y0], [p[0] - X0]], order=order)[0])

    # ---- river: channel, concrete revetments, grass slopes, a gravel path on the levee crest
    cl, dc = img("L", 0)
    dc.line([P(p) for p in plan.river], fill=255, width=1)
    r = ndimage.distance_transform_edt(np.array(cl) == 0)
    wl = water_level(gx)
    crest = tp.floor_h(gx) + CREST_UP
    W = tp.RIVER_WATER
    prof = np.full_like(hgt, np.nan)
    prof = np.where(r < W, wl - 0.35 - 1.1 * np.clip(1 - (r / W) ** 2, 0, 1), prof)
    m = (r >= W) & (r < 12)
    prof = np.where(m, wl - 0.35 + (r - W) / (12 - W) * 1.9, prof)
    m = (r >= 12) & (r < 16)
    prof = np.where(m, wl + 1.55 + (r - 12) / 4.0 * (crest - wl - 1.55), prof)
    prof = np.where((r >= 16) & (r < 21), crest, prof)
    m = (r >= 21) & (r < 24.5)
    prof = np.where(m, crest + (hgt - crest) * (r - 21) / 3.5, prof)
    hgt = np.where(np.isnan(prof), hgt, prof)
    land[r < W] = LANDUSE["riverbed"]
    land[(r >= W) & (r < 12)] = LANDUSE["concrete"]
    land[((r >= 12) & (r < 16)) | ((r >= 21) & (r < 24.5))] = LANDUSE["levee"]
    land[(r >= 16) & (r < 21)] = LANDUSE["gravel"]

    # ---- railway embankment (0.9 m of ballast)
    cl, dc = img("L", 0)
    dc.line([P(p) for p in plan.rail.pts], fill=255, width=1)
    rd = ndimage.distance_transform_edt(np.array(cl) == 0)
    rail_h = ndimage.uniform_filter(natural, 25) + 0.9
    hgt = np.where(rd < 3.2, rail_h, np.where(rd < 6.0, rail_h + (hgt - rail_h) * (rd - 3.2) / 2.8, hgt))
    land[(rd >= 2.3) & (rd < 6.5)] = LANDUSE["grass"]  # the embankment's sides: grassed
    land[rd < 2.3] = LANDUSE["ballast"]  # ballast only on the top, round the sleepers
    # the track's level (ballast top) at every rail point: the rails and sleepers sit on it, the
    # level crossings are built up to it
    plan.rail_heights = [round(sample(rail_h, p), 3) for p in plan.rail.pts]

    # ---- areas made level: paddies (sunk below their levees), vegetable plots, plots, sites
    ids, di = img("I", 0)
    areas = [(q, "paddy", None) for q in plan.paddies] + [(q, "hatake", None) for q in plan.hatake]
    areas += [(s["poly"], "site", s) for s in plan.sites] + [(l["poly"], "plot", l) for l in plan.plots]
    for k, (q, kind, obj) in enumerate(areas):
        di.polygon([P(p) for p in q], fill=k + 1)
    idx = np.array(ids, dtype=np.int64)
    n = len(areas) + 1
    cnt = np.bincount(idx.ravel(), minlength=n)
    mean = np.bincount(idx.ravel(), weights=hgt.ravel(), minlength=n) / np.maximum(cnt, 1)
    lo = np.full(n, np.inf)
    np.minimum.at(lo, idx.ravel(), hgt.ravel())
    level = mean.copy()
    plan.paddy_levels = []
    code = np.zeros(n, np.uint8)
    site_code = {"school": "concrete", "shrine": "moss", "inari": "moss", "temple": "shrine", "plaza": "asphalt",
                 "konbini": "asphalt", "business": "asphalt", "station": "concrete", "civic": "asphalt"}
    for k, (q, kind, obj) in enumerate(areas):
        i = k + 1
        if kind == "paddy":
            level[i] = (lo[i] if np.isfinite(lo[i]) else mean[i]) - 0.12
            plan.paddy_levels.append(round(float(level[i]), 3))
            code[i] = LANDUSE["paddy"]
        elif kind == "hatake":
            code[i] = LANDUSE["hatake"]
        elif kind == "site":
            level[i] = mean[i] + 0.1
            obj["ground"] = round(float(level[i]), 2)
            code[i] = LANDUSE[site_code[obj["kind"]]]
        else:
            level[i] = mean[i] + 0.15  # a platform a step above the road
            obj["ground"] = round(float(level[i]), 2)
            code[i] = LANDUSE["gravel"]
    inside = (idx > 0) & (rd >= 6.5)  # (the rail embankment and its grass slopes stay)
    hgt = np.where(inside, level[idx], hgt)
    land = np.where(inside, code[idx], land)

    # details inside plots and sites: garden lawns, concrete parking and floors, the dirt ground
    det, dd = img("L", 255)
    for l in plan.plots:
        o, e, nrm, w, D = l["frame"]
        hx = [tp.dot(tp.v_sub(p, o), e) for p in l["house"]]
        hy = [tp.dot(tp.v_sub(p, o), nrm) for p in l["house"]]
        a0, a1, b0, b1 = 0.5, w - 0.5, 0.5, D - 0.5
        box = {"front": (a0, a1, b0, min(hy) - 0.6), "back": (a0, a1, max(hy) + 0.6, b1),
               "left": (a0, min(hx) - 0.6, b0, b1), "right": (max(hx) + 0.6, a1, b0, b1)}[l["garden_side"]]
        if l["type"] == "shophouse":  # a shop's plot is concrete out to the road
            dd.polygon([P(p) for p in l["poly"]], fill=LANDUSE["concrete"])
        if l["type"] != "shophouse" and box[1] - box[0] > 1.5 and box[3] - box[2] > 1.5:
            dd.polygon([P(p) for p in tp.rect(o, e, nrm, *box)], fill=LANDUSE["garden"])
        for k in ("parking", "house", "shed"):
            if l[k]:  # (driveway slabs and buildings are meshes: under them, concrete, where
                # nothing grows; tools: crates/town/tests/ground_checks.rs)
                dd.polygon([P(p) for p in l[k]], fill=LANDUSE["concrete"])
    for s in plan.sites:
        if s["kind"] == "school" and "Junior High" in s["name"]:
            # the junior high's yard: packed earth, asphalt paths, concrete aprons
            # (tools/paint_school_grounds.py)
            import sys  # noqa: E402
            sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
            import paint_school_grounds as psg  # noqa: E402
            for poly, use in psg.school_grounds(s, plan.school_gate):
                dd.polygon([P(p) for p in poly], fill=LANDUSE[use])
            continue
        for b in s["buildings"]:
            dd.polygon([P(p) for p in b["poly"]], fill=LANDUSE["dirt"] if b["name"] == "sports ground" else LANDUSE["concrete"])
    det = np.array(det)
    land = np.where(det != 255, det, land)

    # ---- roads: graded along their length; the cross-section level; bridges stay up at the
    # crest height (the deck is a mesh, the channel stays under it)
    best = np.full(hgt.shape, np.inf)
    cbest = np.full(hgt.shape, np.inf)
    # (the road's shoulder and verge stop at a shop's concrete front)
    sm_img, ds = img("L", 0)
    for l in plan.plots:
        if l["type"] == "shophouse":
            ds.polygon([P(p) for p in l["poly"]], fill=1)
    shopfront = np.array(sm_img) > 0
    # ...and a shop's concrete runs on out to the road's edge in front of it
    shop_apron = ndimage.binary_dilation(shopfront, iterations=3)
    road_h = np.zeros_like(hgt)
    road_code = np.zeros(hgt.shape, np.uint8)
    # where a road's surface is (and its edge): nothing levelled later may cover it
    road_surface = np.zeros(hgt.shape, bool)
    ramps = []  # (a, b, ha, hb, hw): road segments raised onto a level crossing's approach
    for ri, road in enumerate(plan.roads):
        pts = road.pts
        hs = np.array([sample(hgt, p) for p in pts])
        over = np.array([sample(r, p, 0) < 21 for p in pts])
        hs = np.where(over, np.array([float(tp.floor_h(p[0])) + CREST_UP for p in pts]), hs)
        L = np.concatenate([[0], np.cumsum([tp.v_len(tp.v_sub(b, a)) for a, b in zip(pts[:-1], pts[1:])])])
        sm = np.array([hs[np.abs(L - L[i]) < 12].mean() for i in range(len(pts))])
        sm = np.where(over, hs, sm)
        if road.kind in ("farm", "levee"):
            sm = hs  # field tracks lie on the ground as it is
        # level crossings: the road rises to the track on ramps, its surface flush with the rail
        # heads at the crossing (the embankment's fill runs on under the ramps)
        rp = plan.rail.pts
        xs = []
        for k, (a, b) in enumerate(zip(pts[:-1], pts[1:])):
            for c, d in zip(rp[:-1], rp[1:]):
                if tp.seg_intersect(a, b, c, d):
                    den = (b[0] - a[0]) * (d[1] - c[1]) - (b[1] - a[1]) * (d[0] - c[0])
                    t = ((c[0] - a[0]) * (d[1] - c[1]) - (c[1] - a[1]) * (d[0] - c[0])) / den
                    q = (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t)
                    xs.append((L[k] + t * (L[k + 1] - L[k]), sample(rail_h, q) + CROSSING_UP))
        if xs:
            extra = [s + o for s, _ in xs for o in (-18, -12, -7, -3.5, 0, 3.5, 7, 12, 18) if 0 < s + o < L[-1]]
            Ln = np.unique(np.concatenate([L, extra]))
            PA = np.array(pts)
            pts = [(float(x), float(y)) for x, y in zip(np.interp(Ln, L, PA[:, 0]), np.interp(Ln, L, PA[:, 1]))]
            sm = np.interp(Ln, L, sm)
            for s, top in xs:
                u = np.clip((np.abs(Ln - s) - 3.5) / 14.5, 0, 1)
                w = 1 - u * u * (3 - 2 * u)
                sm = sm * (1 - w) + top * w
            road.pts = pts
            for k in range(len(pts) - 1):
                if any(min(abs(Ln[k] - s), abs(Ln[k + 1] - s)) < 20 for s, _ in xs):
                    ramps.append((pts[k], pts[k + 1], float(sm[k]), float(sm[k + 1]), road.width / 2))
        road.heights = [round(float(v), 3) for v in sm]
        hw = road.width / 2
        c = LANDUSE["gravel"] if road.kind in ("farm", "levee") else LANDUSE["asphalt"]
        for (a, b), ha, hb in zip(road.segs(), sm[:-1], sm[1:]):
            x0 = max(int(math.floor(min(a[0], b[0]) - hw - 2 - X0)), 0)
            y0 = max(int(math.floor(min(a[1], b[1]) - hw - 2 - Y0)), 0)
            x1 = min(int(math.ceil(max(a[0], b[0]) + hw + 2 - X0)), NX - 1)
            y1 = min(int(math.ceil(max(a[1], b[1]) + hw + 2 - Y0)), NY - 1)
            if x1 <= x0 or y1 <= y0:
                continue
            px, py = np.meshgrid(X0 + np.arange(x0, x1 + 1), Y0 + np.arange(y0, y1 + 1))
            ab = (b[0] - a[0], b[1] - a[1])
            t = np.clip(((px - a[0]) * ab[0] + (py - a[1]) * ab[1]) / max(ab[0] ** 2 + ab[1] ** 2, 1e-9), 0, 1)
            d = np.hypot(px - (a[0] + ab[0] * t), py - (a[1] + ab[1] * t))
            sub = (slice(y0, y1 + 1), slice(x0, x1 + 1))
            # (the road's shoulder grading stops at the edge of plots, fields and sites: a plot
            # meets the road with a step, held by a retaining wall where it is a big one)
            # (nearest by distance from the road's edge, not its centre line: where two roads run
            # side by side, each one's surface keeps its own height)
            e = d - hw
            win = (d < hw + 1.5) & (e < best[sub]) & ~(r[sub] < 12) & (~inside[sub] | (d < hw + 1.45))
            best[sub] = np.where(win, e, best[sub])
            road_surface[sub] |= win & (d < hw + 1.35)  # (and one 1 m cell more: the mesh's slope)
            road_h[sub] = np.where(win, ha + (hb - ha) * t, road_h[sub])
            # the asphalt's edge, then a gravel shoulder (0.3 - 1.2 m, wider and narrower along the
            # road), then a worn dirt verge in patches before the grass
            nz = tp.fbm(px, py, 14.0, 311 + ri, 2)
            sw = 0.75 + 0.45 * nz
            dw = sw + np.clip(0.2 + 0.7 * tp.fbm(px, py, 9.0, 517 + ri, 2), 0.0, 0.9)
            if c == LANDUSE["asphalt"]:
                code = np.where(d < hw - 0.25, c, np.where(d < hw + sw, LANDUSE["gravel"], np.where(d < hw + dw, LANDUSE["dirt"], 0)))
            else:
                code = np.where(d < hw + 0.3, c, np.where(d < hw + 0.3 + np.clip(0.3 + 0.6 * nz, 0.0, 0.9), LANDUSE["dirt"], 0))
            # (the shoulder runs along fields and plot fronts too: only its height stops at them)
            cwin = (d < hw + 1.5) & (d < cbest[sub]) & ~(r[sub] < 12) & ~(shopfront[sub] & (d > hw + 0.2))
            cbest[sub] = np.where(cwin, d, cbest[sub])
            if c == LANDUSE["asphalt"]:
                code = np.where(shop_apron[sub] & (d >= hw - 0.25), LANDUSE["concrete"], code)
            road_code[sub] = np.where(cwin & (code > 0), code, np.where(cwin, 0, road_code[sub]))
    on = np.isfinite(best)
    hgt = np.where(on, road_h, hgt)
    land = np.where(road_code > 0, road_code, land)
    # driveways, houses and sheds last: concrete, whatever the gardens and the road's shoulder
    # painted (nothing grows on them: crates/town/tests/ground_checks.rs)
    pv, dpv = img("L", 0)
    for l in plan.plots:
        for k in ("parking", "house", "shed"):
            if l[k]:
                dpv.polygon([P(p) for p in l[k]], fill=1)
    # (one cell more round them: a cell part under the slab's edge grows nothing either)
    land = np.where(ndimage.binary_dilation(np.array(pv) > 0, iterations=1), LANDUSE["concrete"], land)
    # the crossings' approach ramps stand on a fill: a grass bank down from the road's shoulder at
    # 1 in 1.5, wherever the ground beside the ramp is lower (fields and plots included)
    before = hgt.copy()
    for a, b, ha, hb, hw in ramps:
        x0 = max(int(math.floor(min(a[0], b[0]) - hw - 8 - X0)), 0)
        y0 = max(int(math.floor(min(a[1], b[1]) - hw - 8 - Y0)), 0)
        x1 = min(int(math.ceil(max(a[0], b[0]) + hw + 8 - X0)), NX - 1)
        y1 = min(int(math.ceil(max(a[1], b[1]) + hw + 8 - Y0)), NY - 1)
        px, py = np.meshgrid(X0 + np.arange(x0, x1 + 1), Y0 + np.arange(y0, y1 + 1))
        ab = (b[0] - a[0], b[1] - a[1])
        t = np.clip(((px - a[0]) * ab[0] + (py - a[1]) * ab[1]) / max(ab[0] ** 2 + ab[1] ** 2, 1e-9), 0, 1)
        d = np.hypot(px - (a[0] + ab[0] * t), py - (a[1] + ab[1] * t))
        sub = (slice(y0, y1 + 1), slice(x0, x1 + 1))
        target = ha + (hb - ha) * t - np.maximum(d - hw - 0.8, 0) / 1.5
        # (never over another road, nor its edge: crates/town/tests/ground_checks.rs)
        # (this ramp may fill its own sides; where another road is nearer, its surface stays)
        mine = best[sub] >= (d - hw) - 0.01
        up = (target > hgt[sub] + 0.02) & (d > hw) & (~road_surface[sub] | mine)
        hgt[sub] = np.where(up, target, hgt[sub])
        bank = up & (d > hw + 1.0) & (land[sub] != LANDUSE["ballast"]) & (land[sub] != LANDUSE["concrete"])
        land[sub] = np.where(bank, LANDUSE["grass"], land[sub])
    if ramps:
        # round the fill off (not under the roads themselves: they keep their grade)
        fill = ndimage.gaussian_filter(hgt - before, 1.3)
        hgt = np.where(on, hgt, np.maximum(before + fill, before))

    # terraced plots (more than TERRACE m above or below their road): dug down (or filled up) to
    # the road's level, the way hillside plots are built; the bank left at the back and sides is
    # held by retaining walls (meshes: crates/town/src/plots.rs)
    tid, dj = img("I", 0)
    lut = np.full(len(plan.plots) + 1, np.nan)
    for k, l in enumerate(plan.plots):
        o, e, n, w, D = l["frame"]
        g = l.get("ground")
        if g is None:
            continue
        gate = l["gate"]
        rh = sample(hgt, (gate[0] - n[0] * 1.5, gate[1] - n[1] * 1.5))
        if abs(g - rh) <= TERRACE:
            continue
        lvl = rh + 0.1
        dj.polygon([P(q) for q in l["poly"]], fill=k + 1)
        lut[k + 1] = lvl
        l["ground"] = round(float(lvl), 2)
        l["terraced"] = True
    v = lut[np.array(tid, dtype=np.int64)]
    # (a plot's level stops at the road: crates/town/tests/ground_checks.rs)
    hgt = np.where(np.isnan(v) | road_surface, hgt, v)

    os.makedirs(tp.OUT, exist_ok=True)
    hgt.astype("<f4").tofile(os.path.join(tp.OUT, "height.f32"))
    Image.fromarray(np.ascontiguousarray(land[::-1]), "L").save(os.path.join(tp.OUT, "landuse.png"))
    plan.terrain = {"file": "height.f32", "landuse": "landuse.png", "origin": [X0, Y0], "cell": 1.0, "nx": NX, "ny": NY,
                    "water_drop": WATER_DROP, "crest_up": CREST_UP, "codes": LANDUSE}
    return hgt, land
