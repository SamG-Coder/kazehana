"""The town's people (crates/census: assets/town/census.json) as models, in a simple anime style:
built from shapes in code, skinned to a small skeleton, with an idle and a walk, exported as glTF
for the game (town::people).

    blender --background --factory-startup --python tools/blender/make_people.py -- <what>
      <what>: census ids (12 300 ...), "gallery" (one of each kind of look), or "all" (everyone
      who lives in town)
-> assets/cache/people/p<id>/p<id>.gltf (+ p<id>.json: height, walk speed)

Each person's look comes from the census: height and build for their age, skin, hair (fifteen
cuts), eyes, glasses, what they wear to school or work (the gakuran and the sailor uniform,
suits, farm wear, shop aprons, coveralls, the konbini shirt, a teacher's shirt and tie, the PE
teacher's tracksuit, the priest's samue...) and their accessories (randoseru, the first-graders'
yellow hat, caps, straw hats, sun visors, headbands, aprons).

The face is a painted texture (big eyes with a coloured iris and highlights, a thick upper lash
line, a small nose and mouth) on a head with a small, pointed chin. Proportions follow age:
about six heads tall for grown-ups, nearer four for small children. The body is lofted rings
(torso, arms, legs), each ring weighted to its joint, blended at the elbows, knees, hips and
shoulders.

The character faces -Y in Blender (+Z in glTF), feet on the ground at the origin.
"""
import json
import math
import os
import sys

import bpy
import numpy as np
from mathutils import Euler, Matrix, Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
CENSUS = os.path.join(ROOT, "assets", "town", "census.json")
# (KZ_PEOPLE_OUT: build somewhere else, e.g. a review copy that leaves the town's models alone)
OUT = os.environ.get("KZ_PEOPLE_OUT") or os.path.join(ROOT, "assets", "cache", "people")
FPS = 30
FRONT = Vector((0, -1, 0))


def lin(c):
    """sRGB 0-255 -> linear 0-1 (material colours)."""
    def f(x):
        x = x / 255.0
        return x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4
    return (f(c[0]), f(c[1]), f(c[2]), 1.0)


# --- geometry, gathered into one mesh: vertices with joint weights and UVs, faces with materials


class Build:
    def __init__(self):
        self.v, self.w, self.uv, self.f, self.fm = [], [], [], [], []
        self.mats = []

    def mat(self, name):
        if name not in self.mats:
            self.mats.append(name)
        return self.mats.index(name)

    def vert(self, p, w, uv=(0.0, 0.0)):
        self.v.append(tuple(p))
        self.w.append(w)
        self.uv.append(uv)
        return len(self.v) - 1

    def face(self, idx, m):
        self.f.append(tuple(idx))
        self.fm.append(m)

    def ellipsoid(self, c, r, w, mat, rot=None, seg=20, rings=12, keep=None, deform=None, uvfn=None):
        """An ellipsoid at c with radii r (in its own frame `rot`). keep(local) drops faces whose
        corners it rejects; deform(local) -> local reshapes; uvfn(local) -> (u, v)."""
        rot = rot or Matrix.Identity(3)
        m = self.mat(mat)
        ids = {}

        def add(key, local):
            lp = Vector(local)
            uv = uvfn(lp) if uvfn else (0.0, 0.0)
            if deform:
                lp = deform(lp)
            p = Vector(c) + rot @ Vector((lp.x * r[0], lp.y * r[1], lp.z * r[2]))
            ids[key] = (self.vert(p, w(lp) if callable(w) else w, uv), Vector(local))

        add("top", (0, 0, 1))
        add("bot", (0, 0, -1))
        for i in range(1, rings):
            t = math.pi * i / rings
            for j in range(seg):
                ph = 2 * math.pi * j / seg
                add((i, j), (math.sin(t) * math.cos(ph), math.sin(t) * math.sin(ph), math.cos(t)))

        def put(keys):
            if keep and not all(keep(ids[k][1]) for k in keys):
                return
            self.face([ids[k][0] for k in keys], m)

        for j in range(seg):
            j2 = (j + 1) % seg
            put(["top", (1, j2), (1, j)])
            put(["bot", (rings - 1, j), (rings - 1, j2)])
            for i in range(1, rings - 1):
                put([(i, j), (i, j2), (i + 1, j2), (i + 1, j)])

    def loft(self, pts, radii, weights, mats, seg=18, cap_start=True, cap_end=True, front=FRONT):
        """Rings along a path: pts[i] with radii[i] = (side, front), weights[i] (joint weights, or a
        function of the vertex's position), and the material of the band from ring i to i+1.
        Ends are rounded off."""
        pts = [Vector(p) for p in pts]
        n = len(pts)
        rings = []
        for i in range(n):
            a = pts[max(i - 1, 0)]
            b = pts[min(i + 1, n - 1)]
            t = (b - a).normalized()
            s = t.cross(front)
            if s.length < 1e-4:
                s = t.cross(Vector((0, 0, 1)))
            s.normalize()
            f = s.cross(t).normalized()
            ring = []
            for j in range(seg):
                ph = 2 * math.pi * j / seg
                p = pts[i] + s * radii[i][0] * math.cos(ph) + f * radii[i][1] * math.sin(ph)
                ring.append(self.vert(p, weights[i](p) if callable(weights[i]) else weights[i]))
            rings.append((ring, t, s, f))
        for i in range(n - 1):
            m = self.mat(mats[min(i, len(mats) - 1)])
            r0, r1 = rings[i][0], rings[i + 1][0]
            for j in range(seg):
                j2 = (j + 1) % seg
                self.face([r0[j], r0[j2], r1[j2], r1[j]], m)

        def cap(i, sign, m):
            ring, t, s, f = rings[i]
            rr = radii[i]
            mid = []
            for j in range(seg):
                ph = 2 * math.pi * j / seg
                p = pts[i] + t * sign * min(rr) * 0.45 + s * rr[0] * 0.72 * math.cos(ph) + f * rr[1] * 0.72 * math.sin(ph)
                mid.append(self.vert(p, weights[i](p) if callable(weights[i]) else weights[i]))
            tp = pts[i] + t * sign * min(rr) * 0.62
            tip = self.vert(tp, weights[i](tp) if callable(weights[i]) else weights[i])
            for j in range(seg):
                j2 = (j + 1) % seg
                if sign > 0:
                    self.face([ring[j], ring[j2], mid[j2], mid[j]], m)
                    self.face([mid[j], mid[j2], tip], m)
                else:
                    self.face([ring[j2], ring[j], mid[j], mid[j2]], m)
                    self.face([mid[j2], mid[j], tip], m)

        if cap_start:
            cap(0, -1, self.mat(mats[0]))
        if cap_end:
            cap(n - 1, 1, self.mat(mats[-1]))

    def strand(self, pts, r0, w, mat):
        """A lock of hair: a tapering tube to a point."""
        n = len(pts)
        radii = [(r0 * (1 - i / (n - 1)) ** 0.8 + 0.0008, r0 * 0.55 * (1 - i / (n - 1)) ** 0.8 + 0.0006) for i in range(n)]
        self.loft(pts, radii, [w] * n, [mat], seg=8, cap_start=True, cap_end=False)

    def ring(self, c, axis_rot, R, r, w, mat, seg=28, tube=8):
        """A torus (glasses rims, a headband) in the plane of axis_rot's X and Y."""
        m = self.mat(mat)
        ids = []
        for i in range(seg):
            a = 2 * math.pi * i / seg
            row = []
            for j in range(tube):
                b = 2 * math.pi * j / tube
                local = Vector(((R + r * math.cos(b)) * math.cos(a), (R + r * math.cos(b)) * math.sin(a), r * math.sin(b)))
                row.append(self.vert(Vector(c) + axis_rot @ local, w))
            ids.append(row)
        for i in range(seg):
            i2 = (i + 1) % seg
            for j in range(tube):
                j2 = (j + 1) % tube
                self.face([ids[i][j], ids[i2][j], ids[i2][j2], ids[i][j2]], m)

    def slab(self, top, bottom, width, depth_at, w, mat, nx=8, nz=10, thick=0.006):
        """A cloth panel hanging on the front of the body (an apron): its front follows the body's
        depth at each height (depth_at(z) -> (half width, half depth))."""
        m = self.mat(mat)
        grid_f, grid_b = [], []
        for i in range(nz + 1):
            z = top + (bottom - top) * i / nz
            hw, hd = depth_at(z)
            rf, rb = [], []
            for j in range(nx + 1):
                x = (-1 + 2 * j / nx) * width / 2
                k = min(abs(x) / max(hw, 1e-3), 0.98)
                y = -(hd * math.sqrt(1 - k * k)) - 0.012
                rf.append(self.vert((x, y - thick, z), w(z)))
                rb.append(self.vert((x, y, z), w(z)))
            grid_f.append(rf)
            grid_b.append(rb)
        for i in range(nz):
            for j in range(nx):
                self.face([grid_f[i][j], grid_f[i + 1][j], grid_f[i + 1][j + 1], grid_f[i][j + 1]], m)
                self.face([grid_b[i][j], grid_b[i][j + 1], grid_b[i + 1][j + 1], grid_b[i + 1][j]], m)
        for i in range(nz):
            for j in (0, nx):
                a, b = (grid_f, grid_b) if j == 0 else (grid_b, grid_f)
                self.face([a[i][j], a[i + 1][j], b[i + 1][j], b[i][j]], m)
        for j in range(nx):
            self.face([grid_f[nz][j], grid_b[nz][j], grid_b[nz][j + 1], grid_f[nz][j + 1]], m)


def blend(a, b, t):
    """Weights between joint a and b, t of the way to b."""
    if t <= 0:
        return {a: 1.0}
    if t >= 1:
        return {b: 1.0}
    return {a: 1 - t, b: t}


# --- the face: painted on a square texture laid over the front of the head


def paint_face(spec, path, size=512):
    """The face, painted flat in the anime way (docs/anime_faces.md): eyes just below the middle of
    the head, an eye's width apart; tall irises under a heavy upper lash line, two highlights lit
    from the same side in both eyes; thin brows; a nose that is only a little shadow, halfway from
    the eyes to the chin; a small mouth just above halfway from the nose to the chin. Children:
    rounder, bigger eyes set lower, brows higher and thinner. The old: narrower eyes with bags,
    thinner mouth, a bigger nose, lines at the eyes, mouth and forehead."""
    H = size
    v, u = np.mgrid[0:H, 0:H]
    u = (u + 0.5) / H
    v = 1.0 - (v + 0.5) / H  # (v up, as the head's UVs: v = 0.5 + z / 2 on the unit head)
    px = 1.0 / H
    img = np.ones((H, H, 4), np.float32)
    skin = np.array(spec["skin"], np.float32) / 255
    img[..., :3] = skin
    old = spec.get("old", False)
    female = spec["build"] == "f"
    age = spec.get("age", 30)
    kid = min(max((14 - age) / 10, 0), 1)  # 1 for small children, 0 from 14 up
    elder = min(max((age - 55) / 25, 0), 1)

    def put(mask, color, alpha=1.0):
        a = np.clip(mask.astype(np.float32) * alpha, 0, 1)
        img[..., :3] = img[..., :3] * (1 - a[..., None]) + np.array(color, np.float32)[None, None, :] * a[..., None]

    def ell(cu, cv, ru, rv):
        return ((u - cu) / ru) ** 2 + ((v - cv) / rv) ** 2

    def soft(e, r, feather=1.2):
        """Coverage (0..1, antialiased) inside an ell() of smaller radius r."""
        return np.clip((1 - np.sqrt(e)) * r / (feather * px) + 0.5, 0, 1)

    def stroke(d, half):
        """Coverage of a line: d is the distance to its middle, half its half width."""
        return np.clip((half - np.abs(d)) / (1.2 * px) + 0.5, 0, 1)

    dark = np.array([0.09, 0.06, 0.07])
    hair = np.array(spec["hair"]["color"], np.float32) / 255
    iris = np.array(spec["eyes"], np.float32) / 255
    line = dark * 0.6 + hair * 0.4 * 0.5  # (lashes: near black, a touch of the hair's colour)

    # where things are (v): eyes just below the middle of the head, lower on children
    ey = 0.405 - 0.035 * kid
    nose_v = 0.245 + 0.01 * kid
    mouth_v = 0.162 + 0.022 * kid
    # the eye opening (half sizes in u, v): wide and tall for children and girls, narrower for men,
    # narrow and heavy-lidded for the old
    rx = (0.095 if female else 0.09) * (1 + 0.14 * kid) * (1 - 0.1 * elder)
    grown = min(max((age - 18) / 22, 0), 1)  # (grown-ups' eyes a little narrower, more so men's)
    ry = (0.074 if female else 0.06) * (1 + 0.4 * kid) * (1 - 0.35 * elder) * (1 - (0.06 if female else 0.12) * grown)
    gap = 0.17 + 0.03 * kid  # (eye centres from the middle: about an eye's width between them)

    if spec.get("stubble"):
        jaw = ell(0.5, 0.1, 0.30, 0.16)
        put(np.clip((1 - jaw) * 3, 0, 1) * np.clip((0.2 - v) / 0.05, 0, 1), skin * 0.72 + np.array([0.05, 0.06, 0.08]), 0.22)
    if spec.get("blush"):
        for s in (-1, 1):
            e = ell(0.5 + s * (gap + 0.02), ey - ry - 0.045, 0.065, 0.028)
            put(np.exp(-e * 1.6), (0.98, 0.55, 0.55), 0.38)

    for s in (-1, 1):
        cu = 0.5 + s * gap
        p = s * (u - cu) / rx  # (across the eye: -1 inner corner, +1 outer)
        q = (v - ey) / ry - 0.10 * p  # (up the eye, the outer corner tilted a little up)
        opening = np.abs(p) ** 2.6 + np.abs(q) ** 2  # (< 1 inside: a soft rounded rectangle)
        o_in = soft(opening, min(rx, ry) * 0.5)
        put(o_in, (0.97, 0.97, 0.99))
        # the upper lid's shadow on the white
        put(o_in * np.clip((q - 0.25) / 0.6, 0, 1), (0.72, 0.72, 0.80), 0.6)
        # the iris: tall, cut by both lids; dark at the top, bright toward the bottom
        icv = ey - 0.06 * ry
        irx, iry = rx * (0.6 + 0.04 * kid), ry * (1.08 if not old else 1.2)
        ie = ell(cu, icv, irx, iry)
        grad = np.clip((v - (icv - iry)) / (2 * iry), 0, 1)[..., None]
        ic = np.clip(iris[None, None, :] * (1.35 - 0.95 * grad) + 0.06 * (1 - grad), 0, 1)
        m = (soft(ie, irx) * o_in)[..., None]
        img[..., :3] = img[..., :3] * (1 - m) + ic * m
        put(o_in * stroke(np.sqrt(ie) - 1, 0.09) * (np.sqrt(ie) < 1.05), iris * 0.3)  # (its rim)
        put(o_in * soft(ell(cu, icv + 0.08 * iry, irx * 0.42, iry * 0.48), irx * 0.42), iris * 0.18)  # (the pupil)
        put(o_in * (soft(ie, irx) * np.clip((q - 0.35) / 0.4, 0, 1)), iris * 0.2, 0.55)  # (the lid's shadow on it)
        # highlights: a big one up on the light's side, a small one down on the other (same in both eyes)
        put(o_in * soft(ell(cu - 0.42 * irx, icv + 0.38 * iry, 0.27 * irx, 0.24 * iry), 0.24 * iry), (1, 1, 1))
        put(o_in * soft(ell(cu + 0.4 * irx, icv - 0.48 * iry, 0.12 * irx, 0.1 * iry), 0.1 * iry), (1, 1, 1), 0.9)
        # the upper lash line: heavy, thicker toward the outer corner, out past it in a little wing
        th = (0.32 + 0.16 * np.clip(p, 0, 1)) * (1.15 if female else 1.0) * (0.8 if old else 1.0)
        lash = np.abs(p / 1.06) ** 2.6 + np.abs(q / (1 + th)) ** 2
        put(soft(lash, ry * 0.4) * (1 - o_in) * (q > -0.05), line)
        wing = ((p - 0.98) / 0.26) ** 2 + ((q - 0.26 + 0.55 * (p - 0.98)) / (0.13 * np.clip(1.25 - p, 0.2, 1))) ** 2
        put(soft(wing, ry * 0.1), line)
        if female:
            for k, (pp, qq) in enumerate(((0.95, 0.55), (0.72, 0.85))):
                spike = ((p - pp - 0.1) / 0.16) ** 2 + ((q - qq - 0.12 - 0.6 * (p - pp - 0.1)) / 0.07) ** 2
                put(soft(spike, ry * 0.07), line)
        # the lower lid: a short thin line at the outer side
        lower = np.abs(p / 1.0) ** 2.6 + np.abs(q / 1.08) ** 2
        put(stroke(np.sqrt(lower) - 1.0, 0.07) * (q < -0.25) * (p > -0.15) * np.clip((p + 0.15) / 0.4, 0, 1), line, 0.75)
        # the lid crease above (grown-ups and teens; faint on children)
        crease = np.abs(p / 1.0) ** 2.4 + np.abs((q - 0.1) / 1.55) ** 2
        put(stroke(np.sqrt(crease) - 1.0, 0.035) * (q > 0.5) * (p > -0.5) * (p < 0.95), skin * 0.6, 0.65 - 0.4 * kid)
        # the brow: thin and arched, higher (and thinner) on children, lower and straighter on men
        bu = s * (u - cu) / (rx * 1.05)
        bv0 = ey + ry * 1.25 + (0.05 if female else 0.04) + 0.02 * kid - 0.01 * elder
        bv = bv0 + (0.014 if female else 0.008) * (1 - bu ** 2) - 0.006 * bu
        bw = (0.0075 if female else 0.0105) * (1 - 0.25 * kid) * np.clip(1.15 - 0.45 * (bu + 0.8) / 1.9, 0.4, 1)
        put(stroke(v - bv, bw) * (bu > -0.85) * (bu < 1.05), hair * 0.75 + dark * 0.25 if not old else hair * 0.8)
        if old:
            # bags, and crow's feet at the outer corner
            bag = np.abs(p / 0.9) ** 2.2 + np.abs((q + 0.55) / 1.0) ** 2
            put(stroke(np.sqrt(bag) - 1.0, 0.05) * (q < -0.9) * (np.abs(p) < 0.75), skin * 0.62, 0.8)
            for k in range(3):
                d = (v - (ey + 0.012 - k * 0.016)) - 0.35 * s * (u - cu - s * rx * 1.15)
                put(stroke(d, 0.0016) * (s * (u - cu) > rx * 1.12) * (s * (u - cu) < rx * 1.5), skin * 0.66, 0.8)

    # the nose: a small shadow on the side away from the light, and the tip
    nl = 1 + 0.35 * elder - 0.3 * kid
    put(soft(ell(0.506, nose_v + 0.022 * nl, 0.006 * nl, 0.026 * nl), 0.006 * nl, 2.0), skin * 0.78, 0.55)
    put(stroke(v - nose_v - 0.25 * np.abs(u - 0.5), 0.0028) * (np.abs(u - 0.5 - 0.003) < 0.009 * nl), skin * 0.62)
    if old:
        for s in (-1, 1):
            put(stroke(np.sqrt(ell(0.5 + s * 0.022, nose_v + 0.006, 0.012, 0.01)) - 1, 0.12) * (v < nose_v + 0.008), skin * 0.66, 0.7)

    # the mouth: a small curve, corners up a touch; a hint of the lower lip under it
    mw = (0.034 if female else 0.04) * (1 - 0.15 * kid)
    x = (u - 0.5) / mw
    mv = mouth_v + 0.008 * x ** 2 - (0.004 * elder) * np.abs(x) ** 3
    mthick = 0.0044 * (1 - 0.45 * x ** 2) * (0.8 if old else 1.0)
    put(stroke(v - mv, np.maximum(mthick, 0.0016)) * (np.abs(x) < 1), (0.50, 0.24, 0.24) if female else (0.42, 0.24, 0.22))
    put(soft(ell(0.5, mouth_v - 0.026, 0.014, 0.004), 0.004, 2.0), skin * 0.82, 0.6 - 0.3 * kid)
    if spec.get("mustache"):
        mc = np.array(spec["mustache"], np.float32) / 255
        for s in (-1, 1):
            put(soft(ell(0.5 + s * 0.022, mouth_v + 0.024 - 0.12 * np.abs(u - 0.5 - s * 0.022), 0.026, 0.011), 0.011), mc)
    if old:
        # lines from beside the nose down past the mouth, and across the forehead
        for s in (-1, 1):
            t = (nose_v - v) / (nose_v - mouth_v + 0.02)
            put(stroke(u - 0.5 - s * (0.045 + 0.02 * t + 0.012 * t * t), 0.0017) * (t > 0.05) * (t < 1.0), skin * 0.66, 0.85 * (1 - 0.6 * t))
        for k in range(2):
            put(stroke(v - (0.63 + 0.03 * k) - 0.008 * np.cos((u - 0.5) * 9), 0.0016) * (np.abs(u - 0.5) < 0.13 - 0.03 * k), skin * 0.72)
    im = bpy.data.images.new(os.path.basename(path), H, H, alpha=True)
    im.pixels.foreach_set(np.flipud(img).ravel())
    im.filepath_raw = path
    im.file_format = "PNG"
    im.save()
    return im


