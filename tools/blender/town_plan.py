"""Kazehana-chō: the whole town, planned (docs/TOWN_PLAN.md).

    python tools/blender/town_plan.py   -> assets/town/town.json, map.png, map_centre.png + review

The town is one connected place: Tsukumodō (home) on the station street, the walk to school (the
Path level) and the junior high all sit on this map, and every household the sim gives a student
lives in one of its houses.

How the town is put together (the way satoyama towns actually grew):
  * A valley floor of rice paddies with the Kazehana river meandering through it, forested hills
    north and south, and small side valleys (yato) cutting into the hills.
  * Hamlets (shūraku) sit at the yato mouths and along the hill foot, out of the flood plain,
    with a knot of winding lanes, vegetable plots, bamboo and windbreak trees. Terraced paddies
    climb the yato. Between hamlets the paddies run right up to the hill foot.
  * The old road follows the north hill foot from hamlet to hamlet. The railway and the
    prefectural road came later and run straighter across the floor.
  * The town centre is a small street network round the station: the station street (with the
    shops, Tsukumodō among them) runs from the old road down to the station and on, as the
    school road, over the level crossing, across the paddies and the river to the junior high.
"""

import json
import math
import os

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUT = os.path.join(ROOT, "assets", "town")
SEED = 2026
X0, X1, Y0, Y1 = -1500.0, 1500.0, -1200.0, 1200.0

# ============================================================================= noise


def _hash(ix, iy, s):
    h = (ix * 374761393 + iy * 668265263 + s * 1442695041) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return ((h ^ (h >> 16)) & 0xFFFFFFFF) / 4294967296.0


def vnoise(x, y, scale, seed):
    x = np.asarray(x, dtype=np.float64) / scale
    y = np.asarray(y, dtype=np.float64) / scale
    ix, iy = np.floor(x).astype(np.int64), np.floor(y).astype(np.int64)
    fx, fy = x - ix, y - iy
    u, v = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    a, b = _hash(ix, iy, seed), _hash(ix + 1, iy, seed)
    c, d = _hash(ix, iy + 1, seed), _hash(ix + 1, iy + 1, seed)
    return (a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v) * 2 - 1


def fbm(x, y, scale, seed, octaves=4):
    s, amp, norm = 0.0, 1.0, 0.0
    for i in range(octaves):
        s = s + amp * vnoise(x, y, scale / 2 ** i, seed + 17 * i)
        norm += amp
        amp *= 0.5
    return s / norm


def sstep(t):
    t = np.clip(t, 0.0, 1.0)
    return t * t * (3 - 2 * t)


# ============================================================================= terrain

A = lambda x: np.asarray(x, dtype=np.float64)


def vc(x):
    """Valley centre line (y at x)."""
    x = A(x)
    return 60 * np.sin(x / 650.0) - 0.06 * x


def floor_h(x):
    return 22.0 - 0.004 * A(x)


def north_foot(x):
    x = A(x)
    return 305 + 32 * np.sin(x / 310 + 0.7) + 12 * np.sin(x / 137 + 2) + 16 * vnoise(x, 0.0, 85, 11) + 7 * vnoise(x, 0.0, 33, 12) - 35 * np.exp(-((x - 330) / 70) ** 2)


def south_base(x):
    x = A(x)
    return 255 + 28 * np.sin(x / 280 + 2) + 12 * np.sin(x / 120) + 16 * vnoise(x, 0.0, 80, 13) + 7 * vnoise(x, 0.0, 31, 14)


def south_foot(x):
    x = A(x)
    return south_base(x) + 160 * np.exp(-((x - 60) / 320) ** 2)  # the bay the junior high sits in


# side valleys (yato): name, mouth x, side (+1 north), length, half width, angle (deg), meander
YATO = [
    ("Kami", -1150, 1, 380, 30, -10, 25),
    ("Yamanoue", -650, 1, 760, 45, 8, 40),
    ("Tera", -260, 1, 220, 22, 0, 10),
    ("Higashi", 560, 1, 420, 32, 12, 30),
    ("Shimo", 1150, 1, 300, 28, -5, 20),
    ("Minami-nishi", -950, -1, 400, 32, 10, 30),
    ("Minami", -350, -1, 300, 26, -8, 20),
    ("Minami-higashi", 420, -1, 380, 30, 5, 25),
    ("Oku", 950, -1, 320, 26, 0, 20),
    ("Nishi", -440, 1, 260, 24, -6, 15),
    ("Hazama", 1320, -1, 300, 26, -5, 20),
]


def yato_frame(k):
    _, mx, side, L, hw, ang, amp = YATO[k]
    foot = north_foot(mx) if side > 0 else south_foot(mx)
    m = (float(mx), float(vc(mx)) + side * float(foot))
    a = math.radians(ang)
    return m, (math.sin(a), side * math.cos(a))


def yato_point(k, t):
    m, d = yato_frame(k)
    amp = YATO[k][6]
    lat = amp * math.sin(t / 110)
    return (m[0] + d[0] * t - d[1] * lat, m[1] + d[1] * t + d[0] * lat)


def yato_carve(x, y):
    x, y = A(x), A(y)
    c = np.zeros(np.broadcast(x, y).shape)
    rise = np.zeros_like(c)
    for k, (_, mx, side, L, hw, ang, amp) in enumerate(YATO):
        m, d = yato_frame(k)
        rx, ry = x - m[0], y - m[1]
        t = rx * d[0] + ry * d[1]
        lat = rx * -d[1] + ry * d[0] - amp * np.sin(t / 110)
        w = hw * 1.25 * (1 + 1.4 * np.exp(-np.maximum(t, 0) / 80))
        ck = np.exp(-(lat / w) ** 4) * sstep((t + 40) / 40) * sstep((L - t) / 90)
        c = np.maximum(c, ck)
        rise = np.maximum(rise, ck * np.maximum(t, 0) * 0.03)
    return c, rise


def height(x, y):
    x, y = A(x), A(y)
    d = y - vc(x)
    n = fbm(x, y, 260, 7)
    north = sstep((d - north_foot(x)) / 200) * (55 + 25 * n)
    south = sstep((-d - south_foot(x)) / 190) * (45 + 20 * n)
    hills = north + south
    bumps = fbm(x, y, 90, 3) * 7 * sstep(hills / 8)
    carve, rise = yato_carve(x, y)
    return floor_h(x) + 0.5 * vnoise(x, y, 200, 1) + (hills + bumps) * (1 - 0.93 * carve) + rise * sstep(hills / 3)


def rel_height(x, y):
    return height(x, y) - floor_h(x)


def H(p):
    return float(height(p[0], p[1]))


def RH(p):
    return float(rel_height(p[0], p[1]))


def VC(x):
    return float(vc(x))


# lines along the valley (offsets from its centre line)
OLD_D = lambda x: float(north_foot(x)) - 9 + 3 * math.sin(x / 40)
RAIL_D = lambda x: 192 + 10 * math.sin(x / 900 + 0.5)
PREF_D = lambda x: 110 + 20 * math.sin(x / 450 + 1)
RIVER_D = lambda x: -70 + 55 * math.sin(x / 260 + 1) + 18 * math.sin(x / 97)
MINAMI_D = lambda x: -float(south_base(x)) + 8 + 2 * math.sin(x / 37)
SX = 60.0
SCHOOL_X = lambda y: SX + 4 * math.sin(y / 90)
RIVER_WATER, RIVER_LEVEE = 9.0, 22.0  # half widths

# ============================================================================= geometry


def v_sub(a, b):
    return (a[0] - b[0], a[1] - b[1])


def v_add(a, b):
    return (a[0] + b[0], a[1] + b[1])


def v_mul(a, s):
    return (a[0] * s, a[1] * s)


def v_len(a):
    return math.hypot(a[0], a[1])


def v_norm(a):
    l = v_len(a) or 1.0
    return (a[0] / l, a[1] / l)


def perp(a):
    return (-a[1], a[0])


def neg(a):
    return (-a[0], -a[1])


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1]


def rect(o, e, n, a0, a1, b0, b1):
    """Rectangle in a frame (origin o, unit axes e, n): a along e, b along n."""
    return [v_add(o, v_add(v_mul(e, a), v_mul(n, b))) for a, b in ((a0, b0), (a1, b0), (a1, b1), (a0, b1))]


def poly_area(p):
    return 0.5 * abs(sum(p[i][0] * p[i - 1][1] - p[i - 1][0] * p[i][1] for i in range(len(p))))


def poly_center(p):
    return (sum(q[0] for q in p) / len(p), sum(q[1] for q in p) / len(p))


def bbox(p):
    xs, ys = [q[0] for q in p], [q[1] for q in p]
    return min(xs), min(ys), max(xs), max(ys)


def convex_overlap(p, q, eps=0.05):
    for poly in (p, q):
        for i in range(len(poly)):
            ax = v_norm(perp(v_sub(poly[i], poly[i - 1])))
            pa = [dot(ax, v) for v in p]
            qa = [dot(ax, v) for v in q]
            if max(pa) <= min(qa) + eps or max(qa) <= min(pa) + eps:
                return False
    return True


def point_in_convex(pt, poly):
    sign = 0
    for i in range(len(poly)):
        a, b = poly[i - 1], poly[i]
        c = (b[0] - a[0]) * (pt[1] - a[1]) - (b[1] - a[1]) * (pt[0] - a[0])
        if c != 0:
            s = 1 if c > 0 else -1
            if sign == 0:
                sign = s
            elif s != sign:
                return False
    return True


def seg_point_dist(a, b, p):
    ab = v_sub(b, a)
    t = min(1.0, max(0.0, dot(v_sub(p, a), ab) / max(dot(ab, ab), 1e-9)))
    return v_len(v_sub(p, v_add(a, v_mul(ab, t))))


def seg_intersect(a, b, c, d):
    o = lambda p, q, r: (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    return (o(a, b, c) > 0) != (o(a, b, d) > 0) and (o(c, d, a) > 0) != (o(c, d, b) > 0)


def seg_poly_dist(a, b, poly):
    if point_in_convex(a, poly) or point_in_convex(b, poly):
        return 0.0
    best = 1e9
    for i in range(len(poly)):
        c, d = poly[i - 1], poly[i]
        if seg_intersect(a, b, c, d):
            return 0.0
        best = min(best, seg_point_dist(c, d, a), seg_point_dist(c, d, b), seg_point_dist(a, b, c))
    return best


def shrink(poly, m):
    c = poly_center(poly)
    out = []
    for q in poly:
        d = v_sub(q, c)
        l = v_len(d)
        out.append(v_add(c, v_mul(d, max(0.0, (l - m) / l))) if l > 0 else q)
    return out


class Grid:
    def __init__(self, cell=50.0):
        self.cell, self.cells = cell, {}

    def _keys(self, x0, y0, x1, y1):
        c = self.cell
        for i in range(int(math.floor(x0 / c)), int(math.floor(x1 / c)) + 1):
            for j in range(int(math.floor(y0 / c)), int(math.floor(y1 / c)) + 1):
                yield (i, j)

    def add(self, box, item):
        for k in self._keys(*box):
            self.cells.setdefault(k, []).append(item)

    def query(self, box):
        seen = set()
        for k in self._keys(*box):
            for it in self.cells.get(k, ()):
                if id(it) not in seen:
                    seen.add(id(it))
                    yield it


class Road:
    def __init__(self, name, kind, width, pts):
        self.name, self.kind, self.width = name, kind, width
        self.pts = [(float(x), float(y)) for x, y in pts]

    def segs(self):
        return list(zip(self.pts[:-1], self.pts[1:]))

    def length(self):
        return sum(v_len(v_sub(b, a)) for a, b in self.segs())

    def at(self, s):
        segs = self.segs()
        for i, (a, b) in enumerate(segs):
            l = v_len(v_sub(b, a))
            if s <= l or i == len(segs) - 1:
                t = min(1.0, max(0.0, s / l)) if l > 0 else 0.0
                return v_add(a, v_mul(v_sub(b, a), t)), v_norm(v_sub(b, a))
            s -= l
        return self.pts[-1], v_norm(v_sub(self.pts[-1], self.pts[-2]))

    def nearest(self, p):
        best, bs, s = 1e18, 0.0, 0.0
        for a, b in self.segs():
            ab = v_sub(b, a)
            l = v_len(ab)
            t = min(1.0, max(0.0, dot(v_sub(p, a), ab) / max(l * l, 1e-9)))
            d = v_len(v_sub(p, v_add(a, v_mul(ab, t))))
            if d < best:
                best, bs = d, s + t * l
            s += l
        return best, bs


def valley_line(d_of_x, xa, xb, step=10.0):
    n = max(2, int(abs(xb - xa) / step) + 1)
    return [(xa + (xb - xa) * i / (n - 1), VC(xa + (xb - xa) * i / (n - 1)) + d_of_x(xa + (xb - xa) * i / (n - 1))) for i in range(n)]


def vquad(xa, xb, da, db):
    return [(xa, VC(xa) + da), (xb, VC(xb) + da), (xb, VC(xb) + db), (xa, VC(xa) + db)]


def wander(start, heading, length, rng, bias, step=8.0, max_rel=13.0, avoid=None):
    """A lane taking the easy line over the land, the way old lanes do."""
    pts = [start]
    p, h = start, heading
    for _ in range(int(length / step)):
        best = None
        for dh in (-0.35, -0.15, 0.0, 0.15, 0.35):
            nh = v_norm(v_add(h, v_mul(perp(h), dh)))
            q = v_add(p, v_mul(nh, step))
            grade = abs(H(q) - H(p)) / step
            cost = grade * 4 + abs(dh) * 0.3 - dot(nh, bias) * 0.2 + rng.random() * 0.12
            if best is None or cost < best[0]:
                best = (cost, nh, q)
        _, h, q = best
        if RH(q) > max_rel or (avoid and avoid(q)):
            break
        pts.append(q)
        p = q
    return pts


# ============================================================================= the plan

STYLE = {
    #             frontage      depth         gap   house type weights
    "shotengai": ((7.0, 10.0), (14.0, 20.0), 0.03, {"shophouse": 1}),
    "centre": ((10.0, 15.0), (15.0, 21.0), 0.06, {"modern_gable": 4, "modern_hip": 2, "small_house": 3, "apartment": 1}),
    "danchi": ((14.0, 16.0), (17.0, 19.0), 0.02, {"modern_gable": 4, "modern_hip": 4, "small_house": 1}),
    "farm": ((18.0, 32.0), (22.0, 34.0), 0.10, {"farmhouse": 5, "modern_gable": 3, "modern_hip": 2, "small_house": 1}),
    "mixed": ((14.0, 26.0), (19.0, 28.0), 0.08, {"farmhouse": 2, "modern_gable": 4, "modern_hip": 3, "small_house": 2}),
    "scatter": ((18.0, 30.0), (22.0, 32.0), 0.88, {"farmhouse": 3, "modern_gable": 2, "modern_hip": 1}),
    "ribbon": ((16.0, 24.0), (20.0, 26.0), 0.80, {"modern_gable": 3, "modern_hip": 2, "small_house": 1}),
}

# hamlets: name, yato index (None: placed by position), style, lanes up, lanes down, radius
HAMLETS = [
    ("Kami", 0, "farm", 4, 2, 230),
    ("Yamanoue", 1, "farm", 2, 0, 170),
    ("Higashi", 3, "mixed", 5, 2, 230),
    ("Shimo", 4, "farm", 4, 2, 210),
    ("Minami-nishi", 5, "farm", 4, 2, 220),
    ("Minami", 6, "mixed", 4, 2, 210),
    ("Minami-higashi", 7, "mixed", 4, 2, 220),
    ("Oku", 8, "farm", 4, 2, 200),
    ("Nishi", 9, "mixed", 3, 1, 150),
    ("Hazama", 10, "farm", 3, 1, 180),
]

HOUSES = {  # (long side, short side, floors, roof)
    "modern_gable": (10.0, 7.5, 2, "gable"),
    "modern_hip": (9.5, 8.0, 2, "hip"),
    "small_house": (8.0, 6.5, 1, "gable"),
    "farmhouse": (15.0, 10.0, 1, "irimoya"),
    "apartment": (16.0, 8.0, 2, "gable"),
    "shophouse": (0.0, 0.0, 2, "gable"),
}

# land-use raster codes
FREE, ROAD, PLOT, SITE, WATER, PADDY, HATAKE = 0, 1, 2, 3, 4, 5, 6


def _rects_hit(a, b):
    return b is not None and a[0] < b[1] and b[0] < a[1] and a[2] < b[3] and b[2] < a[3]


class Plan:
    def __init__(self):
        self.rng = np.random.default_rng(SEED)
        self.roads, self.sites, self.plots, self.paddies, self.hatake, self.trees, self.poles = [], [], [], [], [], [], []
        self.blocked = Grid()
        self.road_grid = Grid(40.0)
        self.hamlet_of = {}

    # --------------------------------------------------------------- occupancy
    def add_road(self, r):
        self.roads.append(r)
        for a, b in r.segs():
            w = r.width
            self.road_grid.add((min(a[0], b[0]) - w, min(a[1], b[1]) - w, max(a[0], b[0]) + w, max(a[1], b[1]) + w), (a, b, r))
        return r

    def block(self, poly, tag):
        self.blocked.add(bbox(poly), (poly, tag))

    def poly_free(self, poly, road_margin=0.4):
        for q, tag in self.blocked.query(bbox(poly)):
            if convex_overlap(poly, q):
                return False
        x0, y0, x1, y1 = bbox(poly)
        for a, b, r in self.road_grid.query((x0 - 10, y0 - 10, x1 + 10, y1 + 10)):
            if seg_poly_dist(a, b, poly) < r.width / 2 + road_margin:
                return False
        return True

    def near_road(self, p, extra=0.0, ignore=None):
        for a, b, r in self.road_grid.query((p[0] - 14, p[1] - 14, p[0] + 14, p[1] + 14)):
            if r is not ignore and seg_point_dist(a, b, p) < r.width / 2 + extra:
                return True
        return False

    def occupied(self, p):
        for q, tag in self.blocked.query((p[0], p[1], p[0], p[1])):
            if point_in_convex(p, q):
                return tag
        return None

    # --------------------------------------------------------------- 1. the trunk network
    def network(self):
        self.river = valley_line(RIVER_D, X0, X1, 10)
        self.rail = Road("JR Kazehana line", "rail", 5.0, valley_line(RAIL_D, X0, X1, 15))
        self.old = self.add_road(Road("old road (kyūdō)", "old", 5.0, valley_line(OLD_D, X0, X1, 8)))
        self.pref = self.add_road(Road("prefectural road 32", "pref", 7.0, valley_line(PREF_D, X0, X1, 20)))
        self.minami = self.add_road(Road("Minami road", "old", 4.5, valley_line(MINAMI_D, X0, X1, 8)))
        ya, yb = VC(SX) + OLD_D(SX) - 2.5, VC(SX) + MINAMI_D(SX) + 2.25
        n = int((ya - yb) / 8)
        self.school = self.add_road(Road("station street / school road", "school", 6.0, [(SCHOOL_X(ya - (ya - yb) * i / n), ya - (ya - yb) * i / n) for i in range(n + 1)]))
        self.add_road(Road("river levee path", "levee", 3.5, valley_line(lambda x: RIVER_D(x) + RIVER_LEVEE - 3, -1350, 1350, 10)))
        # bridges: Kami bridge (west) and Shimo bridge (east), each a lane from the old road across the floor
        for name, x in (("Kami bridge road", -760.0), ("Shimo bridge road", 820.0)):
            ya, yb = VC(x) + OLD_D(x) - 2.5, VC(x) + MINAMI_D(x) + 2.25
            self.add_road(Road(name, "lane", 5.0, [(x + 6 * math.sin(i / 4), ya - (ya - yb) * i / 50) for i in range(51)]))

    def centre_streets(self):
        """The station quarter: back street, station-front street, south-exit street, cross streets."""
        self.centre = []
        add = lambda r: self.centre.append(self.add_road(r))
        add(Road("ura-dōri (back street)", "street", 4.5, valley_line(lambda x: OLD_D(x) - 45, -280, SCHOOL_X(0) - 3.0, 8)))
        add(Road("ura-dōri east", "street", 4.5, valley_line(lambda x: OLD_D(x) - 45, SCHOOL_X(0) + 3.0, 330, 8)))
        add(Road("station front street", "street", 5.0, valley_line(lambda x: RAIL_D(x) + 14, -230, SCHOOL_X(0) - 3.0, 8)))
        add(Road("station front street east", "street", 5.0, valley_line(lambda x: RAIL_D(x) + 14, SCHOOL_X(0) + 3.0, 250, 8)))
        add(Road("south exit street", "street", 4.5, valley_line(lambda x: RAIL_D(x) - 36, -300, SCHOOL_X(0) - 3.0, 8)))
        add(Road("Hikarigaoka danchi lane", "street", 4.5, valley_line(lambda x: RAIL_D(x) - 40, SCHOOL_X(0) + 3.0, 560, 8)))
        add(Road("ue-dōri (upper street)", "street", 4.0, valley_line(lambda x: OLD_D(x) + 32, -190, 270, 8)))
        for x in (-190.0, 270.0, 40.0):
            d0, d1 = OLD_D(x) + 2.5, OLD_D(x) + 32
            add(Road(f"slope lane {int(x)}", "street", 4.0, [(x + 1.5 * math.sin(i), VC(x) + d0 + (d1 - d0) * i / 4) for i in range(5)]))
        for x in (-95.0, 170.0):
            d0, d1 = RAIL_D(x) + 16.5, OLD_D(x) - 2.5
            n = max(2, int((d1 - d0) / 8))
            add(Road(f"alley {int(x)}", "street", 4.0, [(x + 1.5 * math.sin(i / 2), VC(x) + d0 + (d1 - d0) * i / n) for i in range(n + 1)]))
        for x, d0, d1 in ((-200.0, PREF_D(-200) + 3.5, OLD_D(-200) - 2.5), (250.0, RAIL_D(250) + 16.5, OLD_D(250) - 2.5), (560.0, PREF_D(560) + 3.5, RAIL_D(560) - 42.25), (330.0, RAIL_D(330) - 38, PREF_D(330) + 3.5)):
            n = max(2, int((d1 - d0) / 8))
            add(Road(f"cross street {int(x)}", "street", 4.5, [(x + 2.5 * math.sin(i / 3), VC(x) + d0 + (d1 - d0) * i / n) for i in range(n + 1)]))

    # --------------------------------------------------------------- 2. sites
    def site(self, name, kind, poly, buildings=()):
        s = {"name": name, "kind": kind, "poly": poly, "buildings": list(buildings)}
        self.sites.append(s)
        self.block(poly, name)
        return s

    def civic(self):
        for x in np.arange(X0, X1, 20.0):
            x = float(x)
            self.block(vquad(x, x + 20, RIVER_D(x + 10) - RIVER_LEVEE, RIVER_D(x + 10) + RIVER_LEVEE), "river")
            self.block(vquad(x, x + 20, RAIL_D(x + 10) - 3.5, RAIL_D(x + 10) + 3.5), "railway")
        # the junior high in its bay under the south hills, gate on the Minami road
        top = MINAMI_D(SX) - 2.25 - 1.0
        sch = vquad(-100, 220, top - 137, top)
        o = sch[3]
        e = v_norm(v_sub(sch[2], sch[3]))
        s_ = neg(perp(e))
        self.school_site = self.site("Kazehana Junior High School", "school", sch, [
            {"name": "school building", "poly": rect(o, e, s_, 8, 88, 10, 26), "floors": 3},
            {"name": "gym", "poly": rect(o, e, s_, 250, 290, 10, 42), "floors": 1},
            {"name": "pool", "poly": rect(o, e, s_, 250, 275, 50, 66), "floors": 0},
            {"name": "sports ground", "poly": rect(o, e, s_, 12, 220, 40, 128), "floors": 0},
        ])
        self.school_gate = (SCHOOL_X(VC(SX) + top), VC(SX) + top - 4.0)
        self.site("Kazehana Elementary School", "school", vquad(-520, -380, PREF_D(-450) + 8, RAIL_D(-450) - 8))
        self.site("Kazehana Station", "station", vquad(-45, 40, RAIL_D(0) + 4, RAIL_D(0) + 11))
        self.site("station plaza", "plaza", vquad(-30, 25, RAIL_D(0) + 17, RAIL_D(0) + 40))
        self.site("Hachiman shrine", "shrine", vquad(305, 360, OLD_D(330) + 3.5, OLD_D(330) + 95))
        m, d = yato_frame(2)
        self.site("Shōrin-ji temple and cemetery", "temple", vquad(-305, -215, OLD_D(-260) + 3.5, OLD_D(-260) + 75))
        self.site("post office", "civic", vquad(-130, -105, OLD_D(-118) - 3.0, OLD_D(-118) - 21))
        self.site("town office branch", "civic", vquad(130, 175, OLD_D(152) + 3.0, OLD_D(152) + 28))
        ix = SCHOOL_X(0.0)
        self.site("Inari shrine", "inari", vquad(ix - 42, ix - 12, 20, 50))
        sx = SCHOOL_X(VC(SX) + PREF_D(SX))
        self.site("konbini", "konbini", vquad(sx + 6, sx + 40, PREF_D(sx) + 5, PREF_D(sx) + 36))
        self.site("JA farm co-op", "business", vquad(-170, -100, PREF_D(-135) - 40, PREF_D(-135) - 6))
        self.site("petrol station", "business", vquad(-60, -25, PREF_D(-45) - 32, PREF_D(-45) - 6))
        self.site("drugstore", "business", vquad(150, 200, PREF_D(175) - 45, PREF_D(175) - 6))
        self.site("car repair", "business", vquad(-330, -295, PREF_D(-310) - 30, PREF_D(-310) - 6))

    # --------------------------------------------------------------- 3. hamlet lanes
    def hamlet_lanes(self):
        rng = self.rng
        avoid = lambda q: self.occupied(q) is not None or self.near_road(q, 7.0)
        self.hamlets = []
        for name, k, style, n_up, n_down, radius in HAMLETS:
            m, d = yato_frame(k)
            side = YATO[k][2]
            road = self.old if side > 0 else self.minami
            _, s_m = road.nearest(m)
            start, _ = road.at(s_m)
            # the yato lane: up the side valley along its floor
            L = YATO[k][3] * (0.9 if name == "Yamanoue" else 0.6)
            pts = [start] + [yato_point(k, t) for t in np.arange(12.0, L, 10.0)]
            main = self.add_road(Road(f"{name} yato lane", "lane", 4.0, pts))
            lanes = [main]
            # lanes into the hill foot either side of the mouth, and down towards the paddies
            offs = list(rng.uniform(45, radius * 0.9, n_up) * np.resize([-1, 1], n_up)) + list(rng.uniform(25, radius * 0.7, n_down) * np.resize([1, -1], n_down))
            for i, off in enumerate(offs):
                up = i < n_up
                p0, t = road.at(s_m + off)
                bias = (0.0, float(side)) if up else (0.0, -float(side))
                p0 = v_add(p0, v_mul(bias, road.width / 2))
                first = v_add(p0, v_mul(bias, 8.0))
                if avoid(first):
                    continue
                pts = [p0] + wander(first, bias, rng.uniform(55, 140) if up else rng.uniform(35, 80), rng, bias, max_rel=14.0 if up else 2.5, avoid=avoid)
                if len(pts) >= 5:
                    lanes.append(self.add_road(Road(f"{name} lane {i}", "lane", 4.0, pts)))
            # a loop: join the ends of two uphill lanes when they are close (lanes knot together)
            ups = [l for l in lanes[1:] if l.pts[-1] and dot(v_sub(l.pts[-1], l.pts[0]), (0, side)) > 20]
            for a in ups:
                for b in ups:
                    if a is b:
                        continue
                    pa, pb = a.pts[min(len(a.pts) - 1, 5)], b.pts[min(len(b.pts) - 1, 5)]
                    dd = v_len(v_sub(pb, pa))
                    if 30 < dd < 110 and pa[0] < pb[0]:
                        mid = v_add(v_mul(v_add(pa, pb), 0.5), (0.0, side * rng.uniform(4, 14)))
                        link = [pa, v_add(pa, v_mul(v_sub(mid, pa), 0.5)), mid, v_add(mid, v_mul(v_sub(pb, mid), 0.5)), pb]
                        if all(not self.occupied(q) for q in link) and max(H(q) for q in link) - min(H(q) for q in link) < 6:
                            lanes.append(self.add_road(Road(f"{name} loop", "lane", 4.0, link)))
                            break
            self.hamlets.append({"name": name, "style": style, "centre": m, "radius": radius, "road": road, "s": s_m, "lanes": lanes, "yato": k})

    # --------------------------------------------------------------- 4. plots
    def make_lot(self, road, s, side, w, depth, district):
        p0, _ = road.at(s)
        p1, _ = road.at(s + w)
        e = v_norm(v_sub(p1, p0))
        if side < 0:  # keep the frame right-handed (b always points away from the road)
            p0, e = p1, neg(e)
        n = perp(e)
        o = v_add(p0, v_mul(n, road.width / 2 + 0.5))
        return {"road": road, "district": district, "poly": rect(o, e, n, 0, w, 0, depth), "frame": (o, e, n, w, depth)}

    def special_plots(self):
        """Tsukumodō, Uncle Kenji's game and card shop (and home): on the station street, west side,
        between the old road and the back street. Every kid walks past it to and from school."""
        r = self.school
        s = r.nearest((SCHOOL_X(0), VC(SX) + OLD_D(SX) - 24))[1]
        lot = self.make_lot(r, s, -1, 10.0, 17.0, "Centre")
        lot["style"], lot["force"], lot["name"], lot["kind"] = "shotengai", "shophouse", "Tsukumodō", "shop_home"
        lot["retaining"] = 0.0
        assert self.poly_free(lot["poly"]) and self.layout(lot)
        self.plots.append(lot)
        self.block(lot["poly"], "plot")
        self.home = lot

    def lots_along(self, road, side, district, style, s0=0.0, s1=None, keep=None):
        rng = self.rng
        fw, fd, gap_p, _ = STYLE[style]
        s1 = road.length() - 3 if s1 is None else s1
        s = max(0.0, s0) + rng.uniform(0, 5)
        while s < s1 - fw[0]:
            if rng.random() < gap_p:
                s += rng.uniform(8, 35)  # a vegetable plot, an empty lot, a paddy reaching the road
                continue
            w = rng.uniform(*fw)
            force = None
            if style == "centre" and rng.random() < 0.12:
                w, force = rng.uniform(24, 27), "apartment"  # a two-storey apaato block
            placed = False
            tries = [(w, rng.uniform(*fd)), (w, fd[0]), (w * 0.8, fd[0]), (w, fd[0] * 0.85)]
            for ww, depth in tries:
                lot = self.make_lot(road, s, side, ww, depth, district)
                lot["style"], lot["force"] = style, force
                c = poly_center(lot["poly"])
                if keep and not keep(c):
                    break
                hs = [H(q) for q in lot["poly"]] + [H(c)]
                # on the hill foot, plots are cut into level platforms behind retaining walls
                if max(hs) - min(hs) > 6.5 or RH(c) > 20.0:
                    continue
                if not self.poly_free(lot["poly"]):
                    continue
                if self.layout(lot) and road.nearest(lot["gate"])[0] <= road.width / 2 + 1.5:
                    lot["retaining"] = round(max(hs) - min(hs), 1) if max(hs) - min(hs) > 2.0 else 0.0
                    self.plots.append(lot)
                    self.block(lot["poly"], "plot")
                    placed, w = True, ww
                    break
            s += w + (rng.uniform(0.0, 1.2) if placed else 3.0)

    def plots_all(self):
        # the station quarter
        sch = self.school
        rail_s = sch.nearest((SCHOOL_X(VC(SX) + RAIL_D(SX)), VC(SX) + RAIL_D(SX)))[1]
        for side in (-1, 1):
            self.lots_along(sch, side, "Centre", "shotengai", 0, rail_s - 18)
        on_old = lambda x: self.old.nearest((x, VC(x) + OLD_D(x)))[1]
        for side in (-1, 1):
            self.lots_along(self.old, side, "Centre", "shotengai", on_old(-200), on_old(260))
            self.lots_along(self.old, side, "Centre", "centre", on_old(-330), on_old(-200))
            self.lots_along(self.old, side, "Centre", "centre", on_old(260), on_old(420))
        for r in self.centre:
            style = "danchi" if "danchi" in r.name else "centre"
            for side in (-1, 1):
                self.lots_along(r, side, "Centre", style, 3)
        # south of the railway, the school road gets houses up to the prefectural road
        pref_s = sch.nearest((SCHOOL_X(VC(SX) + PREF_D(SX)), VC(SX) + PREF_D(SX)))[1]
        for side in (-1, 1):
            self.lots_along(sch, side, "Centre", "centre", rail_s + 8, pref_s - 6)
        # hamlets: the trunk road through the hamlet, then its lanes
        for h in self.hamlets:
            c, R = h["centre"], h["radius"]
            within = lambda p, c=c, R=R: v_len(v_sub(p, c)) < R
            for side in (-1, 1):
                self.lots_along(h["road"], side, h["name"], h["style"], h["s"] - R, h["s"] + R, keep=within)
            for lane in h["lanes"]:
                keep = None if h["name"] == "Yamanoue" else (lambda p, c=c, R=R: v_len(v_sub(p, c)) < R * 1.25)
                for side in (-1, 1):
                    self.lots_along(lane, side, h["name"], h["style"], 6, None, keep=keep)
        # the odd house between hamlets, and along the prefectural road and the bridge roads
        for side in (-1, 1):
            self.lots_along(self.old, side, "Scattered", "scatter")
            self.lots_along(self.minami, side, "Scattered", "scatter")
            self.lots_along(self.pref, side, "Roadside", "ribbon", keep=lambda c: -1000 < c[0] < 1000)
        for r in self.roads:
            if r.name.endswith("bridge road"):
                for side in (-1, 1):
                    self.lots_along(r, side, "Kawabata", "mixed", 4, None, keep=lambda c: abs(c[1] - VC(c[0]) - PREF_D(c[0])) < 60)

    # --------------------------------------------------------------- house layout
    def layout(self, lot):
        """Place the house (garden to the south), parking on the road, entrance facing the road."""
        rng = self.rng
        o, e, n, w, D = lot["frame"]
        if lot.get("force"):
            kinds = [lot["force"]]
        else:
            weights = STYLE[lot["style"]][3]
            ks = list(weights)
            p = np.array([weights[k] for k in ks], float)
            first = ks[int(rng.choice(len(ks), p=p / p.sum()))]
            kinds = [first] + ([] if first == "shophouse" else [k for k in ("modern_gable", "small_house") if k != first])
        sides = {"front": neg(n), "back": n, "right": e, "left": neg(e)}
        garden = min(sides, key=lambda k: sides[k][1])  # the plot side facing most south
        for kind in kinds:
            g = "back" if kind == "shophouse" else garden  # shops face the street whatever the sun does
            lay = self._fit(kind, w, D, g)
            if lay:
                garden = g
                break
        else:
            return False
        house, parking, shed, ent_a = lay
        R = lambda r: rect(o, e, n, *r) if r else None
        lot.setdefault("kind", "house")
        lot["type"] = kind
        lot["house"], lot["parking"], lot["shed"] = R(house), R(parking), R(shed)
        lot["garden_side"] = garden
        lot["floors"], lot["roof"] = HOUSES[kind][2], HOUSES[kind][3]
        lot["households"] = 4 if kind == "apartment" else 1
        lot["entrance"] = v_add(o, v_add(v_mul(e, ent_a), v_mul(n, house[2])))
        lot["gate"] = v_add(o, v_mul(e, ent_a))
        lot["yaw"] = math.atan2(e[1], e[0])
        lot["front"] = "open" if kind == "shophouse" else str(rng.choice(["block", "hedge", "hedge", "open"] if kind != "farmhouse" else ["stone", "hedge", "hedge"]))
        return True

    def _fit(self, kind, w, D, garden):
        rng = self.rng
        if kind == "shophouse":
            hd = min(D - 3.0, rng.uniform(10, 13))
            if w < 6.5 or hd < 8:
                return None
            return ((0.4, w - 0.4, 0.3, 0.3 + hd), None, None, w / 2)
        L, S, _, _ = HOUSES[kind]
        if kind == "apartment":
            if w < L + 7.5 or D < S + 8:
                return None
            return ((w - 1.0 - L, w - 1.0, 2.0, 2.0 + S), (0.5, 6.0, 0.3, 5.8), None, w - 1.0 - L * 0.5)
        hw, hd = (L, S) if garden in ("front", "back") else (S, L)  # long side faces the garden
        if garden == "front":  # road to the south: garden in front, house at the back
            if D < hd + 1.2 + 8.5 or w < hw + 1.6:
                return None
            a0 = 0.8 if rng.random() < 0.5 else w - 0.8 - hw
            house = (a0, a0 + hw, D - 1.2 - hd, D - 1.2)
            pa = (w - 6.0, w - 0.5) if a0 < w / 2 else (0.5, 6.0)
            parking = (pa[0], pa[1], 0.3, 5.8)
            ent = a0 + hw * 0.3
        elif garden == "back":  # road to the north: house near the road, garden behind
            if D < hd + 2.0 + 4.0 or w < hw + 7.5:
                return None
            house = (w - 1.0 - hw, w - 1.0, 2.0, 2.0 + hd)
            parking = (0.5, 6.0, 0.3, 5.8)
            ent = w - 1.0 - hw * 0.7
        else:  # garden on one side
            if w < hw + 1.0 + 7.0 or D < hd + 3.0:
                return None
            if garden == "left":
                a0, parking = w - 1.0 - hw, (0.5, 6.0, 0.3, 5.8)
            else:
                a0, parking = 1.0, (w - 6.0, w - 0.5, 0.3, 5.8)
            house = (a0, a0 + hw, 1.8, 1.8 + hd)
            ent = a0 + hw / 2
        shed = None
        if kind == "farmhouse":  # the work yard in front, a shed / storehouse along a side
            sw, sd = 8.0, 5.0
            for a0 in (0.8, w - 0.8 - sd):
                for cand in ((a0, a0 + sd, D - 0.8 - sw, D - 0.8), (a0, a0 + sd, 6.5, 6.5 + sw)):
                    if not _rects_hit(cand, house) and not _rects_hit(cand, parking) and cand[2] >= 0.3 and cand[3] <= D - 0.3:
                        shed = cand
                        break
                if shed:
                    break
        return (house, parking, shed, ent)

    # --------------------------------------------------------------- 5. land-use raster
    def rasterize(self):
        from PIL import Image, ImageDraw
        self.res = 1.0
        W, Hh = int((X1 - X0) / self.res), int((Y1 - Y0) / self.res)
        im = Image.new("L", (W, Hh), FREE)
        d = ImageDraw.Draw(im)
        T = lambda p: ((p[0] - X0) / self.res, (Y1 - p[1]) / self.res)
        d.line([T(p) for p in self.river], fill=WATER, width=int(2 * RIVER_LEVEE / self.res))
        d.line([T(p) for p in self.rail.pts], fill=WATER, width=int(8 / self.res))
        for s in self.sites:
            d.polygon([T(p) for p in s["poly"]], fill=SITE)
        for l in self.plots:
            d.polygon([T(p) for p in l["poly"]], fill=PLOT)
        for q in self.hatake:
            d.polygon([T(p) for p in q], fill=HATAKE)
        for q in self.paddies:
            d.polygon([T(p) for p in q], fill=PADDY)
        for r in self.roads:
            d.line([T(p) for p in r.pts], fill=ROAD, width=max(1, int((r.width + 1.5) / self.res)), joint="curve")
        self.land = np.array(im)

    def land_at(self, xs, ys):
        xs, ys = A(xs), A(ys)
        i = np.clip(((Y1 - ys) / self.res).astype(int), 0, self.land.shape[0] - 1)
        j = np.clip(((xs - X0) / self.res).astype(int), 0, self.land.shape[1] - 1)
        return self.land[i, j]

    def quad_free(self, q, max_rel=2.2, flat=None):
        pts = []
        for u in (0.04, 0.3, 0.5, 0.7, 0.96):
            for v in (0.06, 0.5, 0.94):
                a = v_add(q[0], v_mul(v_sub(q[1], q[0]), u))
                b = v_add(q[3], v_mul(v_sub(q[2], q[3]), u))
                pts.append(v_add(a, v_mul(v_sub(b, a), v)))
        xs, ys = np.array([p[0] for p in pts]), np.array([p[1] for p in pts])
        if np.any(self.land_at(xs, ys) != FREE):
            return False
        hs = height(xs, ys)
        if flat is not None:
            return float(hs.max() - hs.min()) < flat
        return float((hs - floor_h(xs)).max()) < max_rel

    def stamp(self, q, code):
        from PIL import Image, ImageDraw
        x0, y0, x1, y1 = bbox(q)
        i0, i1 = int((Y1 - y1) / self.res) - 1, int((Y1 - y0) / self.res) + 2
        j0, j1 = int((x0 - X0) / self.res) - 1, int((x1 - X0) / self.res) + 2
        i0, j0 = max(i0, 0), max(j0, 0)
        sub = Image.fromarray(self.land[i0:i1, j0:j1])
        ImageDraw.Draw(sub).polygon([((p[0] - X0) / self.res - j0, (Y1 - p[1]) / self.res - i0) for p in q], fill=code)
        self.land[i0:i1, j0:j1] = np.array(sub)

    # --------------------------------------------------------------- 6. fields
    def kitchen_gardens(self):
        """Vegetable plots (hatake) beside and behind hamlet houses."""
        rng = self.rng
        for lot in self.plots:
            if lot["style"] not in ("farm", "mixed", "scatter") or rng.random() > 0.55:
                continue
            o, e, n, w, D = lot["frame"]
            for (a0, a1, b0, b1) in ((0, w, D + 0.8, D + rng.uniform(10, 18)), (w + 0.8, w + rng.uniform(8, 14), 0.5, D), (-rng.uniform(8, 14), -0.8, 0.5, D)):
                q = rect(o, e, n, a0, a1, b0, b1)
                if self.quad_free(q, flat=2.0):
                    self.hatake.append(q)
                    self.stamp(q, HATAKE)
                    break

    def fields(self):
        # consolidated paddies on the floor: ~100 x 30 m, long side along the valley, a farm road
        # every 100 m (offset differently in each band so they do not line up across the river);
        # the edges are filled with smaller, older paddies
        bands = [
            (lambda x: MINAMI_D(x) + 4.0, lambda x: RIVER_D(x) - RIVER_LEVEE - 1.0, 0.0),
            (lambda x: RIVER_D(x) + RIVER_LEVEE + 1.0, lambda x: PREF_D(x) - 5.0, 37.0),
            (lambda x: PREF_D(x) + 5.0, lambda x: RAIL_D(x) - 4.5, 71.0),
            (lambda x: RAIL_D(x) + 4.5, lambda x: OLD_D(x) - 3.5, 13.0),
        ]
        for lo_f, hi_f, off in bands:
            x = X0 + off
            while x < X1 - 30:
                xw = float(self.rng.uniform(75, 125))
                # the farm road on this column's west edge, where the land is free
                run = []
                lo, hi = lo_f(x), hi_f(x)
                for dd in np.arange(lo - 2, hi + 2, 4.0):
                    p = (x, VC(x) + float(dd))
                    ok = self.land_at(p[0], p[1]) in (FREE,) and RH(p) < 2.5
                    if ok:
                        run.append(p)
                    if (not ok or dd + 4 >= hi + 2) and len(run) >= 8:
                        r = self.add_road(Road(f"farm road {int(x)}", "farm", 3.0, run))
                        self.stamp(_road_quad(r.pts[0], r.pts[-1], 2.2), ROAD)
                        run = []
                    elif not ok:
                        run = []
                xm = x + xw / 2
                lo, hi = lo_f(xm), hi_f(xm)
                d = lo
                while d < hi - 8:
                    for dw in (30.0, 15.0):
                        if d + dw > hi + 0.5:
                            continue
                        cells = [(x + 2.0, x + xw - 0.3)]
                        done = []
                        while cells:
                            xa, xb = cells.pop()
                            q = vquad(xa, xb, d + 0.25, d + dw - 0.25)
                            if self.quad_free(q):
                                done.append(q)
                            elif xb - xa > 18:
                                mid = (xa + xb) / 2
                                cells += [(xa, mid - 0.25), (mid + 0.25, xb)]
                        for q in done:
                            self.paddies.append(q)
                            self.stamp(q, PADDY)
                        if done or dw == 15.0:
                            break
                    d += 30.0 if d + 30 <= hi else 15.0
                x += xw

    def terraces(self):
        """Terraced paddies (tanada) up the yato floors, either side of the yato lane."""
        for k, (name, mx, side, L, hw, ang, amp) in enumerate(YATO):
            for t in np.arange(30.0, L - 30, 13.0):
                p, p2 = yato_point(k, float(t)), yato_point(k, float(t) + 12.5)
                e = v_norm(v_sub(p2, p))
                n = perp(e)
                for b0, b1 in ((3.6, 3.6 + hw * 0.55), (-3.6 - hw * 0.55, -3.6), (3.6, 3.6 + hw * 0.3), (-3.6 - hw * 0.3, -3.6)):
                    q = rect(p, e, n, 0.0, 12.0, b0, b1)
                    if self.quad_free(q, flat=2.2):
                        self.paddies.append(q)
                        self.stamp(q, PADDY)

    # --------------------------------------------------------------- 7. poles
    def utility_poles(self):
        for r in self.roads:
            if r.kind in ("farm", "levee"):
                continue
            L, s = r.length(), 5.0
            while s < L:
                p, t = r.at(s)
                q = v_add(p, v_mul(perp(t), r.width / 2 + 0.45))
                if self.occupied(q) not in ("river", "railway"):
                    self.poles.append(q)
                s += 33.0

    # --------------------------------------------------------------- 8. trees
    def vegetation(self):
        rng = self.rng
        T = self.trees
        step = 5.0
        gx, gy = np.meshgrid(np.arange(X0, X1, step), np.arange(Y0, Y1, step))
        jx = gx + rng.uniform(-2.2, 2.2, gx.shape)
        jy = gy + rng.uniform(-2.2, 2.2, gy.shape)
        rel = rel_height(jx, jy)
        land = self.land_at(jx, jy)
        carve, _ = yato_carve(jx, jy)
        plant = fbm(jx, jy, 180, 41, 2)
        bamboo = fbm(jx, jy, 70, 77, 3)
        kind_n = fbm(jx, jy, 120, 91, 2)
        maple_n = fbm(jx, jy, 45, 123, 2)  # small maple stands along the valley sides
        rr_all = rng.random(gx.shape)
        # near a hamlet house? (bamboo and orchards grow behind the hamlets)
        hx = np.array([poly_center(l["poly"])[0] for l in self.plots])
        hy = np.array([poly_center(l["poly"])[1] for l in self.plots])
        near = np.zeros(gx.shape, bool)
        for cx, cy in zip(hx, hy):
            i0, i1 = int((cy - 60 - Y0) / step), int((cy + 60 - Y0) / step) + 1
            j0, j1 = int((cx - 60 - X0) / step), int((cx + 60 - X0) / step) + 1
            near[max(i0, 0):i1, max(j0, 0):j1] = True
        mask = (land == FREE) & ((rel >= 2.0) | (near & (rr_all < 0.05)))
        for x, y, h, pl, bb, bk, mn, rr, nr in zip(jx[mask], jy[mask], rel[mask], plant[mask], bamboo[mask], kind_n[mask], maple_n[mask], rr_all[mask], near[mask]):
            x, y = float(x), float(y)
            if h < 2.0:  # a lone tree on the floor near the houses: persimmon, chestnut
                T.append((x, y, "kaki" if rr < 0.03 else "kuri", 6 + 30 * rr))
            elif nr and h < 16 and bb > 0.18:  # bamboo groves behind the hamlets: dense, uneven
                n = 2 + int(rr * 3) + (2 if bb > 0.32 else 0)
                for _ in range(n):
                    T.append((x + rng.uniform(-2.5, 2.5), y + rng.uniform(-2.5, 2.5), "bamboo", 10.0 + 4.0 * rng.random()))
            elif pl > 0.15 and h > 10:  # cedar / cypress plantations, planted in rows
                sp = "sugi" if pl > 0.3 else "hinoki"
                bx, by = round(x / 3.5) * 3.5, round(y / 3.5) * 3.5
                T.append((bx, by, sp, 18 + 6 * rr))
                T.append((bx + 1.75, by + 1.75, sp, 17 + 6 * rr))
            else:  # the mixed broadleaf satoyama: closed canopy, in stands of one species
                kind = "konara" if bk > 0.15 else ("shii" if bk < -0.25 else "kunugi")
                if mn > 0.42:
                    kind = "momiji"  # a maple stand
                if rr < 0.012:
                    kind = "yamazakura"  # mountain cherry: a few scattered pale crowns in April
                T.append((x, y, kind, 11 + 8 * rr))
                if rr > 0.6 and kind != "yamazakura":  # young trees of the same stand underneath
                    T.append((x + 2.4, y - 1.7, kind, 5 + 3 * rr))
        # shrine, temple and Inari groves (a clearing in the middle for the shrine itself)
        for s in self.sites:
            if s["kind"] in ("shrine", "inari", "temple"):
                x0, y0, x1, y1 = bbox(s["poly"])
                inner = shrink(s["poly"], 2.0)
                c = poly_center(s["poly"])
                for _ in range(int(poly_area(s["poly"]) / (40 if s["kind"] != "temple" else 120))):
                    p = (rng.uniform(x0, x1), rng.uniform(y0, y1))
                    if point_in_convex(p, inner) and (s["kind"] == "temple" or v_len(v_sub(p, c)) > 8):
                        T.append((p[0], p[1], str(rng.choice(["sugi", "kusu", "keyaki", "sugi"])), 22 + 10 * rng.random()))
        # cherry trees round the school and along both levees near the school bridge
        sch = self.school_site["poly"]
        sc = poly_center(sch)
        for i in range(4):
            a, b = sch[i - 1], sch[i]
            L = v_len(v_sub(b, a))
            inward = v_norm(v_sub(sc, v_add(a, v_mul(v_sub(b, a), 0.5))))
            for j in range(1, int(L / 9)):
                p = v_add(v_add(a, v_mul(v_sub(b, a), j * 9 / L)), v_mul(inward, 3.0))
                if not any(point_in_convex(p, bl["poly"]) for bl in self.school_site["buildings"]) and abs(p[0] - SCHOOL_X(p[1])) > 8:
                    T.append((p[0], p[1], "someiyoshino", 9.0))
        for x in np.arange(-300.0, 420.0, 8.5):
            x = float(x)
            for off in (RIVER_LEVEE - 1.0, -(RIVER_LEVEE - 1.0)):
                p = (x, VC(x) + RIVER_D(x) + off)
                if abs(p[0] - SCHOOL_X(p[1])) > 9 and not self.near_road(p, 0.5):
                    T.append((p[0], p[1], "someiyoshino", 9.0))
        # houses: windbreak trees (yashikirin) on the north and west of farmhouses, garden trees
        for lot in self.plots:
            poly = lot["poly"]
            o, e, n, w, D = lot["frame"]
            c = poly_center(poly)
            if lot["type"] == "farmhouse":
                for i in range(4):
                    a, b = poly[i], poly[(i + 1) % 4]
                    out = perp(v_norm(v_sub(b, a)))
                    if dot(out, v_sub(c, a)) > 0:
                        out = neg(out)
                    if out[1] > 0.55 or out[0] < -0.6:
                        L = v_len(v_sub(b, a))
                        for j in range(1, int(L / 3.8)):
                            p = v_add(v_add(a, v_mul(v_sub(b, a), j * 3.8 / L)), v_mul(out, -1.4))
                            if not self._in_lot_parts(lot, p, 1.5):
                                T.append((p[0], p[1], str(rng.choice(["sugi", "kashi", "kashi", "sugi"])), 14 + 6 * rng.random()))
            n_garden = 0 if lot["type"] == "shophouse" else int(rng.integers(0, 4)) + (1 if lot["type"] == "farmhouse" else 0)
            for _ in range(n_garden):
                for _try in range(8):
                    p = v_add(o, v_add(v_mul(e, rng.uniform(1.5, w - 1.5)), v_mul(n, rng.uniform(1.5, D - 1.5))))
                    if not self._in_lot_parts(lot, p, 2.0):
                        T.append((p[0], p[1], str(rng.choice(["kaki", "matsu", "momiji", "tsubaki", "ume", "kaki", "maki"])), 4 + 4 * rng.random()))
                        break
        # no trunk stands on a road or its edge (the stands, groves, offsets and windbreaks above
        # are placed without looking at the lanes that cross them): 0.8 m clear of the asphalt;
        # their crowns may still hang over it, as they do (crates/town/src/audit.rs)
        T[:] = [t for t in T if not self.near_road((t[0], t[1]), 0.8)]

    def boundaries(self):
        """What each plot is closed with, as segments in the plot's frame: the front (on the road) as
        the plan says (block wall, hedge, stone wall or open), with gaps at the gate and the
        driveway; sides and back with block walls (old farmhouses: stone). Gate posts either side of
        the gate gap, the mailbox on one of them."""
        for lot in self.plots:
            o, e, n, w, D = lot["frame"]
            P = lambda a, b: v_add(o, v_add(v_mul(e, a), v_mul(n, b)))
            front = lot["front"]
            segs, posts, mailbox = [], [], None
            if lot["type"] != "shophouse":
                inset = 0.15
                gate_a = dot(v_sub(lot["gate"], o), e)
                gaps = [(gate_a - 0.75, gate_a + 0.75)]
                if lot["parking"]:
                    pa = [dot(v_sub(q, o), e) for q in lot["parking"]]
                    pb = [dot(v_sub(q, o), n) for q in lot["parking"]]
                    if min(pb) < 1.0:  # the driveway opens onto the road
                        gaps.append((min(pa) - 0.2, max(pa) + 0.2))
                gaps.sort()
                kind = {"block": "block", "hedge": "hedge", "stone": "stone"}.get(front)
                if kind:
                    a = inset
                    for g0, g1 in gaps + [(w - inset, w + 1)]:
                        if g0 - a > 0.4:
                            segs.append({"kind": kind, "a": P(a, inset), "b": P(min(g0, w - inset), inset)})
                        a = max(a, g1)
                    if kind != "hedge":
                        for x in (gate_a - 0.75, gate_a + 0.75):
                            if inset < x < w - inset:
                                posts.append(P(x, inset))
                side = "stone" if front == "stone" else "block"
                segs.append({"kind": side, "a": P(w - inset, inset), "b": P(w - inset, D - inset)})
                segs.append({"kind": side, "a": P(w - inset, D - inset), "b": P(inset, D - inset)})
                segs.append({"kind": side, "a": P(inset, D - inset), "b": P(inset, inset)})
                # the mailbox at the gate, on the road side (on a post, or a stand of its own)
                mb_a = gate_a + 0.5 if gate_a + 0.5 < w - 0.3 else gate_a - 0.5
                mailbox = {"at": P(mb_a, inset + 0.1), "yaw": math.atan2(-n[0], -n[1])}
            lot["boundary"] = {"segments": segs, "posts": posts, "mailbox": mailbox}

    def _in_lot_parts(self, lot, p, margin):
        for k in ("house", "parking", "shed"):
            q = lot.get(k)
            if q and (point_in_convex(p, q) or min(seg_point_dist(q[i - 1], q[i], p) for i in range(4)) < margin):
                return True
        return False

    # --------------------------------------------------------------- build
    def build(self):
        self.network()
        self.civic()
        self.centre_streets()
        self.special_plots()
        self.hamlet_lanes()
        self.plots_all()
        self.rasterize()
        self.kitchen_gardens()
        self.terraces()
        self.fields()
        self.utility_poles()
        self.boundaries()
        self.vegetation()
        for i, lot in enumerate(self.plots):
            lot["id"] = i
        return self


def _road_quad(a, b, hw):
    t = v_norm(v_sub(b, a))
    n = perp(t)
    return [v_add(a, v_mul(n, hw)), v_add(b, v_mul(n, hw)), v_sub(b, v_mul(n, hw)), v_sub(a, v_mul(n, hw))]


# ============================================================================= walk + review


def path_route(plan):
    """The walk to school: out of Tsukumodō onto the station street, south past the station, over
    the level crossing and the prefectural road, across the paddies and the river to the gate."""
    door = plan.home["gate"]
    _, s = plan.school.nearest(door)
    pts = [door]
    L = plan.school.length()
    while s < L:
        pts.append(plan.school.at(s)[0])
        s += 5.0
    return pts + [plan.school.pts[-1], plan.school_gate]


def review(plan):
    problems, notes = [], []
    P = plan.plots
    households = sum(l["households"] for l in P)
    notes.append(f"{len(P)} houses, {households} households, {len(plan.paddies)} paddies, {len(plan.hatake)} vegetable plots, {len(plan.trees)} trees, {len(plan.poles)} poles, {len(plan.roads)} roads")
    if not (480 <= households <= 680):
        problems.append(f"{households} households: a town of ~1500 people needs about 500-600")
    for l in P:
        pid = l["id"]
        if not all(point_in_convex(q, shrink(l["poly"], -0.01)) for q in l["house"]):
            problems.append(f"plot {pid}: house leaves its plot")
        for k in ("parking", "shed"):
            if l.get(k) and convex_overlap(l[k], l["house"]):
                problems.append(f"plot {pid}: {k} overlaps the house")
        dmin = min(v_len(v_sub(l["entrance"], p)) for p in plan.poles)
        if dmin > 40:
            problems.append(f"plot {pid}: nearest pole {dmin:.0f} m (service wire > 40 m)")
        if l["road"].nearest(l["gate"])[0] > l["road"].width / 2 + 1.5:
            problems.append(f"plot {pid}: gate is not on its road")
        if l["frame"][3] < 2.0 or l["road"].width < 4.0:
            problems.append(f"plot {pid}: needs 2 m frontage on a road of 4 m or more")
        hs = [H(q) for q in l["poly"]]
        if max(hs) - min(hs) > 6.5:
            problems.append(f"plot {pid}: platform would need a {max(hs) - min(hs):.1f} m retaining wall")
    grid = Grid()
    for l in P:
        for o in grid.query(bbox(l["poly"])):
            if convex_overlap(l["poly"], o["poly"]):
                problems.append(f"plots {l['id']} and {o['id']} overlap")
        grid.add(bbox(l["poly"]), l)
        for s in plan.sites:
            if convex_overlap(l["poly"], s["poly"]):
                problems.append(f"plot {l['id']} overlaps {s['name']}")
    # lanes must start on another road (the network is connected)
    for r in plan.roads:
        joined = lambda p: any(o is not r and o.kind != "farm" and o.nearest(p)[0] < o.width / 2 + 4.5 for o in plan.roads)
        if r.kind in ("lane", "street") and not (joined(r.pts[0]) or joined(r.pts[-1])):
            problems.append(f"{r.name} starts nowhere")
    route = path_route(plan)
    L = sum(v_len(v_sub(b, a)) for a, b in zip(route[:-1], route[1:]))
    notes.append(f"walk from Tsukumodō to the school gate: {L:.0f} m (~{L / 75:.0f} min)")
    if not (350 <= L <= 1500):
        problems.append(f"walk to school {L:.0f} m is not a believable daily walk")
    by = {}
    for l in P:
        by[l["district"]] = by.get(l["district"], 0) + l["households"]
    notes.append("households: " + ", ".join(f"{k} {v}" for k, v in sorted(by.items(), key=lambda kv: -kv[1])))
    for h in HAMLETS:
        if by.get(h[0], 0) < 15:
            problems.append(f"hamlet {h[0]} has only {by.get(h[0], 0)} households")
    kinds = {}
    for l in P:
        kinds[l["type"]] = kinds.get(l["type"], 0) + 1
    notes.append("house types: " + ", ".join(f"{k} {v}" for k, v in sorted(kinds.items(), key=lambda kv: -kv[1])))
    kinds = {}
    for t in plan.trees:
        kinds[t[2]] = kinds.get(t[2], 0) + 1
    notes.append("trees: " + ", ".join(f"{k} {v}" for k, v in sorted(kinds.items(), key=lambda kv: -kv[1])))
    return problems, notes, route


# ============================================================================= output


def to_json(plan, route):
    R = lambda p: [round(p[0], 2), round(p[1], 2)]
    PL = lambda poly: [R(q) for q in poly] if poly else None
    houses = [{
        "id": l["id"], "district": l["district"], "kind": l["kind"], "type": l["type"], "name": l.get("name"),
        "plot": PL(l["poly"]), "house": PL(l["house"]), "parking": PL(l["parking"]), "shed": PL(l["shed"]),
        "entrance": R(l["entrance"]), "gate": R(l["gate"]), "yaw": round(l["yaw"], 4), "garden": l["garden_side"],
        "floors": l["floors"], "roof": l["roof"], "front": l["front"], "households": l["households"], "road": l["road"].name,
        "ground": l.get("ground", round(sum(H(q) for q in l["poly"]) / 4, 2)),
        "retaining": l.get("retaining", 0.0),
        "boundary": {
            "segments": [{"kind": sg["kind"], "a": R(sg["a"]), "b": R(sg["b"])} for sg in l["boundary"]["segments"]],
            "posts": [R(q) for q in l["boundary"]["posts"]],
            "mailbox": None if l["boundary"]["mailbox"] is None else {"at": R(l["boundary"]["mailbox"]["at"]), "yaw": round(l["boundary"]["mailbox"]["yaw"], 4)},
        },
    } for l in plan.plots]
    return {
        "name": "Kazehana-chō",
        "seed": SEED,
        "extent": [X0, Y0, X1, Y1],
        "terrain": plan.terrain,
        "roads": [{"name": r.name, "kind": r.kind, "width": r.width, "pts": [R(p) for p in r.pts], "h": getattr(r, "heights", None)} for r in plan.roads],
        "rail": [R(p) for p in plan.rail.pts],
        "rail_h": getattr(plan, "rail_heights", []),
        "river": {"pts": [R(p) for p in plan.river], "water": [round(float(floor_h(p[0])) - plan.terrain["water_drop"], 3) for p in plan.river], "water_half": RIVER_WATER, "levee_half": RIVER_LEVEE},
        "sites": [{"name": s["name"], "kind": s["kind"], "poly": PL(s["poly"]), "ground": s.get("ground"), "buildings": [{"name": b["name"], "poly": PL(b["poly"]), "floors": b["floors"]} for b in s["buildings"]]} for s in plan.sites],
        "hamlets": [{"name": h["name"], "centre": R(h["centre"]), "radius": h["radius"], "style": h["style"]} for h in plan.hamlets],
        "houses": houses,
        "home": plan.home["id"],
        "school_gate": R(plan.school_gate),
        "paddies": [PL(q) for q in plan.paddies],
        "paddy_levels": plan.paddy_levels,
        "hatake": [PL(q) for q in plan.hatake],
        "poles": [R(p) for p in plan.poles],
        "trees": [[round(t[0], 1), round(t[1], 1), t[2], round(t[3], 1)] for t in plan.trees],
        "route_to_school": [R(p) for p in route],
    }


TREE_COL = {
    "sugi": (34, 58, 38), "hinoki": (42, 68, 42), "konara": (96, 132, 66), "kunugi": (84, 116, 58), "shii": (50, 82, 46),
    "momiji": (156, 112, 60), "yamazakura": (242, 228, 232), "someiyoshino": (250, 200, 218), "bamboo": (128, 158, 72),
    "kusu": (58, 98, 50), "keyaki": (80, 120, 60), "kashi": (46, 78, 44), "kaki": (92, 132, 60), "matsu": (38, 78, 50),
    "tsubaki": (38, 78, 40), "ume": (232, 188, 200), "maki": (40, 74, 46), "kuri": (100, 128, 62), "hedge": (52, 90, 44),
}


def draw(plan, route, path, view, mpp, labels=True):
    from PIL import Image, ImageDraw
    x0, y0, x1, y1 = view
    W, Hh = int((x1 - x0) / mpp), int((y1 - y0) / mpp)
    gx, gy = np.meshgrid(np.linspace(x0, x1, W), np.linspace(y1, y0, Hh))
    hh = height(gx, gy)
    dzdx = np.gradient(hh, axis=1) / mpp
    dzdy = -np.gradient(hh, axis=0) / mpp
    shade = np.clip(0.8 + (-dzdx + dzdy) * 0.9, 0.35, 1.25)
    rel = hh - floor_h(gx)
    base = np.where(rel[..., None] > 2.0, np.array([104, 126, 80]), np.array([150, 160, 104]))
    im = Image.fromarray(np.clip(base * shade[..., None], 0, 255).astype(np.uint8), "RGB")
    d = ImageDraw.Draw(im)
    T = lambda p: ((p[0] - x0) / mpp, (y1 - p[1]) / mpp)

    def poly(pts, fill=None, outline=None):
        d.polygon([T(p) for p in pts], fill=fill, outline=outline)

    def line(pts, col, width_m, minpx=1):
        d.line([T(p) for p in pts], fill=col, width=max(minpx, int(round(width_m / mpp))), joint="curve")

    for q in plan.paddies:
        poly(q, (146, 112, 82), (190, 172, 124) if mpp < 1 else None)
    for q in plan.hatake:
        poly(q, (110, 92, 62), (96, 140, 70) if mpp < 1 else None)
    line(plan.river, (150, 160, 118), RIVER_LEVEE * 2)
    line(plan.river, (64, 104, 150), RIVER_WATER * 2)
    for s in plan.sites:
        col = {"school": (212, 202, 172), "station": (150, 150, 158), "plaza": (196, 196, 200), "shrine": (70, 96, 60), "temple": (150, 140, 120), "inari": (70, 96, 60), "konbini": (200, 200, 205), "business": (190, 185, 180), "civic": (190, 180, 170)}[s["kind"]]
        poly(s["poly"], col, (60, 60, 60))
        for b in s["buildings"]:
            poly(b["poly"], (176, 150, 116) if b["floors"] == 0 else (112, 102, 102))
    for r in plan.roads:
        col = {"pref": (70, 70, 75), "old": (96, 96, 99), "school": (86, 86, 90), "street": (100, 100, 104), "lane": (118, 114, 108), "farm": (168, 152, 120), "levee": (178, 166, 134)}[r.kind]
        line(r.pts, col, r.width)
    line(plan.rail.pts, (60, 50, 45), 3.0, 2)
    for lot in plan.plots:
        poly(lot["poly"], (184, 194, 152), (92, 92, 82))
        if lot["parking"]:
            poly(lot["parking"], (166, 166, 170))
        if lot["shed"]:
            poly(lot["shed"], (122, 96, 76))
        roof = {"shophouse": (92, 82, 92), "farmhouse": (70, 76, 88), "apartment": (152, 152, 156), "modern_hip": (82, 70, 64), "modern_gable": (98, 86, 80), "small_house": (112, 96, 86)}[lot["type"]]
        poly(lot["house"], roof, (30, 30, 30))
    r_big, r_small = max(1.0, 2.6 / mpp), max(0.8, 1.6 / mpp)
    for t in plan.trees:
        x, y = T(t)
        if -5 < x < W + 5 and -5 < y < Hh + 5:
            rr = r_small if t[2] in ("sugi", "hinoki", "bamboo") else (r_big if t[3] > 6 else r_small)
            d.ellipse([x - rr, y - rr, x + rr, y + rr], fill=TREE_COL[t[2]])
    if mpp < 1:
        for p in plan.poles:
            x, y = T(p)
            d.ellipse([x - 2, y - 2, x + 2, y + 2], fill=(240, 140, 30))
    line(route, (230, 40, 40), 1.2 if mpp < 1 else 3.0, 2)
    # where the students live (from the sim: cargo run -p sim --example households)
    hp = os.path.join(OUT, "households.json")
    if os.path.exists(hp):
        with open(hp, encoding="utf8") as f:
            homes = {}
            for st in json.load(f)["students"]:
                homes[st["house"]] = homes.get(st["house"], 0) + 1
        by_id = {l["id"]: l for l in plan.plots}
        for hid, k in homes.items():
            if hid in by_id:
                x, y = T(poly_center(by_id[hid]["house"]))
                r = 3.5 + 1.5 * k
                d.ellipse([x - r, y - r, x + r, y + r], outline=(30, 120, 255), width=2)
    hx, hy = T(poly_center(plan.home["poly"]))
    d.ellipse([hx - 7, hy - 7, hx + 7, hy + 7], outline=(255, 230, 0), width=3)
    if labels:
        d.text((hx + 9, hy - 6), "Tsukumodo (home)", fill=(255, 240, 0))
        for s in plan.sites:
            if s["kind"] != "plaza":
                d.text(T(poly_center(s["poly"])), s["name"].replace("ō", "o"), fill=(0, 0, 0))
        for h in plan.hamlets:
            d.text(T(v_add(h["centre"], (0, 40 * (1 if h["centre"][1] > VC(h["centre"][0]) else -1)))), h["name"], fill=(255, 255, 255))
    im.save(path)


def main():
    plan = Plan().build()
    problems, notes, route = review(plan)
    os.makedirs(OUT, exist_ok=True)
    import town_terrain
    town_terrain.export_terrain(plan)
    with open(os.path.join(OUT, "town.json"), "w", encoding="utf8") as f:
        json.dump(to_json(plan, route), f, separators=(",", ":"))
    draw(plan, route, os.path.join(OUT, "map.png"), (X0, Y0, X1, Y1), 1.5)
    cy = VC(SX)
    draw(plan, route, os.path.join(OUT, "map_centre.png"), (-340.0, cy - 470, 420.0, cy + 380), 0.7)
    draw(plan, route, os.path.join(OUT, "map_valley.png"), (-1500.0, cy - 560, 1500.0, cy + 760), 1.25)
    for n in notes:
        print(n)
    if problems:
        print(f"TOWN REVIEW: {len(problems)} problem(s)")
        for p in problems[:60]:
            print("  -", p)
    else:
        print("TOWN REVIEW: OK")
    return problems


if __name__ == "__main__":
    raise SystemExit(1 if main() else 0)