# --- from the census: a person's look as the builder's spec


def interp(x, pts):
    if x <= pts[0][0]:
        return pts[0][1]
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        if x <= x1:
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0)
    return pts[-1][1]


def spec_from_census(c, pid, casual=False):
    p = c["people"][pid]
    L = p["look"]
    age, female = p["age"], p["sex"] == "F"
    cl = L["casual"] if casual else L["day"]
    outfit = cl["outfit"]
    top, bottom, shoes, extra = [tuple(x) for x in cl["colors"]]
    h = (pid * 2654435761) % 1000 / 1000.0  # (a steady per-person coin for small choices)
    s = {
        "id": pid, "age": age, "H": L["height_cm"] / 100.0, "build": "f" if female else "m", "physique": L["build"],
        "old": age >= 60, "stoop": L["stoop"], "skin": L["skin"], "eyes": L["eyes"],
        "hair": {"style": L["hair"], "color": L["hair_color"]}, "glasses": L["glasses"],
        "stubble": L["stubble"], "mustache": L["hair_color"] if L["mustache"] else None,
        "blush": (female and age < 35) or age < 10,
        "shirt": top, "pants": bottom, "shoes": shoes, "sleeve": 1.0, "pants_cover": 1.0,
        "skirt": None, "socks": None, "boots": False, "armcovers": None, "extras": [], "accessories": list(L["accessories"]),
    }

    def skirt(to, flare=1.5, pleats=0):
        return {"color": bottom, "to": to, "flare": flare, "pleats": pleats}

    if outfit == "Gakuran":
        s["extras"] += [("collar_stand", top), ("buttons", (210, 175, 70))]
    elif outfit == "Sailor":
        s["skirt"] = skirt(0.32, 1.45, 24)
        s["pants_cover"] = 0.0
        s["socks"] = ((236, 236, 236) if h < 0.6 else (40, 44, 72), 0.38)
        s["extras"] += [("sailor_collar", bottom), ("scarf", extra)]
    elif outfit in ("KidsCasual", "BabyWear"):
        s["sleeve"] = 0.4
        if female and h < 0.55:
            s["skirt"] = skirt(0.34, 1.6, 16)
            s["pants_cover"] = 0.0
        else:
            s["pants_cover"] = 0.45
        s["socks"] = ((236, 236, 236), 0.2)
        if outfit == "BabyWear":
            s["pants"] = top
            s["pants_cover"] = 1.0
            s["skirt"] = None
    elif outfit == "Casual":
        s["sleeve"] = 0.45 if h < 0.5 else 1.0
        if age < 40 and 0.3 < h < 0.55:
            s["extras"] += [("hood", top)]
        if not female or s.get("skirt") is None:
            s["extras"] += [("belt", (50, 40, 34))]
        if female and h > 0.6:
            s["skirt"] = skirt(0.30 if age < 40 else 0.22, 1.45)
            s["pants_cover"] = 0.0
    elif outfit == "Suit":
        s["extras"] += [("shirt_v", (240, 240, 236)), ("tie", extra)]
    elif outfit == "Office":
        s["skirt"] = skirt(0.30, 1.25)
        s["pants_cover"] = 0.0
        s["socks"] = ((70, 64, 64), 1.0)  # (tights)
    elif outfit == "Coveralls":
        s["pants"] = top
        s["extras"] += [("zip", tuple(int(c * 0.6) for c in top)), ("belt", tuple(int(c * 0.55) for c in top)), ("pocket", tuple(int(c * 0.8) for c in top)), ("collar_flat", tuple(int(c * 0.8) for c in top))]
    elif outfit == "FarmWear":
        s["boots"] = True
        s["armcovers"] = extra
        s["extras"] += [("collar_flat", top), ("belt", (60, 50, 40))]
    elif outfit == "ShopApron":
        s["sleeve"] = 0.5 if h < 0.5 else 1.0
        s["extras"] += [("apron", extra)]
    elif outfit == "Konbini":
        s["sleeve"] = 0.45
        s["extras"] += [("vest", extra), ("belt", (40, 40, 44)), ("collar_flat", top)]
    elif outfit == "Tracksuit":
        s["extras"] += [("stripes", extra)]
    elif outfit == "Teacher":
        if not female:
            s["extras"] += [("tie", extra), ("belt", (40, 34, 30)), ("collar_flat", top)]
        else:
            s["skirt"] = skirt(0.28, 1.25)
            s["pants_cover"] = 0.0
            s["socks"] = ((70, 64, 64), 1.0)
    elif outfit == "Cardigan":
        s["extras"] += [("shirt_v", (236, 232, 220)), ("buttons", tuple(int(c * 0.6) for c in top))]
    elif outfit == "Grandma":
        if h < 0.6:
            s["skirt"] = skirt(0.14, 1.3)
            s["pants_cover"] = 0.0
    elif outfit == "Samue":
        s["pants"] = top
        s["extras"] += [("wrap", (40, 44, 60))]
    if "Apron" in s["accessories"] and not any(e[0] == "apron" for e in s["extras"]):
        s["extras"] += [("apron", (232, 226, 210) if h < 0.5 else (150, 110, 120))]
    return s


# --- one person


def person(name, spec, out_dir):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    os.makedirs(out_dir, exist_ok=True)
    Hreal = spec["H"]
    age = spec["age"]
    female = spec["build"] == "f"
    old = spec.get("old", False)
    phys = spec.get("physique", "Average")
    width = {"Slim": 0.9, "Average": 1.0, "Sturdy": 1.07, "Heavy": 1.1}[phys]
    g = {"Slim": 0.95, "Average": 1.0, "Sturdy": 1.08, "Heavy": 1.32}[phys] * (1.04 if old else 1.0)
    # anime proportions: heads tall by age (grown-ups about six; small children nearer four)
    heads = interp(age, [(1, 3.2), (3, 3.8), (6, 4.5), (9, 5.1), (12, 5.5), (15, 5.9), (18, 6.1)])
    if age < 5:
        width *= 1.18
        g *= 1.1
    head_r = Hreal / (heads * 2.2)
    head_c = Vector((0, 0, Hreal - 1.1 * head_r))
    H = (Hreal - 2.2 * head_r) / 0.837  # (the body is built as if for this height, under the head)
    HH = head_r / 0.074  # (sizes on the head: as for a grown-up of this height)
    B = Build()

    # joints (metres)
    sh_x = (0.092 if female else 0.105) * H * width * (0.92 if age < 12 else 1.0)
    J = {
        "root": (Vector((0, 0, 0)), Vector((0, 0, 0.1 * H))),
        "pelvis": (Vector((0, 0, 0.49 * H)), Vector((0, 0, 0.56 * H))),
        "spine": (Vector((0, 0, 0.56 * H)), Vector((0, 0, 0.66 * H))),
        "chest": (Vector((0, 0, 0.66 * H)), Vector((0, 0, 0.80 * H))),
        "neck": (Vector((0, 0, 0.80 * H)), Vector((0, 0, 0.865 * H))),
        "head": (Vector((0, 0, head_c.z - 0.9 * head_r)), Vector((0, 0, Hreal))),
    }
    for side, sx in (("l", 1), ("r", -1)):
        sh = Vector((sx * sh_x, 0.0, 0.785 * H))
        # (anime: the elbow at the waist, the wrist at the crotch, upper arm ~ forearm + palm;
        # the arms hang a little out from the body, further round a big belly)
        el = Vector((sx * (sh_x + 0.016 * H + 0.06 * H * (g - 1)), 0.006 * H, 0.60 * H))
        wr = Vector((sx * (sh_x + 0.026 * H + 0.05 * H * (g - 1)), -0.006 * H, 0.445 * H))
        hip = Vector((sx * 0.05 * H * width, 0, 0.49 * H))
        kn = Vector((sx * 0.052 * H * width, -0.004 * H, 0.27 * H))
        an = Vector((sx * 0.054 * H * width, 0.004 * H, 0.05 * H))
        J[f"upperarm_{side}"] = (sh, el)
        J[f"lowerarm_{side}"] = (el, wr)
        J[f"hand_{side}"] = (wr, wr + Vector((0, 0, -0.07 * H)))
        J[f"thigh_{side}"] = (hip, kn)
        J[f"calf_{side}"] = (kn, an)
        J[f"foot_{side}"] = (an, Vector((an.x, -0.085 * H, 0.02 * H)))
    parents = {"pelvis": "root", "spine": "pelvis", "chest": "spine", "neck": "chest", "head": "neck"}
    for s in ("l", "r"):
        parents.update({f"upperarm_{s}": "chest", f"lowerarm_{s}": f"upperarm_{s}", f"hand_{s}": f"lowerarm_{s}",
                        f"thigh_{s}": "pelvis", f"calf_{s}": f"thigh_{s}", f"foot_{s}": f"calf_{s}"})

    skirt = spec.get("skirt")
    pants_mat = "skirt" if skirt else "pants"

    # torso: rings up from the crotch, weighted pelvis -> spine -> chest: (height, half width, half
    # depth, where its middle sits front to back: - is forward). Anime bodies (after drawing guides): the ribcage
    # sits inside the shoulders so the arms hang clear of a narrow waist; men taper from square
    # shoulders to hips about as wide as the chest; women narrower in the chest with a small waist
    # and hips as wide as the shoulders; small children a soft pear with a round belly; the crotch
    # flat and shallow so the thighs, not a wedge of cloth, carry the line down.
    if age < 11:
        rings = [(0.43, .052, .030, .002), (0.45, .074, .044, .002), (0.475, .082, .052, .002), (0.50, .082, .060, .001), (0.55, .081, .065, -.002),
                 (0.60, .080, .066, -.004), (0.66, .080, .063, -.003), (0.72, .081, .059, -.001), (0.765, .084, .055, 0), (0.79, .082, .049, .001),
                 (0.805, .058, .041, .002), (0.82, .029, .029, .003)]
    elif female:
        bust = interp(age, [(11, 0.0), (13, 0.5), (16, 0.9), (18, 1.0)])
        hip = interp(age, [(11, 0.94), (15, 1.0)])
        rings = [(0.43, .056 * hip, .030, .004), (0.45, .082 * hip, .042, .005), (0.475, .093 * hip, .050, .005), (0.50, .093 * hip, .058, .004),
                 (0.55, .083, .055, .001), (0.60, .068 + .006 * (1 - hip) * 10, .050, 0), (0.66, .073, .054, -.002),
                 (0.71, .081, .056 + .012 * bust, -.004 - .006 * bust), (0.75, .086, .055 + .010 * bust, -.004 - .005 * bust), (0.785, .089, .052, -.001),
                 (0.80, .070, .045, .001), (0.815, .045, .036, .002), (0.83, .027, .027, .003)]
    else:
        lean = interp(age, [(11, 0.9), (15, 0.95), (18, 1.0)])
        rings = [(0.43, .052, .030, .003), (0.45, .076, .042, .004), (0.475, .083, .049, .004), (0.50, .082, .056, .003), (0.55, .077, .057, .001),
                 (0.60, .074 + .004 * (1 - lean) * 10, .055, 0), (0.66, .080 * lean ** 0.5, .059, -.002), (0.72, .090 * lean, .064, -.004),
                 (0.76, .098 * lean, .063, -.004), (0.79, .103 * lean, .057, -.002), (0.805, .084 * lean, .049, 0), (0.818, .055, .041, .002), (0.83, .033, .033, .003)]
    if old:
        # the waist fills out and the torso goes squarer (and an old woman's hips a little wider)
        rings = [(z, x * (1 + 0.07 * math.exp(-((z - 0.62) / 0.06) ** 2) + (0.03 if female and z < 0.53 else 0)), y, yc) for z, x, y, yc in rings]
    if any(e[0] in ("collar_stand", "tie") for e in spec.get("extras", [])):
        # a jacket's padded shoulders (gakuran, suit): a touch wider and squarer
        rings = [(z, x * (1 + 0.05 * math.exp(-((z - 0.785) / 0.02) ** 2)), y, yc) for z, x, y, yc in rings]
    ring_yc = [(z, yc) for z, _, _, yc in rings]
    rings = [(z, x, y) for z, x, y, _ in rings]

    def torso_yc(z):
        # where the middle of the torso is, front to back, at height z (the chest forward, the seat back)
        return interp(z / H, ring_yc) * H

    def torso_w(z):
        zh = z / H
        if zh < 0.53:
            return blend("pelvis", "spine", (zh - 0.52) / 0.06)
        if zh < 0.63:
            return {"spine": 1.0}
        return blend("spine", "chest", (zh - 0.63) / 0.06)

    def girth(zh):
        # the belly (and a little round the waist): only the middle of the torso gets bigger
        return width * (1 + (g - 1) * math.exp(-((zh - 0.60) / 0.08) ** 2))

    pts = [(0, torso_yc(z * H), z * H) for z, _, _ in rings]
    radii = [(x * H * girth(z), y * H * girth(z)) for z, x, y in rings]
    mats = [pants_mat if z < 0.545 else "shirt" for z, _, _ in rings]
    B.loft(pts, radii, [torso_w(p[2]) for p in pts], mats, seg=24)

    def torso_at(z):
        zh = z / H
        for (z0, x0, y0), (z1, x1, y1) in zip(rings, rings[1:]):
            if z0 <= zh <= z1:
                t = (zh - z0) / (z1 - z0)
                return ((x0 + (x1 - x0) * t) * H * girth(zh), (y0 + (y1 - y0) * t) * H * girth(zh))
        return (rings[0][1] * H, rings[0][2] * H)

    # neck
    nr = (0.026 if female else 0.031) * H * (0.9 if age < 12 else 1.0)  # (anime necks: slim, a third of the head's width)
    neck_top = head_c.z - 0.75 * head_r
    B.loft([(0, 0, 0.80 * H), (0, 0.004 * H, (0.80 * H + neck_top) / 2), (0, 0.008 * H, neck_top)], [(nr, nr * 0.95)] * 3,
           [{"chest": 0.5, "neck": 0.5}, {"neck": 1.0}, {"neck": 0.5, "head": 0.5}], ["skin"], seg=14)

    # head: a sphere with a small, pointed chin; the face painted on its front
    kid = min(max((14 - age) / 10, 0), 1)  # (1 for small children, 0 from 14 up)
    eye_x, eye_z = (0.17 + 0.03 * kid) * 2.2 * head_r, (0.405 - 0.035 * kid - 0.5) * 2.2 * head_r  # (where paint_face puts the eyes)

    def head_shape(lp):
        # anime construction (docs/anime_faces.md): a round skull (the top half, which the hair
        # follows, is left alone); below the eyes the cheeks stay full, then the jaw runs in to a
        # small chin a little under the circle; children's faces rounder and shorter
        x, y, z = lp.x, lp.y, lp.z
        if z < 0:
            t = -z
            front = max(0.0, -y)
            x *= 1 - (0.40 - 0.14 * kid) * t ** 1.8
            if y > 0:
                y *= 1 - 0.35 * t
            else:
                y *= 1 + (0.14 - 0.06 * kid) * t ** 2  # (the lower face forward: flatter, with a chin)
            z -= (0.17 - 0.1 * kid) * t ** 2 * front ** 2
        z *= 1.1
        return Vector((x, y, z))

    def face_uv(lp):
        if lp.y < -0.15:
            return (0.5 + lp.x / 2.2, 0.5 + lp.z * 1.1 / 2.2)
        return (0.02, 0.98)  # (the back of the head: a plain bit of skin)

    B.ellipsoid(head_c, (head_r, head_r, head_r), {"head": 1.0}, "face", seg=40, rings=30, deform=head_shape, uvfn=face_uv)
    # ears: from just under the eye line down to the nose line, set just behind the middle of the
    # side of the head; rounded shells, dished on the outside, the top leaning back. (None where
    # the hair hangs over them.)
    if spec["hair"]["style"] not in ("Long", "Shoulder", "Bob"):
        ez = head_r * (1.12 if old else 1.0) * (1 - 0.08 * kid)
        for sx in (-1, 1):
            q = head_shape(Vector((sx * 0.94, 0.12, -0.32)).normalized())
            ec = head_c + q * head_r + Vector((sx * 0.06 * head_r, 0, 0))

            def ear_shape(lp, sx=sx):
                x, y, z = lp.x, lp.y, lp.z
                if x * sx > 0:
                    x -= sx * 0.8 * max(0.0, 1 - (y * y + z * z) * 1.6) * abs(x)
                return Vector((x, y * (1 - 0.3 * max(0.0, -z)), z))  # (narrower at the lobe)

            B.ellipsoid(ec, (0.055 * ez, 0.12 * ez, 0.2 * ez), {"head": 1.0}, "skin", rot=Euler((math.radians(-12), 0, math.radians(-sx * 22))).to_matrix(),
                        seg=16, rings=10, deform=ear_shape)

    # arms (anime guides: slim and smooth, a soft shoulder, tapering to a small wrist; upper arm ~
    # forearm + palm). Sleeve then skin; a sleeve's edge stands off the arm a little (a lip), and so
    # do arm covers over the forearms for farm work. Hands: a mitten (palm and the fingers held
    # together, curling a little towards the thigh) with a thumb lying along the front.
    sleeve = spec.get("sleeve", 1.0)
    ar = (0.88 if female else 1.0) * (0.92 if age < 12 else 1.0) * width
    kid = age < 10
    ak = H * ar * g ** 0.3
    # (along the arm, f: 0 the shoulder, 0.5 the elbow, 1 the wrist) -> half-widths (side, front), in H
    prof = [(0.0, 0.036, 0.037), (0.12, 0.033, 0.034), (0.3, 0.0285, 0.029), (0.5, 0.0235, 0.025),
            (0.6, 0.0245, 0.0275), (0.8, 0.0195, 0.0225), (1.0, 0.0145, 0.019)]
    if kid:  # (small children: softer, rounder arms that hardly taper)
        prof = [(f, x + (0.026 - x) * 0.4, y + (0.027 - y) * 0.4) for f, x, y in prof]
    cover_from = 0.55 if spec.get("armcovers") else None

    def arm_mat(f):
        if cover_from is not None and f > cover_from:
            return "armcover"
        return "shirt" if f < sleeve else "skin"

    def arm_loose(f):
        # (cloth stands off the arm a little; arm covers are baggy)
        return {"skin": 1.0, "shirt": 1.05, "armcover": 1.12}[arm_mat(f)]

    arm_geo = {}
    for s in ("l", "r"):
        sh, el = J[f"upperarm_{s}"]
        _, wr = J[f"lowerarm_{s}"]
        ua, la, hd = f"upperarm_{s}", f"lowerarm_{s}", f"hand_{s}"
        start = sh + Vector((-sh.x * 0.12, 0, -0.022 * H))

        def arm_at(f, start=start, el=el, wr=wr):
            return start + (el - start) * (f / 0.5) if f <= 0.5 else el + (wr - el) * ((f - 0.5) / 0.5)

        def arm_r(f):
            return (interp(f, [(p[0], p[1]) for p in prof]) * ak * arm_loose(f),
                    interp(f, [(p[0], p[2]) for p in prof]) * ak * arm_loose(f))

        def arm_w(f, ua=ua, la=la, hd=hd):
            if f < 0.12:
                return blend("chest", ua, 0.65 + 0.35 * f / 0.12)
            if f < 0.4:
                return {ua: 1.0}
            if f < 0.6:
                return blend(ua, la, (f - 0.4) / 0.2)
            if f < 0.9:
                return {la: 1.0}
            return blend(la, hd, min(f - 0.9, 0.1) / 0.1 * 0.3)

        arm_geo[s] = (arm_at, arm_r, arm_w)
        # rings: (f, extra scale, material of the band to the next ring)
        edges = [e for e in (sleeve if 0.04 < sleeve < 0.97 else None, cover_from) if e is not None]
        rs = [(f, 1.0, arm_mat(f + 1e-3)) for f in (0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9)
              if all(abs(f - e) > 0.035 for e in edges)]
        if sleeve in edges:  # the sleeve's edge: a lip, then the bare arm
            rs += [(sleeve - 0.025, 1.04, "shirt"), (sleeve, 1.1, "shirt"), (sleeve + 0.004, 0.96, arm_mat(sleeve + 0.01))]
        if cover_from is not None:  # the arm cover's elastic top
            rs += [(cover_from - 0.004, 0.97, "armcover"), (cover_from, 1.08, "armcover"), (cover_from + 0.03, 1.05, "armcover")]
        if arm_mat(0.99) == "skin":
            rs += [(1.0, 1.0, "skin")]
            cap = "skin"
        else:  # a cuff at the wrist, open: the hand comes out of it
            rs = [r for r in rs if r[0] < 0.88] + [(0.92, 1.03, arm_mat(0.99)), (0.995, 1.12, arm_mat(0.99)), (1.0, 0.72, arm_mat(0.99))]
            cap = "skin"
        rs.sort(key=lambda r: r[0])
        B.loft([arm_at(f) for f, _, _ in rs], [(arm_r(f)[0] * k, arm_r(f)[1] * k) for f, k, _ in rs],
               [arm_w(f) for f, _, _ in rs], [m for _, _, m in rs[:-1]] + [cap], seg=12)

        # the hand, in its own frame: down the arm, forward, and in (towards the thigh)
        hl = 0.09 * H * (0.94 if female else 1.0) * width ** 0.5  # (wrist to fingertip)
        dn = (wr - el).normalized()
        fw = FRONT.copy()
        fw = (fw - dn * fw.dot(dn)).normalized()
        inw = Vector((-1 if s == "l" else 1, 0, 0))
        inw = (inw - dn * inw.dot(dn) - fw * inw.dot(fw)).normalized()

        def hp(d, f, i):
            return wr + (dn * d + fw * f + inw * i) * hl

        fl = 0.85 if kid else 1.0  # (children: shorter fingers, plumper palms)
        fat = (1.15 if kid else 1.0) * (0.93 if old else 1.0) * (0.94 if female else 1.0)
        # the palm (wrist, heel, middle, knuckles: about as long as the fingers)
        palm = [(0.0, 0.0, 0.0, 0.12, 0.17), (0.12, 0.0, 0.0, 0.125, 0.205), (0.3, 0.01, 0.012, 0.13, 0.25), (0.48, 0.015, 0.022, 0.112, 0.245)]
        B.loft([hp(d, f, i) for d, f, i, _, _ in palm], [(tx * hl * fat, wy * hl * fat) for *_, tx, wy in palm],
               [blend(la, hd, 0.55)] + [{hd: 1.0}] * (len(palm) - 1), ["skin"], seg=10)
        # the fingers, drawn as two blocks held together (index and middle; ring and little finger,
        # a little shorter), curling a little towards the thigh
        for fo, ln in ((0.115, 0.4), (-0.12, 0.33)):
            fb = [(0.42, 0.015 + fo, 0.02, 0.1, 0.12), (0.5 + 0.45 * ln * fl, 0.02 + fo, 0.055, 0.085, 0.112),
                  (0.5 + 0.88 * ln * fl, 0.02 + fo * 0.9, 0.115, 0.068, 0.095)]
            B.loft([hp(d, f, i) for d, f, i, _, _ in fb], [(tx * hl * fat, wy * hl * fat) for *_, tx, wy in fb],
                   [{hd: 1.0}] * len(fb), ["skin"], seg=8, cap_start=False)
        # the thumb: from the heel of the palm, along the front, its tip at the first knuckle
        th = [(0.12, 0.13, 0.04, 0.082, 0.095), (0.28, 0.24, 0.07, 0.078, 0.085), (0.42, 0.265, 0.09, 0.068, 0.074), (0.42 + 0.1 * fl, 0.25, 0.1, 0.056, 0.062)]
        B.loft([hp(d, f, i) for d, f, i, _, _ in th], [(tx * hl * fat, wy * hl * fat) for *_, tx, wy in th],
               [{hd: 1.0}] * len(th), ["skin"], seg=8)

    # legs: trousers, shorts or skin (under a skirt), socks; shoes or boots
    cover = spec.get("pants_cover", 1.0)
    socks = spec.get("socks")
    lr = (0.9 if female else 1.0) * width
    style = shoe_style(spec)
    boots = style == "boots"
    trousers = cover >= 1.0 and not boots
    sd = shoe_dims(spec, H)
    sole_col = {"sneaker": (238, 236, 230), "loafer": (34, 30, 28)}.get(style, tuple(int(c * 0.7) for c in spec["shoes"]))
    lg = lr * g ** 0.4
    for s in ("l", "r"):
        hip, kn = J[f"thigh_{s}"]
        _, an = J[f"calf_{s}"]
        sx = 1 if s == "l" else -1

        def at(u):
            # a point on the bone line: above the hip (u < 0, inside the hips), down to the
            # ankle, and on below it (u > 1: a trouser hem over the shoe)
            if u <= 0.5:
                p = hip + (kn - hip) * (2 * u)
                if u < 0:
                    p = p + Vector((-sx * 0.3 * abs(u) * H, 0, 0))  # (into the hips, towards the middle)
                return p
            return kn + (an - kn) * (2 * u - 1)

        def weight(u):
            if u < 0:
                return {"pelvis": 0.65, f"thigh_{s}": 0.35}
            if u < 0.06:
                return blend("pelvis", f"thigh_{s}", 0.35 + 0.65 * u / 0.06)
            if u < 0.42:
                return {f"thigh_{s}": 1.0}
            if u < 0.58:
                return blend(f"thigh_{s}", f"calf_{s}", (u - 0.42) / 0.16)
            if u < 0.95:
                return {f"calf_{s}": 1.0}
            return blend(f"calf_{s}", f"foot_{s}", min(0.45, 0.2 + (u - 0.95) * 2.5))
        us = [-0.07, -0.03, 0.0, 0.06, 0.12, 0.2, 0.3, 0.38, 0.44, 0.5, 0.56, 0.62, 0.7, 0.78, 0.86, 0.94, 1.0]
        hem = None
        if 0 < cover < 1:
            hem = cover
            us = sorted([u for u in us if abs(u - hem) > 0.02] + [hem - 0.025, hem, hem + 0.003])
        sock_top = 1 - socks[1] if socks and socks[1] < 1 else None
        if sock_top is not None:
            us = sorted([u for u in us if abs(u - sock_top) > 0.015] + [sock_top - 0.003, sock_top])
        if trousers:
            us = [u for u in us if u < 0.95] + [1.0, 1.07]

        def dressed(u):
            # what covers the band just below u
            if cover >= 1.0 or (hem is not None and u <= hem + 1e-9):
                return "pants"
            if socks and (sock_top is None or u >= sock_top):
                return "socks"
            return "skin"
        pts, radii, ws, mats = [], [], [], []
        for i, u in enumerate(us):
            rx, ry, off = leg_profile(min(u, 1.0), female, age, old)
            m = dressed(u)
            if trousers and u > 0.3:
                # straight trouser legs: no calf, no ankle; a little wider at the hem
                k = min(1.0, (u - 0.3) / 0.2)
                rx = rx * (1 - k) + (0.040 + 0.002 * max(0.0, u - 0.9) * 10) * k
                ry = ry * (1 - k) + (0.042 + 0.002 * max(0.0, u - 0.9) * 10) * k
                off *= 1 - k
            elif m == "pants" or (hem is not None and abs(u - hem) < 1e-6):
                rx, ry = rx * 1.08 + 0.003, ry * 1.08 + 0.003  # (shorts: cloth stands off the leg, the hem a little open)
                if hem is not None and u > hem - 0.03:
                    rx, ry = rx * 1.05, ry * 1.05
            elif m == "socks":
                rx, ry = rx * 1.03, ry * 1.03
            if trousers and u > 1.0:
                pts.append(an + (an - kn).normalized() * 0.026 * H + Vector((0, 0.004 * H, 0)))
            else:
                pts.append(at(u) + Vector((0, off * H, 0)))
            radii.append((rx * H * lg, ry * H * lg))
            ws.append(weight(u))
            mats.append(m)
        B.loft(pts, radii, ws, mats[:-1] if len(mats) > 1 else mats, seg=18, cap_start=False, cap_end=not trousers)
        if trousers:  # (the inside of the hem)
            B.loft(pts[-3:][::-1], [(a * 0.95, b * 0.95) for a, b in radii[-3:][::-1]], ws[-3:][::-1], ["pants"], seg=18, cap_start=False, cap_end=False)
        if boots:
            # rubber boots to just under the knee, the trousers tucked in
            bz = [0.03, 0.12, 0.2, 0.235]
            bu = [0.5 + 0.5 * (kn.z - z * H) / (kn.z - an.z) for z in bz]
            bp = [at(min(u, 1.0)) if z * H < an.z else at(u) for u, z in zip(bu, bz)]
            bp[0] = Vector((an.x, an.y, bz[0] * H))
            br = []
            for u in bu:
                rx, ry, _ = leg_profile(min(u, 1.0), female, age, old)
                br.append((max(rx, 0.03) * H * lg + 0.008 * H, max(ry, 0.033) * H * lg + 0.009 * H))
            br[-1] = (br[-1][0] * 1.06, br[-1][1] * 1.06)
            bw = [blend(f"calf_{s}", f"foot_{s}", 0.5), {f"calf_{s}": 1.0}, {f"calf_{s}": 1.0}, {f"calf_{s}": 1.0}]
            B.loft(bp, br, bw, ["shoes"], seg=18, cap_start=False, cap_end=False)
            B.loft(bp[::-1], [(a * 0.93, b * 0.93) for a, b in br[::-1]], bw[::-1], ["shoes"], seg=18, cap_start=False, cap_end=False)
        build_shoe(B, an, H, sd, lr, style, s, {f"foot_{s}": 1.0})

    # a skirt from the waist: school skirts in knife pleats (stitched flat over the hips, opening
    # towards the hem), other skirts in a few soft folds; it clears the hips and seat, and its
    # lower part follows the thighs half and half, so the hem swings without tearing
    if skirt:
        z0 = 0.57 * H
        z1 = skirt["to"] * H
        pleats = skirt.get("pleats", 0)
        seg = pleats * 2 if pleats else 32
        n = 9
        hw0, hd0 = torso_at(z0)
        pts, radii, ws, depth = [], [], [], []
        for i in range(n):
            t = i / (n - 1)
            z = z0 + (z1 - z0) * t
            fl = 1 + (skirt["flare"] - 1) * t ** 0.65
            bw, bd = torso_at(max(z, 0.4 * H)) if z > 0.4 * H else (0, 0)
            rx = max(hw0 * fl + 0.004 * H, bw * 1.05 + 0.006 * H)
            ry = max(hd0 * fl + 0.004 * H, bd * 1.08 + 0.007 * H)
            # (and the legs: round both thighs, standing)
            hip_l, kn_l = J["thigh_l"]
            uu = 0.5 * (hip_l.z - z) / (hip_l.z - kn_l.z)
            if uu > -0.07:
                lx, ly, _ = leg_profile(min(uu, 1.0), female, age, old)
                lx, ly = lx * H * lr * g ** 0.4, ly * H * lr * g ** 0.4
                hx = hip_l.x + (kn_l.x - hip_l.x) * max(0.0, 2 * uu)
                yc = torso_yc(max(z, 0.42 * H)) + 0.006 * H * t
                need = 0.0
                for j in range(24):
                    ph = 2 * math.pi * j / 24
                    kf = 0.22 * H * math.sin(math.acos(1 - leg_crouch(spec) / 0.44)) * min(1.0, 2 * max(uu, 0.0))
                    px, py = hx + lx * math.cos(ph), ly * math.sin(ph) - kf
                    need = max(need, math.hypot(px / rx, (py - yc) / ry))
                if need * 1.1 > 1:
                    rx, ry = rx * need * 1.1, ry * need * 1.1
            if radii:  # (an A-line: never narrower further down)
                rx, ry = max(rx, radii[-1][0] * 1.005), max(ry, radii[-1][1] * 1.005)
            pts.append((0, torso_yc(max(z, 0.42 * H)) + 0.006 * H * t, z))
            radii.append((rx, ry))
            depth.append(max(0.0, (t - 0.12) / 0.88) ** 0.8)

            def w(p, t=t, rx=rx):
                k = (0.75 if skirt["to"] < 0.25 else 0.7) * max(0.0, (t - 0.1) / 0.9) ** 0.55
                wl = min(1.0, max(0.0, 0.5 + 0.5 * p.x / (0.55 * rx)))
                wl = wl * wl * (3 - 2 * wl)
                base = blend("spine", "pelvis", min(1, t * 3))
                out = {j: x * (1 - k) for j, x in base.items()}
                out["thigh_l"] = k * wl
                out["thigh_r"] = k * (1 - wl)
                return {j: x for j, x in out.items() if x > 1e-4}
            ws.append(w)
        v0 = len(B.v)
        B.loft(pts, radii, ws, ["skirt"], seg=seg, cap_start=False, cap_end=False)
        v1 = len(B.v)
        B.loft(pts[::-1], [(a * 0.985, b * 0.985) for a, b in radii[::-1]], ws[::-1], ["skirt"], seg=seg, cap_start=False, cap_end=False)
        # pleats/folds: push each ring's points in and out round the skirt
        for base, order in ((v0, range(n)), (v1, range(n - 1, -1, -1))):
            for ri, i in enumerate(order):
                c = Vector(pts[i])
                for j in range(seg):
                    vi = base + ri * seg + j
                    p = Vector(B.v[vi])
                    d = p - c
                    d.z = 0
                    if pleats:
                        # knife pleats: each pleat's fold out, then a short step in (a saw-tooth)
                        a = (j % 2) * 1.0
                        amt = (0.07 * a - 0.03) * depth[i]
                    else:
                        amt = 0.022 * math.sin(2 * math.pi * j / seg * 7) * depth[i] * (1 if skirt["flare"] > 1.3 or skirt["to"] < 0.25 else 0.4)
                    q = p + d * amt
                    if pleats and j % 2 == 1:
                        # (the step: turned to sit just after the fold, so the pleat leans one way)
                        ang = 2 * math.pi / seg * 0.6 * depth[i]
                        dx, dy = q.x - c.x, q.y - c.y
                        q.x = c.x + dx * math.cos(ang) - dy * math.sin(ang)
                        q.y = c.y + dx * math.sin(ang) + dy * math.cos(ang)
                    B.v[vi] = tuple(q)
                    if pleats and ri > 0:
                        # (crisp pleat edges: the fold lines are sharp)
                        B.sharp = getattr(B, "sharp", set())
                        B.sharp.add((vi - seg, vi))

    # clothes' details
    extras = dict(spec.get("extras", []))

    def front_y(z, out=0.004):
        return torso_yc(z) - (torso_at(z)[1] + out * H)

    def front_at(z):
        # (half width, how far the front is from y=0): for the cloth panels (slab) on the front
        hw, hd = torso_at(z)
        return (hw, hd - torso_yc(z))

    def on_front(x, z, out):
        hw, hd = torso_at(z)
        k = min(abs(x) / max(hw, 1e-4), 0.97)
        return Vector((x, torso_yc(z) - hd * math.sqrt(1 - k * k) - out * H, z))

    def band(z0, z1, cx, hw_, out, mat, n=8, nx=4, thick=0.0015, w=None):
        """A flat strip of cloth lying on the front of the torso, top (z0) to bottom (z1): its middle
        at cx(t), half width hw_(t), t from 0 at the top to 1 at the bottom (a tie, a lapel, a sailor
        collar's front, its stripes, apron straps)."""
        m = B.mat(mat)
        F, Bk = [], []
        for i in range(n + 1):
            t = i / n
            z = z0 + (z1 - z0) * t
            c, h = cx(t) * H, hw_(t) * H
            ww = w(z) if w else torso_w(z)
            rf, rb = [], []
            for j in range(nx + 1):
                p = on_front(c + h * (-1 + 2 * j / nx), z, out)
                rf.append(B.vert(p, ww))
                rb.append(B.vert(p + Vector((0, thick * H, 0)), ww))
            F.append(rf)
            Bk.append(rb)
        for i in range(n):
            for j in range(nx):
                B.face([F[i][j], F[i + 1][j], F[i + 1][j + 1], F[i][j + 1]], m)
                B.face([Bk[i][j], Bk[i][j + 1], Bk[i + 1][j + 1], Bk[i + 1][j]], m)
            for a, b in ((F, Bk), (Bk, F)):
                j = 0 if a is F else nx
                B.face([a[i][j], b[i][j], b[i + 1][j], a[i + 1][j]], m)
        for j in range(nx):
            B.face([F[0][j], F[0][j + 1], Bk[0][j + 1], Bk[0][j]], m)
            B.face([F[n][j], Bk[n][j], Bk[n][j + 1], F[n][j + 1]], m)

    def tube(path, r, w, mat):
        """A thin round cord (apron and randoseru straps)."""
        B.loft(path, [(r, r)] * len(path), [w] * len(path), [mat], seg=8)

    def around(zs, rs, mat, w, seg=24):
        """An open band round the torso through heights zs (rs: (half width, half depth) at each),
        seen from outside and in: a jacket's hem."""
        p = [(0, torso_yc(z), z) for z in zs]
        B.loft(p, rs, [w] * len(p), [mat], seg=seg, cap_start=False, cap_end=False)
        B.loft(p[::-1], [(x * 0.97, y * 0.97) for x, y in rs[::-1]], [w] * len(p), [mat], seg=seg, cap_start=False, cap_end=False)

    def panel(top, bottom, width, depth_at, wfn, mat, nx=10, nz=10, thick=0.004):
        """Like Build.slab (a cloth panel hanging on the front), but weighted by where each point
        is across it too (wfn(z, x)), so the part over a thigh follows the thigh when walking."""
        m = B.mat(mat)
        gf, gb = [], []
        for i in range(nz + 1):
            z = top + (bottom - top) * i / nz
            hw, hd = depth_at(z)
            rf, rb = [], []
            for j in range(nx + 1):
                x = (-1 + 2 * j / nx) * width / 2
                k = min(abs(x) / max(hw, 1e-3), 0.98)
                y = -(hd * math.sqrt(1 - k * k)) - 0.012
                rf.append(B.vert((x, y - thick, z), wfn(z, x)))
                rb.append(B.vert((x, y, z), wfn(z, x)))
            gf.append(rf)
            gb.append(rb)
        for i in range(nz):
            for j in range(nx):
                B.face([gf[i][j], gf[i + 1][j], gf[i + 1][j + 1], gf[i][j + 1]], m)
                B.face([gb[i][j], gb[i][j + 1], gb[i + 1][j + 1], gb[i + 1][j]], m)
            for j in (0, nx):
                a_, b_ = (gf, gb) if j == 0 else (gb, gf)
                B.face([a_[i][j], a_[i + 1][j], b_[i + 1][j], b_[i][j]], m)
        for j in range(nx):
            B.face([gf[nz][j], gb[nz][j], gb[nz][j + 1], gf[nz][j + 1]], m)
            B.face([gf[0][j], gf[0][j + 1], gb[0][j + 1], gb[0][j]], m)

    # (how far out the thighs reach, front and side, for cloth hanging over them)
    thigh_d = (0.05 * width * (0.9 if female else 1.0) * g ** 0.4) * H
    thigh_w = 0.05 * width * H + thigh_d

    def collar_band(z0, z1, k0, k1, mat):
        # a band standing round the neck (k: times the neck's radius at the bottom and top)
        p = [(0, 0.002 * H, z0), (0, 0.004 * H, z1)]
        w = {"chest": 0.5, "neck": 0.5}
        B.loft(p, [(nr * k0, nr * k0 * 0.97), (nr * k1, nr * k1 * 0.97)], [w, w], [mat], seg=20, cap_start=False, cap_end=False)
        B.loft(p[::-1], [(nr * k1 * 0.95, nr * k1 * 0.92), (nr * k0 * 0.95, nr * k0 * 0.92)], [w, w], [mat], seg=20, cap_start=False, cap_end=False)

    jacket = "collar_stand" in extras or ("tie" in extras and "shirt_v" in extras)
    if jacket and not skirt:
        # the jacket's hem over the hips (gakuran, suit): the bottom third of a gakuran has no buttons
        zs = [0.565 * H, 0.52 * H, 0.47 * H]
        rs = []
        for z in zs:
            hw, hd = torso_at(z)
            k = min(1.0, max(0.0, (0.53 * H - z) / (0.06 * H)))  # (below the hips, clear of the thighs)
            rs.append((max(hw * 1.05, k * (thigh_w + 0.006 * H)), max(hd * 1.08, k * (thigh_d + 0.009 * H))))
        around(zs, rs, "shirt", lambda p: blend("spine", "pelvis", min(1.0, max(0.0, (0.565 * H - p.z) / (0.06 * H)))))
    if "collar_stand" in extras:
        # the gakuran's stiff standing collar, and the white inner collar just showing over its top
        collar_band(0.80 * H, 0.838 * H, 1.32, 1.18, "collar")
        B.ring(Vector((0, 0.004 * H, 0.838 * H)), Matrix.Identity(3), nr * 1.13, 0.0022 * H, {"neck": 1.0}, "shirtv", seg=20, tube=5)
    if "buttons" in extras:
        for k in range(5):
            z = (0.775 - 0.046 * k) * H
            B.ellipsoid(Vector((0, front_y(z, 0.001), z)), (0.0055 * H, 0.004 * H, 0.0055 * H), torso_w(z), "buttons", seg=8, rings=5)
    if "sailor_collar" in extras:
        # the sailor collar: a square flap down the back edged with white lines, lying over the
        # shoulders, and its two fronts coming to a V on the chest where the scarf is tied
        hw, hd = torso_at(0.79 * H)
        B.ellipsoid(Vector((0, torso_yc(0.79 * H) + 0.003 * H, 0.792 * H)), (hw * 1.07, hd * 1.18, 0.026 * H), {"chest": 1.0}, "collar", seg=28, rings=10,
                    keep=lambda lp: lp.z > -0.3 and not (lp.y < -0.2 and abs(lp.x) < 0.5 - 0.45 * lp.y))
        bz = 0.735 * H
        bd = torso_yc(bz) + torso_at(bz)[1]

        def square(lp):
            return Vector((math.copysign(abs(lp.x) ** 0.35, lp.x), lp.y, math.copysign(abs(lp.z) ** 0.35, lp.z)))
        for off, sx_, sz_, m_ in ((0.006, 0.084, 0.066, "collar"), (0.0075, 0.079, 0.061, "stripes"), (0.009, 0.073, 0.055, "collar")):
            B.ellipsoid(Vector((0, bd + off * H, bz)), (sx_ * H, 0.003 * H, sz_ * H), {"chest": 1.0}, m_, seg=12, rings=8, deform=square)
        for sx in (-1, 1):
            band(0.802 * H, 0.712 * H, lambda t, sx=sx: sx * (0.046 - 0.036 * t), lambda t: 0.021 - 0.014 * t, 0.002, "collar", w=lambda z: {"chest": 1.0})
            band(0.80 * H, 0.716 * H, lambda t, sx=sx: sx * (0.060 - 0.046 * t), lambda t: 0.0022, 0.0034, "stripes", w=lambda z: {"chest": 1.0})
    if "scarf" in extras:
        z = 0.712 * H
        B.ellipsoid(Vector((0, front_y(z, 0.008), z)), (0.014 * H, 0.008 * H, 0.011 * H), {"chest": 1.0}, "scarf", seg=12, rings=6)
        for sx in (-1, 1):
            band(0.708 * H, 0.655 * H, lambda t, sx=sx: sx * (0.006 + 0.008 * t), lambda t: 0.007 + 0.004 * t, 0.006, "scarf", n=5, nx=3, w=lambda z: {"chest": 1.0})
    if "shirt_v" in extras:
        # the shirt in the open front of a jacket or cardigan, and its collar points
        band(0.808 * H, 0.655 * H, lambda t: 0.0, lambda t: 0.036 * (1 - t) ** 0.9 + 0.001, 0.0008, "shirtv", n=8, nx=6)
        if "tie" in extras:
            for sx in (-1, 1):
                band(0.818 * H, 0.784 * H, lambda t, sx=sx: sx * (0.014 + 0.010 * t), lambda t: 0.010 * (1 - t) + 0.002, 0.0018, "shirtv", n=4, nx=3)
    if "tie" in extras:
        # a flat tie, widening to a point, under a small knot
        tz = 0.796 * H
        B.ellipsoid(Vector((0, front_y(tz, 0.004), tz)), (0.009 * H, 0.005 * H, 0.010 * H), {"chest": 1.0}, "tie", seg=10, rings=6)
        band(0.79 * H, 0.612 * H, lambda t: 0.0, lambda t: (0.006 + 0.009 * min(t / 0.86, 1.0)) * (1.0 if t < 0.86 else max(0.05, (1 - t) / 0.14)), 0.0035, "tie", n=10, nx=4)
    if jacket and "tie" in extras:
        # the suit jacket's lapels along the edge of the V, and its button
        for sx in (-1, 1):
            band(0.812 * H, 0.645 * H, lambda t, sx=sx: sx * (0.046 - 0.040 * t), lambda t: (0.012 + 0.007 * math.sin(math.pi * min(t * 1.6, 1.0))) * (1 - 0.7 * t), 0.0028, "shirt", n=9, nx=4)
        z = 0.63 * H
        B.ellipsoid(Vector((0, front_y(z, 0.002), z)), (0.006 * H, 0.003 * H, 0.006 * H), torso_w(z), "tie_band", seg=8, rings=5)
    if "vest" in extras:
        B.slab(0.775 * H, 0.555 * H, 0.165 * H, front_at, lambda z: torso_w(z), "vest", nx=6, nz=6, thick=0.003)
    if "stripes" in extras:
        # (a flat stripe down the outside of each sleeve, shoulder to cuff)
        for s in ("l", "r"):
            arm_at, arm_r, arm_w = arm_geo[s]
            out = Vector((1 if s == "l" else -1, 0, 0))
            fs = [0.04, 0.15, 0.3, 0.45, 0.55, 0.7, 0.85, 0.95]
            B.loft([arm_at(f) + out * (arm_r(f)[0] + 0.0012 * H) for f in fs], [(0.0022 * H, 0.0062 * H)] * len(fs),
                   [arm_w(f) for f in fs], ["stripes"], seg=6)
    if "wrap" in extras:
        # the samue's crossed front: the wearer's left over the right, a dark band along each edge
        band(0.818 * H, 0.60 * H, lambda t: 0.03 - 0.075 * t ** 1.2, lambda t: 0.0095, 0.003, "wrap", n=12, nx=3)
        band(0.818 * H, 0.712 * H, lambda t: -0.03 + 0.026 * t, lambda t: 0.0095, 0.0016, "wrap", n=6, nx=3)
    if "belt" in extras:
        z = 0.555 * H
        hw, hd = torso_at(z)
        B.loft([(0, torso_yc(z), z - 0.008 * H), (0, torso_yc(z), z + 0.008 * H)], [(hw * 1.03, hd * 1.04)] * 2, [torso_w(z)] * 2, ["belt"], seg=24, cap_start=False, cap_end=False)
        B.ellipsoid(Vector((0, torso_yc(z) - (hd * 1.04) - 0.002 * H, z)), (0.012 * H, 0.003 * H, 0.009 * H), torso_w(z), "buttons", seg=8, rings=4)
    if "zip" in extras:
        band(0.80 * H, 0.56 * H, lambda t: 0.0, lambda t: 0.0025, 0.001, "zip", n=8, nx=2)
    if "pocket" in extras:
        z = 0.73 * H
        B.ellipsoid(on_front(0.04 * H, z, 0.0), (0.022 * H, 0.004 * H, 0.025 * H), {"chest": 1.0}, "zip", seg=10, rings=6,
                    deform=lambda lp: Vector((math.copysign(abs(lp.x) ** 0.4, lp.x), lp.y, math.copysign(abs(lp.z) ** 0.4, lp.z))))
    if "collar_flat" in extras:
        # a shirt collar: a low band round the neck, its points folded down on the chest
        collar_band(0.798 * H, 0.826 * H, 1.42, 1.25, "collarflat")
        for sx in (-1, 1):
            band(0.812 * H, 0.772 * H, lambda t, sx=sx: sx * (0.016 + 0.014 * t), lambda t: 0.013 * (1 - t) + 0.002, 0.002, "collarflat", n=4, nx=3)
    if "hood" in extras:
        z = 0.795 * H
        bd = torso_yc(z) + torso_at(z)[1]
        B.ellipsoid(Vector((0, bd * 0.85, 0.80 * H)), (0.07 * H, 0.032 * H, 0.038 * H), {"chest": 1.0}, "shirt", seg=16, rings=8)
        for sx in (-1, 1):
            band(0.80 * H, 0.745 * H, lambda t, sx=sx: sx * 0.014, lambda t: 0.0016, 0.003, "buttons", n=4, nx=1)
    if "apron" in extras:
        # a shop apron: a bib on the chest on a strap round the neck, tied at the waist, the skirt
        # of it wider and wrapping round the hips
        def apron_at(z):
            hw, hd = torso_at(max(z, 0.47 * H))
            hd -= torso_yc(max(z, 0.47 * H))
            if not skirt and z < 0.53 * H:
                k = min(1.0, (0.53 * H - z) / (0.06 * H))
                hw, hd = max(hw, k * thigh_w * 1.6), max(hd, k * (thigh_d + 0.006 * H))  # (flatter across, so it stays over the thighs)
            if skirt and z < 0.57 * H:
                # over the skirt: as the skirt is cut (from the torso at its waist), a little outside it
                hw, hd = torso_at(0.57 * H)
                t = min(1.0, (0.57 * H - z) / max(0.57 * H - skirt["to"] * H, 1e-3))
                fl = 1 + (skirt["flare"] - 1) * t ** 0.8
                hw, hd = hw * 1.06 * fl + 0.006 * H, hd * 1.15 * fl + (0.006 + 0.004 * t) * H
            return (hw, hd)
        aw = lambda z: torso_w(z) if z > 0.5 * H else {"pelvis": 1.0}
        B.slab(0.765 * H, 0.57 * H, 0.15 * H, apron_at, aw, "apron", nx=6, nz=8)
        def aw2(z, x):
            # over the thighs the apron's skirt goes with them (the middle less, so it hangs between them)
            if z > 0.5 * H:
                return torso_w(z)
            k = 0.85 * min(1.0, (0.5 * H - z) / (0.10 * H)) * min(1.0, abs(x) / (0.05 * H)) ** 0.7
            return {"pelvis": 1 - k, "thigh_l" if x > 0 else "thigh_r": k}
        panel(0.575 * H, (skirt["to"] + 0.03 if skirt and skirt["to"] > 0.3 else 0.33) * H, 0.23 * H, apron_at, aw2, "apron", nx=10, nz=10)
        z = 0.572 * H
        hw, hd = torso_at(z)
        B.loft([(0, torso_yc(z), z - 0.006 * H), (0, torso_yc(z), z + 0.006 * H)], [(hw * 1.07, hd * 1.08)] * 2, [torso_w(z)] * 2, ["apron"], seg=24, cap_start=False, cap_end=False)
        for sx in (-1, 1):
            z0 = 0.762 * H
            p0 = on_front(sx * 0.068 * H, z0, 0.014)
            tube([p0, Vector((sx * 0.05 * H, torso_yc(0.80 * H) - torso_at(0.80 * H)[1] * 0.6, 0.805 * H)),
                  Vector((sx * nr * 1.15, 0.0, 0.825 * H)), Vector((0, nr * 1.15, 0.83 * H))], 0.0035 * H, {"chest": 0.6, "neck": 0.4}, "apron")

    # hair (anime: thought of as a few big clumps, not strands): a shell over the skull that
    # stands off it on top (the hair's volume), its edge on a smooth hairline and tucked in to the
    # scalp; over it, locks: flat, slightly curved ribbons, full a little way from the root and
    # tapering to a point (or cut blunt), fanning from the crown or a part down over the shell to
    # the hairline and past it. Under a hat the hair keeps inside the hat's crown and only shows
    # below its band.
    hs = spec["hair"]
    st = hs["style"]
    hr = head_r * 1.07
    acc = set(spec.get("accessories", []))
    hatted = bool(acc & {"YellowHat", "StrawHat", "Cap"})
    hw_ = {"head": 1.0}
    hid = spec.get("id", 0)
    deg = math.radians

    def on_head(lp, scale=1.0):
        q = head_shape(Vector(lp).normalized())
        return head_c + Vector((q.x * hr * scale, q.y * hr * scale, q.z * hr * scale))

    def jit(k):
        """A steady per-person wobble in about [-1, 1] (the lengths of locks)."""
        return 0.6 * math.sin(hid * 12.9898 + k * 78.233) + 0.4 * math.sin(hid * 4.1 + k * 2.7)

    # under a hat: the hair stays inside its crown (head-local z / hr where the hat starts, and how
    # far out the hair may stand there); a headband or visor: the hair stays inside the band
    # (hat_s allows for the thickness of a lock over its centre line)
    hat_z, hat_s = (0.12, 0.98) if "YellowHat" in acc else (0.22, 0.96) if "Cap" in acc else (0.34, 0.93) if "StrawHat" in acc else (None, None)
    banded = bool(acc & {"Headband", "SunVisor"})
    VT, VS = 0.1, 0.04  # (volume on top of the skull, round the sides: set per style)

    def vol(phi, z):
        return 1.0 + VT * max(0.0, z) ** 1.5 + VS * max(0.0, 1 - abs(z))

    def clamp_s(z, s):
        if hat_z is not None and z > hat_z - 0.14:
            s = min(s, hat_s)
        if banded and 0.18 < z < 0.66:
            s = min(s, 0.975)
        return s

    def surf(phi, z, s, hang=0.0):
        """A point over the head at azimuth phi (0: the face, +90 degrees: the head's +X side, 180:
        the back) and height z (head-local, in hr: 1.1 is the crown), s times out from its centre;
        hang (0..1): hair below the widest part of the head falls straight instead of hugging the
        nape and jaw (and goes on down past the head)."""
        zz = max(-1.0, min(1.0, z / 1.1))
        c = math.sqrt(1 - zz * zz)
        a = on_head((math.sin(phi) * c, -math.cos(phi) * c, zz), s)
        if hang <= 0 or z >= 0:
            return a
        e = on_head((math.sin(phi), -math.cos(phi), 0.0), s)
        k = 1 - 0.06 * min(-z, 1.0)
        b = Vector((head_c.x + (e.x - head_c.x) * k, head_c.y + (e.y - head_c.y) * k, head_c.z + z * hr))
        return b if z < -1.0 else a.lerp(b, hang)

    def hair_w(p):
        d = (head_c.z - p.z) / hr
        return hw_ if d < 1.4 else blend("head", "chest", min(1.0, (d - 1.4) / 1.4))

    def face_out(ids, out):
        """A face, wound to face `out`."""
        a, b, c = (Vector(B.v[i]) for i in ids[:3])
        if (b - a).cross(c - a).dot(out) < 0:
            ids = ids[::-1]
        B.face(ids, B.mat("hair"))

    def spline(ctrl, n):
        """n points along a Catmull-Rom curve through ctrl."""
        P = [ctrl[0] * 2 - ctrl[1]] + list(ctrl) + [ctrl[-1] * 2 - ctrl[-2]]
        out = []
        for k in range(n):
            u = k / (n - 1) * (len(ctrl) - 1)
            i = min(int(u), len(ctrl) - 2)
            t = u - i
            p0, p1, p2, p3 = P[i], P[i + 1], P[i + 2], P[i + 3]
            out.append(0.5 * ((2 * p1) + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t * t + (-p0 + 3 * p1 - 3 * p2 + p3) * t ** 3))
        return out

    def lock(ctrl, W, T, tipw=0.0, bend=0.3, n=7, axis=None, full=0.3):
        """A clump along ctrl (world points, root first): a ribbon W wide and T thick (in hr) whose
        edges curl in toward the head (axis(p): the point it curls round), full width a third of the
        way along, then tapering to a point (tipw=0) or to tipw of its width, cut off."""
        pts = spline(ctrl, n)
        ax = axis or (lambda p: head_c + Vector((0, 0, min(0.0, p.z - head_c.z))))
        rings = []
        last = n - 1 if tipw <= 0 else n
        for i in range(last):
            p = pts[i]
            t = (pts[min(i + 1, n - 1)] - pts[max(i - 1, 0)]).normalized()
            nn = p - ax(p)
            nn -= t * nn.dot(t)
            if nn.length < 1e-6:
                nn = Vector((0, 1, 0))
            nn.normalize()
            sd = t.cross(nn).normalized()
            f = i / (n - 1)
            taper = 1 - (1 - tipw) * max(0.0, (f - full) / (1 - full)) ** 0.9
            grow = min(1.0, 0.3 + 2.2 * f)  # (thin at the root, where it comes out of the shell)
            w = W * hr * 0.5 * grow * taper
            th = T * hr * 0.5 * (0.45 + 0.55 * taper) * grow
            ring = []
            for k in range(6):
                a = 2 * math.pi * k / 6
                ca = math.cos(a)
                q = p + sd * (w * ca) + nn * (th * math.sin(a) - bend * w * (ca * ca - 0.5))
                ring.append(B.vert(q, hair_w(q)))
            rings.append((ring, p))
        for (r0, p0), (r1, p1) in zip(rings, rings[1:]):
            for k in range(6):
                k2 = (k + 1) % 6
                ids = [r0[k], r0[k2], r1[k2], r1[k]]
                cen = sum((Vector(B.v[i]) for i in ids), Vector()) / 4
                face_out(ids, cen - (p0 + p1) / 2)
        r0, p0 = rings[0]
        t0 = pts[0] - pts[1]
        for k in range(1, 5):
            face_out([r0[0], r0[k], r0[k + 1]], t0)
        if tipw <= 0:
            tip = B.vert(pts[-1], hair_w(pts[-1]))
            r1, p1 = rings[-1]
            for k in range(6):
                face_out([r1[k], r1[(k + 1) % 6], tip], pts[-1] - p1)
        else:
            r1, p1 = rings[-1]
            t1 = pts[-1] - pts[-2]
            for k in range(1, 5):
                face_out([r1[0], r1[k], r1[k + 1]], t1)

    def shell(edge, top=None, nphi=24, nt=6, hang=0.0, lumpy=0.0, scale=1.0):
        """The base of the hair, from the crown (or top(phi): a bald patch above it, or a hat's
        band) down to edge(phi): its hairline (head-local z), where it tucks in to the scalp."""
        if top is None and hat_z is not None:
            top = lambda phi: hat_z + 0.1
        cols = []
        for j in range(nphi):
            phi = 2 * math.pi * j / nphi
            th0 = math.acos(max(-1.0, min(1.0, top(phi) / 1.1))) if top else 0.0
            th1 = math.acos(max(-1.0, min(1.0, edge(phi) / 1.1)))
            col = []
            for i in range(nt + 1):
                if top is None and i == 0:
                    continue
                th = th0 + (th1 - th0) * i / nt
                z = 1.1 * math.cos(th)
                s = vol(phi, z) * scale * (1 + lumpy * min(1.0, th / 0.6) * math.sin(5 * phi + 1.3) * math.sin(9 * th))
                if i == nt or (top and i == 0):
                    s = 0.955
                p = surf(phi, z, clamp_s(z, s), hang)
                col.append(B.vert(p, hair_w(p)))
            cols.append(col)
        if top is None:
            apex = B.vert(surf(0, 1.1, vol(0, 1.1) * scale), hw_)
        for j in range(nphi):
            c0, c1 = cols[j], cols[(j + 1) % nphi]
            if top is None:
                face_out([apex, c0[0], c1[0]], Vector((0, 0, 1)))
            for i in range(len(c0) - 1):
                ids = [c0[i], c1[i], c1[i + 1], c0[i + 1]]
                cen = sum((Vector(B.v[x]) for x in ids), Vector()) / 4
                face_out(ids, cen - head_c - Vector((0, 0, min(0.0, cen.z - head_c.z))))

    def fan(n, phi0, phi1, ztip, root=0.88, W=0.42, T=0.13, lift=0.05, tipw=0.0, flick=0.0, curl=0.0,
            hang=0.0, jl=0.06, k0=0, part=None):
        """n locks with their tips spread from azimuth phi0 to phi1 (degrees) at height ztip (or
        ztip(phi)), each running down over the shell from the crown (root: its height), or from
        a part (part: its azimuth), and lifting out (flick) or curling under (curl) at the end."""
        for i in range(n):
            ph = deg(phi0 + (phi1 - phi0) * (i + 0.5) / n)
            zt = (ztip(ph) if callable(ztip) else ztip) + jl * jit(k0 + i)
            ph0 = deg(part) if part is not None else ph
            z0 = (root + 0.07 * jit(k0 + i + 90)) if hat_z is None else min(root, hat_z + 0.06)
            ctrl = []
            for k in range(5):
                f = k / 4
                phk = ph0 + (ph - ph0) * min(1.0, f * 1.6)
                zk = z0 + (zt - z0) * f
                s = vol(phk, min(zk, 1.1)) + lift * (1 - 0.4 * f) - (0.07 if k == 0 else 0.0)
                e = max(0.0, (f - 0.55) / 0.45)
                s += flick * e * 0.35 - curl * e * 0.22
                ctrl.append(surf(phk, zk, clamp_s(zk, s), hang))
            lock(ctrl, W * (1 + 0.12 * jit(k0 + i + 50)), T, tipw)

    def drape(n, phi0, phi1, zt, W=0.5, T=0.14, lift=0.05, back=True, k0=0, jl=0.12, tipw=0.0):
        """n long locks from the crown down over the head and on down past it (long hair): those
        at the sides go round behind the shoulders as they fall (back=True)."""
        for i in range(n):
            ph = deg(phi0 + (phi1 - phi0) * (i + 0.5) / n)
            ztip = zt + jl * jit(k0 + i)
            ctrl = []
            for zk in (0.85 + 0.08 * jit(k0 + i + 90), 0.55, 0.0, -0.6, -1.2, (ztip - 1.2) * 0.5 - 1.2, ztip):
                if zk < ztip:
                    continue
                phk = ph
                if back and zk < -0.9:
                    sg = 1 if ph < math.pi else -1
                    want = math.pi - sg * deg(32 + 40 * abs(math.sin(ph)) * 0.5)
                    u = min(1.0, (-0.9 - zk) / 1.2)
                    phk = ph + (want - ph) * u if abs(ph - math.pi) > abs(want - math.pi) else ph
                s = vol(phk, max(zk, -0.5)) + lift + 0.04 * max(0.0, -zk - 1.0) - (0.07 if not ctrl else 0.0)
                ctrl.append(surf(phk, zk, clamp_s(zk, s), 1.0))
            lock(ctrl, W * (1 + 0.1 * jit(k0 + i + 50)), T, tipw, n=9, full=0.6)

    def curtain(phi0, phi1, z0, z1, nphi=12, nz=5):
        """The mass of long hair hanging behind the locks (so they don't part like a comb): a
        sheet with a front and a back, from z0 down to z1 (head-local), the sides going round
        behind the shoulders like the locks over it."""
        g = []
        for i in range(nz + 1):
            zk = z0 + (z1 - z0) * i / nz
            row = []
            for j in range(nphi + 1):
                ph = deg(phi0 + (phi1 - phi0) * j / nphi)
                if zk < -0.9:
                    sg = 1 if ph < math.pi else -1
                    want = math.pi - sg * deg(32 + 20 * abs(math.sin(ph)))
                    u = min(1.0, (-0.9 - zk) / 1.2)
                    ph = ph + (want - ph) * u if abs(ph - math.pi) > abs(want - math.pi) else ph
                s = vol(ph, max(zk, -0.5)) + 0.02 + 0.04 * max(0.0, -zk - 1.0)
                row.append([surf(ph, zk, clamp_s(zk, s - k * 0.05), 1.0) for k in (0, 1)])
            g.append(row)
        ids = [[[B.vert(p, hair_w(p)) for p in c] for c in row] for row in g]
        for i in range(nz):
            for j in range(nphi):
                for k in (0, 1):
                    q = [ids[i][j][k], ids[i][j + 1][k], ids[i + 1][j + 1][k], ids[i + 1][j][k]]
                    cen = sum((Vector(B.v[x]) for x in q), Vector()) / 4
                    out = cen - head_c - Vector((0, 0, cen.z - head_c.z))
                    face_out(q, out if k == 0 else -out)

    def edge_of(*pts):
        return lambda phi: interp(abs(math.degrees((phi + math.pi) % (2 * math.pi) - math.pi)), list(pts))

    # the usual hairline: a curve over the forehead, down in front of the ears, to the nape
    hairline = edge_of((0, 0.56), (30, 0.52), (55, 0.3), (75, 0.0), (95, -0.2), (140, -0.42), (180, -0.48))

    def bangs(n=6, ztip=0.3, spread=48, W=0.52, T=0.13, **kw):
        fan(n, -spread, spread, lambda ph: ztip - 0.06 * math.cos(ph * 3.5) + 0.12 * (abs(ph) / deg(spread)) ** 2, W=W, T=T, k0=0, **kw)

    def tie(c, nrm):
        B.ring(c, nrm.to_track_quat("Z", "Y").to_matrix(), 0.13 * hr, 0.05 * hr, hw_, "tie_band", seg=12, tube=5)

    def tail(base, out, length, n, W=0.36, T=0.2, spread=0.25, k0=20):
        """Hair gathered at base, springing out along `out` then falling, split into n pointed locks
        of a few lengths that fan apart a little at the ends."""
        side = out.cross(Vector((0, 0, 1)))
        side = side.normalized() if side.length > 1e-4 else Vector((1, 0, 0))
        for i in range(n):
            u = (i - (n - 1) / 2) / max(1, (n - 1) / 2)
            L = length * (1 - 0.18 * abs(u) + 0.08 * jit(k0 + i))
            sp = side * (u * spread * hr)
            ctrl = [base - out * 0.05 * hr, base + out * 0.32 * hr + sp * 0.3 + Vector((0, 0, 0.08 * hr)),
                    base + out * 0.5 * hr + sp * 0.7 - Vector((0, 0, 0.35 * L * hr)),
                    base + out * (0.42 + 0.1 * abs(u)) * hr + sp * 1.2 - Vector((0, 0, 0.75 * L * hr)),
                    base + out * (0.3 + 0.18 * abs(u)) * hr + sp * 1.5 - Vector((0, 0, L * hr))]
            lock(ctrl, W, T, n=8, bend=0.15, axis=lambda p, b=base: Vector((b.x, b.y - 0.3 * hr, p.z)))

    def back_short(n=5, zt=-0.45, W=0.68, **kw):
        fan(n, 112, 248, zt, W=W, k0=30, **kw)

    def sides(per, zt, phis=(62, 100), W=0.52, k0=10, **kw):
        for sx in (-1, 1):
            fan(per, sx * phis[0], sx * phis[1], zt, W=W, k0=k0 + (0 if sx > 0 else 7), **kw)

    if st == "Buzz":
        # (the hairline is all there is: an M over the forehead, square at the temples)
        VT, VS = 0.03, 0.0
        shell(edge_of((0, 0.6), (22, 0.54), (40, 0.6), (62, 0.3), (80, 0.0), (95, -0.12), (140, -0.4), (180, -0.45)), scale=0.975)
        if hat_z is None:
            # (a fine, serrated edge along the front: the short hair's tips)
            buzz_line = edge_of((0, 0.6), (22, 0.54), (40, 0.6), (62, 0.3))
            for i in range(11):
                ph = deg(-58 + 116 * (i + 0.5) / 11)
                z0 = buzz_line(ph)
                lock([surf(ph, z0 + 0.12, 0.965), surf(ph, z0 + 0.04, 0.978), surf(ph * 1.02, z0 - 0.035, 0.968)], 0.17, 0.02, n=4, bend=0.1)
    elif st == "Short":
        VT, VS = 0.13, 0.05
        shell(hairline)
        bangs(6, 0.32)
        sides(2, -0.12)
        back_short(5)
    elif st == "Spiky":
        VT, VS = 0.16, 0.06
        shell(hairline)
        bangs(6, 0.26, W=0.5, flick=0.45, jl=0.1)
        sides(2, -0.1, flick=0.9, jl=0.1)
        back_short(5, -0.4, flick=1.0, jl=0.1)
        if not hatted:
            # big clumps standing up off the crown and the back, leaning back and down
            # (lying back along the head, lifting off it at the ends: from the front they only
            # roughen the outline of the crown)
            for k in range(4):
                ph = deg(148 + 64 * k / 3)
                zr = 0.8 + 0.12 * math.cos(k * 1.9)
                root = surf(ph, zr, vol(ph, zr) - 0.05)
                out = (root - head_c).normalized()
                back = Vector((math.sin(ph) * 0.4, 1.0, -0.15)).normalized()
                L = 0.55 + 0.1 * jit(60 + k)
                lock([root - out * 0.06 * hr, root + (back * 0.25 + out * 0.08) * hr, root + (back * L + out * 0.22) * hr], 0.7, 0.18, n=6, bend=0.35)
    elif st == "SidePart":
        # parted on the +X side: the front swept across the forehead to the other side
        VT, VS = 0.12, 0.05
        shell(hairline)
        fan(6, -70, 40, lambda ph: 0.42 - 0.2 * max(0.0, -ph) / deg(70), root=0.92, part=48, W=0.46, k0=0, jl=0.04)
        fan(2, 58, 85, 0.05, root=0.92, part=48, W=0.4, k0=6)
        sides(2, -0.15, phis=(66, 102))
        back_short(5)
    elif st == "Bowl":
        # cut blunt all round at the brows: a helmet of locks with square ends
        VT, VS = 0.12, 0.1
        shell(edge_of((0, 0.3), (50, 0.24), (80, 0.0), (120, -0.18), (180, -0.25)))
        fan(9, -68, 68, 0.2, W=0.44, tipw=0.55, jl=0.015, curl=0.3)
        fan(11, 68, 292, lambda ph: -0.25 * max(0.0, 1 - abs(math.degrees(ph) - 180) / 110), W=0.54, tipw=0.5, jl=0.015, curl=0.4, k0=20)
    elif st in ("Long", "Shoulder"):
        VT, VS = 0.1, 0.06
        long_ = st == "Long"
        shell(hairline)
        bangs(6, 0.18)
        sides(2, -1.45 if long_ else -1.2, phis=(60, 82), W=0.5, T=0.14, hang=1.0, jl=0.1)
        zt = -3.3 if long_ else -1.95
        drape(9, 95, 265, zt, W=0.72 if long_ else 0.66, T=0.17, jl=0.22)
        curtain(100, 260, -0.3, zt + 0.35)
    elif st == "Bob":
        # chin length, full at the sides, the ends curling under
        VT, VS = 0.13, 0.15
        bob_edge = edge_of((0, 0.5), (32, 0.42), (52, -0.2), (68, -0.62), (180, -0.66))
        shell(bob_edge, hang=1.0)
        bangs(7, 0.2, spread=52, W=0.46, tipw=0.3, jl=0.03)
        fan(12, 56, 304, -0.74, W=0.58, T=0.14, tipw=0.25, curl=0.7, hang=1.0, jl=0.04, k0=20)
    elif st == "Ponytail":
        # pulled back close to the head into a high tail at the back
        VT, VS = 0.05, 0.0
        shell(edge_of((0, 0.56), (30, 0.52), (55, 0.3), (75, 0.0), (95, -0.2), (140, -0.45), (180, -0.52)))
        tz = 0.38 if hat_z is None else min(0.38, hat_z - 0.14)  # (tied below a hat's band)
        tb = surf(deg(180), tz, 1.06)
        for i in range(6):
            ph = deg((40 + 55 * (i % 3) / 2) * (1 if i < 3 else -1))
            z0 = hairline(ph) + 0.04
            ctrl = [surf(ph, z0 + 0.12, 0.96), surf(ph * 0.8, 0.5 * (z0 + 1.1), vol(ph, 0.8) + 0.03), surf(ph * 0.3 + math.copysign(deg(150), ph), 0.62, 1.04), tb]
            lock(ctrl, 0.66, 0.06, tipw=0.5, bend=0.15)
        bangs(6, 0.3, spread=42, W=0.48)
        sides(1, -0.55, phis=(58, 62), W=0.3, T=0.08, hang=0.5)
        if not ("StrawHat" in acc):
            tie(tb, (tb - head_c).normalized())
        out = (Vector((0.15, 1.0, 0.25))).normalized()
        tail(tb, out, interp(age, [(6, 1.3), (14, 1.9), (25, 2.1)]), 5, W=0.44, T=0.24)
    elif st == "TwinTails":
        VT, VS = 0.06, 0.02
        shell(hairline)
        bangs(7, 0.26)
        for sx in (-1, 1):
            tb = surf(sx * deg(105), 0.42 if hat_z is None else min(0.42, hat_z - 0.12), 1.06)
            for i in range(4):
                ph = sx * deg(15 + 150 * (i + 0.5) / 4)
                ctrl = [surf(sx * deg(4), 1.0, vol(0, 1.0) - 0.02), surf(ph, 0.8, vol(ph, 0.8) + 0.04), tb]
                lock(ctrl, 0.42, 0.06, tipw=0.5)
            tie(tb, (tb - head_c).normalized())
            out = Vector((sx * 1.0, 0.35, 0.3)).normalized()
            tail(tb, out, interp(age, [(6, 1.25), (12, 1.7)]), 4, W=0.42, T=0.24, spread=0.2, k0=20 + 5 * (sx > 0))
    elif st == "Braid":
        VT, VS = 0.06, 0.02
        shell(edge_of((0, 0.56), (30, 0.52), (55, 0.3), (75, 0.0), (95, -0.2), (140, -0.45), (180, -0.55)))
        bangs(7, 0.24)
        nb = surf(deg(180), -0.5, 1.0)
        for i in range(6):
            ph = deg(100 + 160 * (i + 0.5) / 6)
            lock([surf(ph, 0.9, vol(ph, 0.9) + 0.03), surf(ph, 0.2, vol(ph, 0.2) + 0.04), nb], 0.42, 0.06, tipw=0.5)
        # plaited: segments leaning alternately left and right, getting smaller, a tuft at the end
        n = 9
        for k in range(n):
            u = k / (n - 1)
            c = nb + Vector((0.09 * hr * (1 if k % 2 else -1) * (1 - 0.3 * u), 0.12 * hr + 0.05 * hr * u, -0.26 * hr * k - 0.15 * hr))
            sz = 1.25 - 0.4 * u
            rot = Euler((0, deg(28 if k % 2 else -28), 0)).to_matrix()
            B.ellipsoid(c, (0.15 * hr * sz, 0.13 * hr * sz, 0.24 * hr * sz), hair_w(c), "hair", rot=rot, seg=8, rings=5)
        end = nb + Vector((0, 0.17 * hr, -0.26 * hr * n - 0.05 * hr))
        tie(end + Vector((0, 0, 0.12 * hr)), Vector((0, 0, 1)))
        for i in range(3):
            lock([end + Vector((0, 0, 0.12 * hr)), end + Vector(((i - 1) * 0.06 * hr, 0, -0.2 * hr)), end + Vector(((i - 1) * 0.12 * hr, 0.02 * hr, -0.45 * hr))], 0.2, 0.1, n=5)
    elif st == "Bun":
        # parted in the middle and drawn back over the ears into a round bun at the back of the crown
        VT, VS = 0.06, 0.07
        shell(edge_of((0, 0.6), (35, 0.52), (60, 0.25), (85, -0.05), (140, -0.4), (180, -0.45)))
        bz = 0.6
        bc = surf(deg(180), bz, 1.22)
        for sx in (-1, 1):
            for i in range(4):
                ph = sx * deg(8 + 95 * (i + 0.5) / 4)
                z0 = hairline(ph) + 0.03
                ctrl = [surf(sx * deg(3), 1.0, vol(0, 1.0) - 0.02) if i < 2 else surf(ph, z0, 1.0), surf(ph, 0.35 if i < 2 else 0.1, vol(ph, 0.3) + 0.05),
                        surf(sx * deg(140), 0.35, vol(deg(140), 0.35) + 0.04), surf(sx * deg(172), bz - 0.05, 1.05)]
                lock(ctrl, 0.46, 0.07, tipw=0.4)
        if hat_z is None or hat_z > 0.5:
            B.ellipsoid(bc, (0.36 * hr, 0.3 * hr, 0.33 * hr), hw_, "hair", seg=14, rings=8)
            # (wound round: a lock wrapped about the bun)
            ring = [bc + Euler((deg(-20), 0, 0)).to_matrix() @ Vector((0.34 * hr * math.cos(a), 0.04 * hr + 0.12 * hr * math.sin(a * 0.5), 0.3 * hr * math.sin(a)))
                    for a in [i * 0.9 for i in range(8)]]
            lock(ring, 0.22, 0.12, tipw=0.3, n=12, axis=lambda p: bc)
    elif st == "ShortPerm":
        # a puffy, lumpy round of tight curls, rolled up and back all round the edge
        VT, VS = 0.2, 0.17
        perm_edge = edge_of((0, 0.58), (40, 0.48), (70, 0.05), (100, -0.25), (180, -0.4))
        shell(perm_edge, lumpy=0.07, nphi=28)
        # (round the edge: soft puffs, half sunk in the shell, overlapping: a scalloped outline)
        nc = 13
        for i in range(nc):
            ph = 2 * math.pi * (i + 0.5) / nc + 0.08 * jit(80 + i)
            for row, dz in enumerate((0.04, 0.3)):
                z0 = perm_edge(ph) + dz
                if hat_z is not None and z0 > hat_z - 0.12:
                    continue
                front = abs(math.degrees((ph + math.pi) % (2 * math.pi) - math.pi)) < 48
                if front and row == 1:
                    continue
                sz = (0.8 if front else 1.0) * (0.85 if row else 1.0) * (1 + 0.08 * jit(90 + i))
                c = surf(ph + (0.5 * 2 * math.pi / nc if row else 0.0), z0, vol(ph, z0) - 0.05)
                out = (c - head_c - Vector((0, 0, min(0.0, c.z - head_c.z)))).normalized()
                rot = out.to_track_quat("Z", "Y").to_matrix()
                B.ellipsoid(c, (0.29 * hr * sz, 0.24 * hr * sz, 0.15 * hr * sz), hair_w(c), "hair", rot=rot, seg=8, rings=5)
    elif st == "Receding":
        # a horseshoe: the back and the sides above the ears, swept back; the top bare
        VT, VS = 0.04, 0.08
        rec_top = edge_of((0, 0.2), (48, 0.2), (68, 0.36), (110, 0.5), (150, 0.78), (180, 0.86))
        rec_edge = edge_of((0, 0.2), (48, 0.2), (68, 0.02), (85, -0.08), (100, -0.2), (140, -0.42), (180, -0.46))
        shell(rec_edge, top=rec_top if hat_z is None else (lambda phi: max(rec_top(phi), hat_z + 0.1)))
        for sx in (-1, 1):
            for i in range(3):
                ph = sx * deg(72 + 24 * i)
                z0 = min(0.4 + 0.08 * i, hat_z if hat_z is not None else 9)
                lock([surf(ph, z0, 0.97), surf(ph + sx * deg(25), z0 - 0.16, vol(ph, z0) + 0.06), surf(ph + sx * deg(48), z0 - 0.34 + 0.04 * jit(70 + i), vol(ph, z0) + 0.03)],
                     0.48, 0.11, n=5)
    # (Bald: no hair)

    # hats and the rest
    if "YellowHat" in acc:
        B.ellipsoid(head_c + Vector((0, 0, 0.12 * hr)), (hr * 1.12, hr * 1.12, hr * 1.12), hw_, "hat", seg=24, rings=14, keep=lambda lp: lp.z > 0.0)
        B.ellipsoid(head_c + Vector((0, 0, 0.12 * hr)), (hr * 1.45, hr * 1.45, hr * 0.06), hw_, "hat", seg=24, rings=6)
    if "Cap" in acc:
        B.ellipsoid(head_c, (hr * 1.08, hr * 1.08, hr * 1.08), hw_, "hat", seg=24, rings=14, deform=head_shape, keep=lambda lp: lp.z > 0.2)
        B.ellipsoid(head_c + Vector((0, -hr * 0.95, hr * 0.38)), (hr * 0.62, hr * 0.55, hr * 0.05), hw_, "hat", seg=16, rings=6, keep=lambda lp: lp.y < 0.1)
    if "StrawHat" in acc:
        B.ellipsoid(head_c + Vector((0, 0, hr * 0.42)), (hr * 1.02, hr * 1.02, hr * 0.62), hw_, "straw", seg=20, rings=10, keep=lambda lp: lp.z > -0.1)
        B.ellipsoid(head_c + Vector((0, 0, hr * 0.38)), (hr * 2.1, hr * 2.1, hr * 0.05), hw_, "straw", seg=28, rings=6)
    if "SunVisor" in acc:
        B.ring(head_c + Vector((0, 0, hr * 0.45)), Euler((math.radians(-6), 0, 0)).to_matrix(), hr * 0.98, 0.006 * HH, hw_, "hat", seg=24, tube=5)
        B.ellipsoid(head_c + Vector((0, -hr * 0.9, hr * 0.42)), (hr * 0.95, hr * 0.7, hr * 0.04), hw_, "hat", seg=16, rings=6, keep=lambda lp: lp.y < 0.0)
    if "Headband" in acc:
        B.ring(head_c + Vector((0, 0.004 * HH, hr * 0.42)), Euler((math.radians(-12), 0, 0)).to_matrix(), hr * 0.97, 0.007 * HH, hw_, "headband")
    if "Randoseru" in acc:
        z = 0.68 * H
        bk = torso_yc(z) + torso_at(z)[1]

        def boxy(lp):
            return Vector([math.copysign(abs(v) ** 0.35, v) for v in lp])
        B.ellipsoid(Vector((0, bk + 0.045 * H, z)), (0.08 * H, 0.042 * H, 0.095 * H), {"chest": 1.0}, "randoseru", seg=16, rings=10, deform=boxy)
        # its shoulder straps: from the top of the bag over the shoulders and down under the arms
        for sx in (-1, 1):
            zt = 0.80 * H
            tube([Vector((sx * 0.04 * H, bk + 0.012 * H, 0.765 * H)), Vector((sx * 0.045 * H, torso_yc(zt) + torso_at(zt)[1] * 0.7, zt + 0.006 * H)),
                  Vector((sx * 0.05 * H, torso_yc(zt) - torso_at(zt)[1] * 0.75, zt)), on_front(sx * 0.056 * H, 0.74 * H, 0.006),
                  on_front(sx * 0.072 * H, 0.67 * H, 0.006), Vector((sx * torso_at(0.62 * H)[0] * 1.02, torso_yc(0.62 * H), 0.62 * H)),
                  Vector((sx * 0.05 * H, bk + 0.01 * H, 0.60 * H))], 0.0075 * H, {"chest": 1.0}, "randoseru")
    if spec.get("glasses"):
        for sx in (-1, 1):
            c = head_c + Vector((sx * eye_x, -head_r * 1.0, eye_z))
            B.ring(c, Euler((math.radians(90), 0, 0)).to_matrix(), head_r * (0.23 + 0.03 * kid), 0.0016 * HH, hw_, "glasses", seg=20, tube=6)
        B.strand([head_c + Vector((-eye_x + head_r * (0.23 + 0.03 * kid), -head_r * 1.02, eye_z + head_r * 0.01)), head_c + Vector((0, -head_r * 1.04, eye_z + head_r * 0.03)),
                  head_c + Vector((eye_x - head_r * (0.23 + 0.03 * kid), -head_r * 1.02, eye_z + head_r * 0.01))], 0.0016 * HH, hw_, "glasses")

    # --- into Blender: the mesh, its materials, its skeleton
    me = bpy.data.meshes.new(name)
    me.from_pydata(B.v, [], B.f)
    me.update()
    if getattr(B, "sharp", None):  # (edges marked sharp: pleat folds)
        es = {tuple(sorted(e.vertices)): e.index for e in me.edges}
        sharp = me.attributes.get("sharp_edge") or me.attributes.new("sharp_edge", "BOOLEAN", "EDGE")
        for a, b in B.sharp:
            i = es.get((min(a, b), max(a, b)))
            if i is not None:
                sharp.data[i].value = True
    obj = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(obj)
    for p in me.polygons:
        p.use_smooth = True
        p.material_index = B.fm[p.index]
    uvl = me.uv_layers.new(name="UVMap")
    for loop in me.loops:
        uvl.data[loop.index].uv = B.uv[loop.vertex_index]

    face_spec = dict(spec)
    face_spec["hair"] = {"color": hs["color"]}
    face_img = paint_face(face_spec, os.path.join(out_dir, f"{name}_face.png"))
    rand_col = [(30, 30, 34), (40, 44, 72), (170, 40, 50), (220, 140, 160), (150, 130, 190)]
    colors = {"skin": spec["skin"], "shirt": spec["shirt"], "pants": spec["pants"], "shoes": spec["shoes"], "hair": hs["color"],
              "skirt": (skirt or {}).get("color", (80, 80, 80)), "sole": sole_col, "socks": (socks or [(236, 236, 236)])[0], "armcover": spec.get("armcovers") or (220, 220, 210),
              "collar": extras.get("collar_stand") or extras.get("sailor_collar") or (30, 30, 34), "buttons": extras.get("buttons", (200, 170, 70)),
              "scarf": extras.get("scarf", (180, 40, 50)), "shirtv": extras.get("shirt_v", (240, 240, 236)), "tie": extras.get("tie", (150, 40, 50)),
              "vest": extras.get("vest", (240, 240, 240)), "stripes": extras.get("stripes", (240, 240, 240)), "wrap": extras.get("wrap", (40, 44, 60)),
              "apron": extras.get("apron", (40, 40, 80)), "hat": (240, 210, 40) if "YellowHat" in acc else ((60, 70, 90) if "Cap" in acc else (236, 236, 230)),
              "straw": (214, 190, 130), "headband": (240, 240, 235), "randoseru": rand_col[(spec.get("id", 0) * 7 + (2 if female else 0)) % len(rand_col)],
              "glasses": spec.get("glasses") or (40, 40, 40), "tie_band": (40, 40, 44),
              "belt": extras.get("belt", (40, 34, 30)), "zip": extras.get("zip", (60, 60, 60)), "collarflat": extras.get("collar_flat", spec["shirt"])}
    rough = {"skin": 0.55, "face": 0.55, "hair": 0.45, "shoes": 0.4, "sole": 0.8, "glasses": 0.3, "randoseru": 0.3, "buttons": 0.25}
    for mname in B.mats:
        m = bpy.data.materials.new(f"{name}_{mname}")
        m.use_nodes = True
        bsdf = m.node_tree.nodes["Principled BSDF"]
        bsdf.inputs["Roughness"].default_value = rough.get(mname, 0.75)
        if mname == "face":
            t = m.node_tree.nodes.new("ShaderNodeTexImage")
            t.image = face_img
            m.node_tree.links.new(t.outputs["Color"], bsdf.inputs["Base Color"])
        else:
            bsdf.inputs["Base Color"].default_value = lin(colors.get(mname, (128, 128, 128)))
        if mname in ("glasses", "buttons"):
            bsdf.inputs["Metallic"].default_value = 0.8
        me.materials.append(m)

    arm_data = bpy.data.armatures.new(name + "_skel")
    arm = bpy.data.objects.new(name + "_skel", arm_data)
    bpy.context.scene.collection.objects.link(arm)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="EDIT")
    for jn, (h, t) in J.items():
        eb = arm_data.edit_bones.new(jn)
        eb.head, eb.tail = h, t
        eb.roll = 0.0
    for jn, p in parents.items():
        arm_data.edit_bones[jn].parent = arm_data.edit_bones[p]
    bpy.ops.object.mode_set(mode="OBJECT")
    groups = {jn: obj.vertex_groups.new(name=jn) for jn in J}
    for vi, w in enumerate(B.w):
        tot = sum(w.values())
        for jn, x in w.items():
            groups[jn].add([vi], x / tot, "REPLACE")
    obj.parent = arm
    mod = obj.modifiers.new("skin", "ARMATURE")
    mod.object = arm

    info = {"id": name, "height_m": Hreal, "clips": animate(arm, spec, H)}
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    obj.select_set(True)
    bpy.context.view_layer.objects.active = arm
    out = os.path.join(out_dir, f"{name}.gltf")
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLTF_SEPARATE", use_selection=True, export_yup=True, export_skins=True,
                              export_animations=True, export_animation_mode="NLA_TRACKS", export_morph=False,
                              export_image_format="AUTO", export_texture_dir="tex")
    json.dump(info, open(os.path.join(out_dir, f"{name}.json"), "w"), indent=1)
    return out


# --- legs and feet: the leg's shape, shoes, and the footwork the clips use
#
# Anime legs (see docs/people_legs_notes.md): long and slender, the thigh a touch longer than the
# shin, the knee small, the calf a soft swell at the back just under the knee, a thin ankle and
# small feet (smaller for women); children's legs rounder with little taper; old people's legs
# thinner, the knees a little bent. Trousers hang straight from the thigh and break on the shoe.

# (s along the leg: 0 the hip, 0.5 the knee, 1 the ankle; half width, half depth, how far the
# middle sits behind the bone; all x H)
LEG = [(-0.07, .040, .042, .000), (-0.03, .044, .045, .000), (0.0, .047, .047, .000), (0.06, .049, .049, .001), (0.12, .050, .050, .002), (0.30, .045, .045, .002),
       (0.44, .037, .039, .000), (0.52, .034, .036, .000), (0.60, .036, .041, .007), (0.70, .034, .039, .006),
       (0.84, .025, .027, .002), (0.95, .020, .022, .000), (1.0, .020, .022, .000)]


def leg_profile(s, female, age, old):
    """Half width, half depth, back offset (x H) of the bare leg at s."""
    def at(k):
        for a, b in zip(LEG, LEG[1:]):
            if a[0] <= s <= b[0]:
                t = (s - a[0]) / (b[0] - a[0])
                return a[k] + (b[k] - a[k]) * t
        return LEG[0][k] if s < 0 else LEG[-1][k]
    rx, ry, off = at(1), at(2), at(3)
    smooth = 0.06 * (1 - min(1.0, max(0.0, s - 0.3) / 0.7)) + 0.024  # (an untapered leg, for small children)
    if female:
        if s < 0.35:
            rx, ry = rx * 1.04, ry * 1.03
        if s > 0.8:
            rx, ry = rx * 0.93, ry * 0.93
    if age < 10:
        k = 0.55 if age < 6 else 0.3
        bulge = 1 - k
        rx, ry, off = rx * (1 - k) + smooth * k * 0.9, ry * (1 - k) + smooth * k * 0.95, off * bulge
    if old:
        if 0.1 < s < 0.9:
            rx, ry = rx * 0.94, ry * 0.94
        off *= 0.5
    return rx, ry, off


def leg_crouch(spec):
    """How far (x H) the hips sit below straight legs standing: old people's knees stay bent."""
    return 0.012 if spec.get("stoop") else (0.007 if spec.get("old") else 0.001)


def shoe_dims(spec, H):
    """The shoe's length and half width, and where the heel and the ball sit forward of the ankle."""
    female = spec["build"] == "f"
    L = (0.122 if female else 0.132) * H * (1.1 if spec["age"] < 12 else 1.0) * (1.05 if spec.get("boots") else 1.0)
    return {"len": L, "heel": -0.24 * L, "ball": 0.56 * L, "toe": 0.76 * L, "w": (0.029 if female else 0.032) * H}


def shoe_style(spec):
    if spec.get("boots"):
        return "boots"
    light = sum(spec["shoes"]) / 3 > 165
    if light or spec["age"] < 10 or any(e[0] == "stripes" for e in spec.get("extras", [])):
        return "sneaker"
    return "loafer"


def build_shoe(B, an, H, d, wscale, style, side, weights):
    """A shoe: an upper on a sole, a heel at the back, a rounded toe box, a little toe spring.
    an: the ankle (rest); the shoe's bottom is on the floor (z = 0)."""
    sx = 1 if side == "l" else -1
    L, w = d["len"], d["w"] * wscale
    # (t heel -> toe, half width, top of the upper x H)
    if style == "sneaker":
        st = [(0.0, .52, .036), (0.05, .78, .046), (0.2, .9, .050), (0.36, .9, .049), (0.55, .99, .042), (0.72, 1.0, .034),
              (0.86, .92, .028), (0.95, .7, .024), (1.0, .32, .019)]
        sole = lambda t: 0.014 * H
        sw = 1.07
    elif style == "boots":
        st = [(0.0, .55, .044), (0.05, .82, .052), (0.2, .92, .056), (0.36, .92, .056), (0.55, 1.0, .046), (0.72, 1.0, .036),
              (0.86, .92, .030), (0.95, .7, .025), (1.0, .34, .020)]
        sole = lambda t: 0.011 * H
        sw = 1.04
    else:  # loafers, school shoes, office shoes: slim, a low heel
        st = [(0.0, .48, .034), (0.05, .74, .044), (0.2, .84, .046), (0.36, .84, .045), (0.55, .94, .036), (0.72, .97, .028),
              (0.86, .88, .022), (0.95, .66, .018), (1.0, .3, .014)]
        sole = lambda t: (0.017 if t < 0.27 else 0.008) * H
        sw = 1.03
    spring = lambda t: 0.007 * H * max(0.0, (t - 0.8) / 0.2) ** 2
    y_of = lambda t: an.y - (d["heel"] + t * L)
    x_of = lambda t: an.x + sx * 0.12 * w * t  # (the toes a little out)
    k = 16
    m_up, m_sole = B.mat("shoes"), B.mat("sole")

    def tube(rings, m):
        """Quads between rings of points (all rings k long), each end fanned to its middle;
        faces turned to face out."""
        ids = [[B.vert(p, weights) for p in r] for r in rings]
        cen = [sum((Vector(p) for p in r), Vector()) / len(r) for r in rings]

        def put(f, inside):
            a, b, c = (Vector(B.v[i]) for i in f[:3])
            mid = sum((Vector(B.v[i]) for i in f), Vector()) / len(f)
            if (b - a).cross(c - a).dot(mid - inside) < 0:
                f = f[::-1]
            B.face(f, m)
        for i in range(len(rings) - 1):
            for j in range(k):
                j2 = (j + 1) % k
                put([ids[i][j], ids[i][j2], ids[i + 1][j2], ids[i + 1][j]], (cen[i] + cen[i + 1]) / 2)
        for i, step in ((0, 1), (len(rings) - 1, -1)):
            tip = B.vert(cen[i] + (cen[i] - cen[i + step]).normalized() * 0.002 * H, weights)
            for j in range(k):
                put([ids[i][j], ids[i][(j + 1) % k], tip], cen[i + step])

    def section(t, hw, zb, zt, flat):
        r = []
        for j in range(k):
            ph = 2 * math.pi * j / k
            c, s = math.cos(ph), math.sin(ph)
            x = hw * math.copysign(abs(c) ** 0.7, c)
            e = 0.9 if s > 0 else flat
            z = (zb + zt) / 2 + (zt - zb) / 2 * math.copysign(abs(s) ** e, s)
            r.append(Vector((x_of(t) + x, y_of(t), z)))
        return r
    up, so = [], []
    for t, fw, top in st:
        lift = spring(t)
        th = sole(t)
        so.append(section(t, w * fw * sw, lift, lift + th, 0.3))
        up.append(section(t, w * fw, lift + th * 0.6, max(top * H, lift + th + 0.006 * H), 0.4))
    tube(up, m_up)
    tube(so, m_sole)


class FootWork:
    """The legs in the clips, solved from where the feet are: each leg a two-bone chain in the
    side plane (forward f, up z), knees bending forward, the shoe rolling on its heel as it lands
    and on its ball as it leaves. Walk (anime walk sheets: contact, down, passing, up): a planted
    foot moves back at exactly the walk speed (no sliding); the swing foot lifts clear of the floor;
    the hips sit as high as the planted leg allows, so they dip after contact and rise at passing."""

    def __init__(self, arm, spec, H):
        self.H = H
        bones = arm.data.bones
        self.rest = {}
        for s in ("l", "r"):
            hip, kn, an = bones[f"thigh_{s}"].head_local, bones[f"thigh_{s}"].tail_local, bones[f"calf_{s}"].tail_local
            fz = lambda v: (-v.y, v.z)
            h, k, a = fz(hip), fz(kn), fz(an)
            self.rest[s] = {"hip": h, "x": hip.x, "an": a,
                            "L1": math.dist(h, k), "L2": math.dist(k, a),
                            "a0": math.atan2(k[0] - h[0], h[1] - k[1]), "g0": math.atan2(a[0] - k[0], k[1] - a[1])}
        d = shoe_dims(spec, H)
        self.heel, self.ball, self.toe = d["heel"], d["ball"], d["toe"]
        old, age = spec.get("old", False), spec["age"]
        r = self.rest["l"]
        self.hip_z = r["hip"][1]
        self.reach = r["L1"] + r["L2"]
        legh = self.hip_z
        # (a calm walk: a step about 3/4 of the leg; old people shorter and slower, children quicker)
        self.step = legh * (0.52 if old else (0.8 if age < 12 else (0.68 if spec["build"] == "f" else 0.74)))
        self.T = 1.35 if old else (0.78 if age < 6 else (0.86 if age < 12 else 1.05))
        self.D = 0.66 if old else 0.62  # (time on the ground)
        self.p_hs = -6.0 if old else -15.0  # (toe up as the heel lands)
        self.p_to = 12.0 if old else 24.0  # (heel up as the foot pushes off)
        self.lift = (0.022 if old else 0.035) * H
        self.k_stance = 0.955 if old else 0.988  # (how straight the planted leg gets)
        self.crouch = leg_crouch(spec) * H
        self.speed = 2 * self.step / self.T

    def solve(self, s, hf, hz, af, az, p):
        """Thigh, calf and foot turns (degrees about X: + swings back / turns the toe down) that
        put this side's ankle at (af, az) with the shoe pitched p degrees, the hip at (hf, hz)."""
        r = self.rest[s]
        L1, L2 = r["L1"], r["L2"]
        dx, dz = af - hf, hz - az
        d = min(max(math.hypot(dx, dz), 0.3 * (L1 + L2)), 0.9995 * (L1 + L2))
        phi = math.atan2(dx, dz)
        a1 = math.acos(max(-1.0, min(1.0, (L1 * L1 + d * d - L2 * L2) / (2 * L1 * d))))
        knee = math.acos(max(-1.0, min(1.0, (L1 * L1 + L2 * L2 - d * d) / (2 * L1 * L2))))
        alpha = phi + a1
        beta = math.pi - knee
        rt = -(alpha - r["a0"])
        rc = -((alpha - beta) - r["g0"]) - rt
        rf = math.radians(p) - rt - rc
        return math.degrees(rt), math.degrees(rc), math.degrees(rf)

    def planted(self, s, X, p):
        """The ankle of a shoe on the floor at X (where its ankle would be, flat), pitched p:
        toe up rolls on the heel, heel up rolls on the ball."""
        af0, az0 = self.rest[s]["an"]
        # (a stiff shoe: it rolls off over the tip of its sole, which the toe spring lifts a little)
        piv, pz = (self.heel, 0.0) if p < 0 else (self.toe * 0.96, 0.003 * self.H)
        q = math.radians(p)
        f, z = -piv, az0 - pz  # (the ankle from the pivot)
        return X + piv + f * math.cos(q) + z * math.sin(q), pz - f * math.sin(q) + z * math.cos(q)

    def lowest(self, af, az, p):
        """How high the shoe's lowest point (heel or toe) is."""
        q = math.radians(p)
        out = []
        for off in (self.heel, self.toe):
            f, z = off, -self.rest["l"]["an"][1]
            out.append(az + (-f * math.sin(q) + z * math.cos(q)))
        return min(out)

    def foot(self, s, u):
        """Where one foot is at u (0..1 of its cycle; 0 = its heel lands): ankle (f, z), pitch."""
        S, D = 2 * self.step, self.D
        af0 = self.rest[s]["an"][0]
        reach = S * D / 2

        def stance(q):
            X = af0 + reach - 2 * reach * q
            if q < 0.12:
                p = self.p_hs * (1 - q / 0.12) ** 2
            elif q > 0.6:
                p = self.p_to * ((q - 0.6) / 0.4) ** 1.6
            else:
                p = 0.0
            return self.planted(s, X, p), p
        if u < D:
            (f, z), p = stance(u / D)
            return f, z, p
        w = (u - D) / (1 - D)
        (f0, z0), p0 = stance(1.0)
        (f1, z1), p1 = stance(0.0)
        e = 0.5 - 0.5 * math.cos(math.pi * w)
        f = f0 + (f1 - f0) * e
        z = z0 + (z1 - z0) * e + self.lift * math.sin(math.pi * min(1.0, w * 1.15)) ** 1.3
        p = p0 + (p1 - p0) * min(1.0, w * 1.25) ** 0.8
        need = 0.012 * self.H * math.sin(math.pi * w) - self.lowest(f, z, p)
        if need > 0:
            z += need
        return f, z, p

    def hips(self, feet, yaw=0.0, roll=0.0, k=None):
        """The highest the hips may sit with every planted foot reached by a nearly straight leg."""
        hz = self.hip_z - self.crouch
        for s, (f, z, planted) in feet.items():
            if not planted:
                continue
            r = self.rest[s]
            hf = r["hip"][0] - r["x"] * math.sin(math.radians(yaw))
            dz = r["x"] * math.sin(math.radians(roll))
            reach = (k or self.k_stance) * self.reach
            hz = min(hz, z + math.sqrt(max(reach * reach - (f - hf) ** 2, 0.0)) + dz)
        return hz

    def walk_table(self, frames, yaw_of):
        """Per frame: the hips' height and each side's (ankle f, z, pitch), smoothed round the loop."""
        raw, feet = [], []
        for i in range(frames):
            t = i / frames
            fs = {}
            for s, ph in (("l", -0.25), ("r", 0.25)):  # (the left heel lands as the left arm is furthest back)
                u = (t + ph) % 1.0
                f, z, p = self.foot(s, u)
                fs[s] = (f, z, p, u < self.D)
            feet.append(fs)
            raw.append(self.hips({s: (v[0], v[1], v[3]) for s, v in fs.items()}, yaw_of(t)))
        n = len(raw)
        sm = [sum(raw[(i + j) % n] for j in range(-2, 3)) / 5 for i in range(n)]
        return [min(a, b + 0.0015 * self.H) for a, b in zip(sm, raw)], feet

    def pose(self, hz, fs, yaw=0.0, roll=0.0):
        """Joint turns for the legs (thigh, calf, foot each side), the hips at height hz."""
        out = {}
        for s, (f, z, p) in fs.items():
            r = self.rest[s]
            hf = r["hip"][0] - r["x"] * math.sin(math.radians(yaw))
            h = hz - r["x"] * math.sin(math.radians(roll))
            rt, rc, rf = self.solve(s, hf, h, f, z, p)
            out[f"thigh_{s}"] = (rt, -roll, -yaw)  # (the legs stay upright under the turning hips)
            out[f"calf_{s}"] = (rc, 0, 0)
            out[f"foot_{s}"] = (rf, 0, 0)
        return out


# --- clips: keyed in the skeleton's own frame (each joint's turn about the armature's axes,
# from its rest: X pitches forward(-)/back(+) for limbs hanging down, Z twists)


def animate(arm, spec, H):
    old = spec.get("old", False)
    young = spec["age"] < 12
    for pb in arm.pose.bones:
        pb.rotation_mode = "QUATERNION"
    rest = {pb.name: pb.bone.matrix_local.to_3x3() for pb in arm.pose.bones}
    stoop = 9.0 if spec.get("stoop") else (3.0 if old else 0.0)
    arm.animation_data_create()
    clips = {}

    def key(f, rots, locs=None):
        for jn, (x, y, z) in rots.items():
            R = Euler((math.radians(x), math.radians(y), math.radians(z))).to_matrix()
            M = rest[jn]
            pb = arm.pose.bones[jn]
            pb.rotation_quaternion = (M.inverted() @ R @ M).to_quaternion()
            pb.keyframe_insert("rotation_quaternion", frame=f)
        for jn, d in (locs or {}).items():
            pb = arm.pose.bones[jn]
            pb.location = rest[jn].inverted() @ Vector(d)
            pb.keyframe_insert("location", frame=f)

    def clip(cname, frames, pose):
        act = bpy.data.actions.new(cname)
        act.use_fake_user = True
        arm.animation_data.action = act
        for f in range(frames + 1):
            r, l = pose(f / frames)
            key(f, r, l)
        tr = arm.animation_data.nla_tracks.new()
        tr.name = cname
        tr.strips.new(cname, 0, act)
        arm.animation_data.action = None
        return frames / FPS

    # idle: 4 s of breathing, a slow sway, the head looking about a little (kids fidget more)
    fid = 1.8 if young else 1.0
    fw = FootWork(arm, spec, H)

    def idle(t):
        p = 2 * math.pi * t
        br = math.sin(p * 2)
        sw = math.sin(p)
        r = {
            "pelvis": (0, 0.8 * sw * fid, 0),
            "spine": (stoop * 0.5 + 0.6 * br, -0.5 * sw * fid, 0),
            "chest": (stoop * 0.5 + 0.8 * br, -0.4 * sw, 0),
            "neck": (-stoop * 0.6, 0, 0),
            "head": (-stoop * 0.4 + 2 * math.sin(p + 1), 0, 6 * math.sin(p + 0.5) * fid),
        }
        for s, k in (("l", 1), ("r", -1)):
            # (arms: relaxed, a little out from the body, the elbows soft; Y turns them out)
            r[f"upperarm_{s}"] = (1.5 * br + 2 - (2 if old else 0), -k * (3 + 0.6 * br), 0)
            r[f"lowerarm_{s}"] = (-(16 if old else 11) - 1.5 * br, 0, 0)
            r[f"hand_{s}"] = (-5, k * 2, 0)
        # (legs: the feet stay put on the floor, the knees soft, as the hips sway and breathe)
        hz = fw.hip_z - max(fw.crouch, 0.001 * H) - 0.002 * H * (1 - br) / 2
        r.update(fw.pose(hz, {s: (*fw.rest[s]["an"], 0.0) for s in ("l", "r")}, roll=0.8 * sw * fid))
        return r, {"pelvis": (0, 0, hz - fw.hip_z)}

    clips["idle"] = {"seconds": clip("idle", 4 * FPS, idle)}

    # walk: a loop of two steps (old people slower and shorter, kids quicker)
    T = fw.T
    nwf = round(T * FPS)
    hz_tab, feet_tab = fw.walk_table(nwf, lambda t: 5 * math.sin(2 * math.pi * t))
    C = 10.0 if old else 20.0
    Ca = 9.0 if old else (19.0 if young else 16.0)  # (arm swing)
    Eb = 10.0 if old else 18.0  # (elbow bend as the arm comes forward)

    def walk(t):
        p = 2 * math.pi * t
        sp, cp = math.sin(p), math.cos(p)
        r = {
            "pelvis": (0, 0, 5 * sp),
            "spine": (stoop * 0.5 + 3, 0, -2 * sp),
            "chest": (stoop * 0.5, 0, -5 * sp),
            "neck": (-stoop * 0.6, 0, 2 * sp),
            "head": (-stoop * 0.4 - 2, 0, 3 * sp),
            "upperarm_l": (Ca * sp + 2, -(4 + 2 * max(0.0, -sp)), 0),
            "upperarm_r": (-Ca * sp + 2, 4 + 2 * max(0.0, sp), 0),
            "lowerarm_l": (-12 - Eb * max(0.0, -math.sin(p - 0.4)), 0, 0), "lowerarm_r": (-12 - Eb * max(0.0, math.sin(p - 0.4)), 0, 0),
            "hand_l": (-5 + 5 * math.sin(p - 0.9), 2, 0), "hand_r": (-5 - 5 * math.sin(p - 0.9), -2, 0),
        }
        i = round(t * nwf) % nwf
        r.update(fw.pose(hz_tab[i], {s: v[:3] for s, v in feet_tab[i].items()}, yaw=5 * sp))
        return r, {"pelvis": (0, 0, hz_tab[i] - fw.hip_z)}

    clips["walk"] = {"seconds": clip("walk", nwf, walk), "speed": round(fw.speed, 3)}
    for pb in arm.pose.bones:
        pb.rotation_quaternion = (1, 0, 0, 0)
        pb.location = (0, 0, 0)
    return clips


def gallery(c, n=40):
    """One of each kind of look: outfit, sex, haircut, age band, accessories."""
    seen, out = set(), []
    for p in c["people"]:
        if p["household"] is None or p["away"] is not None or p["look"]["height_cm"] < 80:
            continue
        L = p["look"]
        band = 0 if p["age"] < 6 else 1 if p["age"] < 12 else 2 if p["age"] < 15 else 3 if p["age"] < 40 else 4 if p["age"] < 65 else 5
        k = (L["day"]["outfit"], p["sex"], L["hair"], band)
        if k in seen:
            continue
        seen.add(k)
        out.append(p["id"])
    # spread over the kinds: every outfit first, then the rest
    by_outfit = {}
    for pid in out:
        by_outfit.setdefault(c["people"][pid]["look"]["day"]["outfit"], []).append(pid)
    pick = []
    while len(pick) < n and any(by_outfit.values()):
        for k in list(by_outfit):
            if by_outfit[k] and len(pick) < n:
                pick.append(by_outfit[k].pop(0))
    return pick


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else ["gallery"]
    census = json.load(open(CENSUS, encoding="utf8"))
    if args[0] == "gallery":
        ids = gallery(census, int(args[1]) if len(args) > 1 else 40)
        os.makedirs(OUT, exist_ok=True)
        open(os.path.join(OUT, "gallery_ids.txt"), "w").write(" ".join(map(str, ids)))
    elif args[0] == "all":
        ids = [p["id"] for p in census["people"] if p["household"] is not None and p["away"] is None]
    else:
        ids = [int(a) for a in args]
    for pid in ids:
        out = person(f"p{pid}", spec_from_census(census, pid), os.path.join(OUT, f"p{pid}"))
        p = census["people"][pid]
        print("wrote", out, p["family_name"], p["given_name"], p["age"], p["look"]["day"]["outfit"], p["look"]["hair"])
